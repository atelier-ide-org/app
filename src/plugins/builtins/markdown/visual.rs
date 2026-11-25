//! The visual editor of Markdown: the document is shown rendered, block by block (a paragraph,
//! a title, a list, a table…), and a click on a block turns it into its source, styled, to be
//! edited there; the others stay rendered. A toolbar and the usual shortcuts format the text.

use super::{preview_style, show_markdown};
use crate::{plugins::builtins::mermaid, ui};
use eframe::egui::{
    self, Color32, FontId, Key, Margin, Rect, Sense, Stroke, TextFormat,
    text::{CCursor, CCursorRange, LayoutJob},
};
use egui_commonmark::CommonMarkCache;
use pulldown_cmark::{Event, Options, Parser};
use std::{collections::HashMap, ops::Range};

/// The visual editor of one document.
#[derive(Default)]
pub struct Visual {
    active: Option<Active>,
    /// Where each block was drawn by the last frame, by its start in the text: a click there
    /// edits it, links and boxes of the block answering first.
    rects: HashMap<usize, Rect>,
}

/// The block being edited.
struct Active {
    start: usize,
    /// Its source, as it is in the text from `start`.
    buffer: String,
    /// The length of the text before the block was added at its end: it goes away again when
    /// it is left empty.
    added_at: Option<usize>,
    /// The editor takes the keyboard at the next frame.
    focus: bool,
    /// Where the caret goes in `buffer` at the next frame: where the block was clicked.
    caret: Option<usize>,
}

impl Active {
    fn range(&self) -> Range<usize> {
        self.start..self.start + self.buffer.len()
    }
}

/// What the toolbar or a shortcut asks of the text being edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Format {
    /// Surrounds the selection with these marks, or removes them.
    Wrap(&'static str, &'static str),
    Link,
    Heading(usize),
    /// Starts the lines of the selection with this, or removes it.
    Prefix(&'static str),
    Numbered,
    /// A block of its own after the line of the caret.
    Insert(&'static str),
}

const TABLE: &str = "| Colonne | Colonne |\n| --- | --- |\n| | |";
const CODE: &str = "```\n\n```";

pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_DEFINITION_LIST
}

/// The blocks of the document, as ranges of whole lines without their last line break: every
/// part of the text that is not blank is in one of them (a definition of a link reference, which
/// renders as nothing, is a block too, so that it can be edited).
pub(crate) fn blocks(text: &str) -> Vec<Range<usize>> {
    let mut found: Vec<Range<usize>> = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(_) => {
                if depth == 0 {
                    start = range.start;
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    found.push(whole_lines(text, start..range.end));
                }
            }
            _ if depth == 0 => found.push(whole_lines(text, range)),
            _ => {}
        }
    }
    found.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in found {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    // What the parser gave no event for, and is not blank.
    let mut all = Vec::new();
    let mut at = 0;
    for range in merged {
        all.extend(loose_lines(text, at..range.start));
        at = range.end;
        all.push(range);
    }
    all.extend(loose_lines(text, at..text.len()));
    all
}

/// `range` grown to the start of its first line, and cut before the line breaks at its end.
fn whole_lines(text: &str, range: Range<usize>) -> Range<usize> {
    let start = text[..range.start].rfind('\n').map_or(0, |i| i + 1);
    let mut end = range.end.min(text.len()).max(start);
    while end > start && matches!(text.as_bytes()[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    start..end
}

/// The text of `range` that is not blank, as one block.
fn loose_lines(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let piece = &text[range.clone()];
    let first = piece.find(|c: char| !c.is_whitespace())?;
    Some(whole_lines(
        text,
        range.start + first..range.start + piece.trim_end().len(),
    ))
}

/// Blank lines between two blocks beyond the first one: CommonMark makes them one, the
/// editor keeps their room.
fn extra_blank_lines(gap: &str) -> usize {
    gap.matches('\n').count().saturating_sub(2)
}

impl Visual {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cache: &mut CommonMarkCache,
        id: egui::Id,
        text: &mut String,
        base: &str,
    ) {
        let t = ui::theme(ui.ctx());
        // The text changed elsewhere (an AI, the file on disk): the block is left.
        if self
            .active
            .as_ref()
            .is_some_and(|a| text.get(a.range()) != Some(a.buffer.as_str()))
        {
            self.active = None;
        }
        let mut format = toolbar(ui, &t);
        let width = ui.available_width();
        let mut leave = false;
        let mut edit: Option<(Range<usize>, usize)> = None;
        let mut append = false;
        egui::ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(Margin::symmetric(28, 18))
                    .show(ui, |ui| {
                        ui.set_max_width((width - 56.0).max(200.0));
                        preview_style(ui, &t);
                        let max_width = (width - 80.0).max(100.0);
                        let blocks = blocks(text);
                        let active = self.active.as_ref().map(Active::range);
                        let mut editor_drawn = false;
                        let mut previous_end: Option<usize> = None;
                        let mut rects = HashMap::new();
                        let mut ticked: Vec<(Range<usize>, String)> = Vec::new();
                        for block in &blocks {
                            if let Some(range) = &active {
                                let inside = block.start < range.end && block.end > range.start
                                    || (block.start == range.start && !range.is_empty());
                                if inside {
                                    continue;
                                }
                                if !editor_drawn && block.start >= range.end {
                                    editor_drawn = true;
                                    if previous_end.is_some() {
                                        ui.add_space(6.0);
                                    }
                                    if let Some(a) = &mut self.active {
                                        leave |= editor(ui, id, a, text, format.take(), &t);
                                    }
                                    previous_end = Some(range.end);
                                }
                            }
                            if let Some(end) = previous_end {
                                // The room of a paragraph, and of the blank lines kept.
                                let extra = extra_blank_lines(&text[end..block.start]);
                                ui.add_space(6.0 + extra as f32 * 14.0);
                            }
                            previous_end = Some(block.end);
                            let source = &text[block.clone()];
                            let (rect, caret, changed) = rendered_block(
                                ui,
                                cache,
                                id.with(("block", block.start)),
                                source,
                                self.rects.get(&block.start).copied(),
                                base,
                                max_width,
                                &t,
                            );
                            rects.insert(block.start, rect);
                            if let Some(caret) = caret {
                                edit = Some((block.clone(), caret));
                            }
                            if let Some(changed) = changed {
                                ticked.push((block.clone(), changed));
                            }
                        }
                        if !editor_drawn && let Some(a) = &mut self.active {
                            if previous_end.is_some() {
                                ui.add_space(6.0);
                            }
                            leave |= editor(ui, id, a, text, format.take(), &t);
                        }
                        self.rects = rects;
                        // A box ticked: the same length, nothing moves.
                        for (range, changed) in ticked.into_iter().rev() {
                            text.replace_range(range, &changed);
                        }
                        // The room below the document: a click there adds a paragraph.
                        let height = ui.available_height().max(120.0);
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), height),
                            Sense::click(),
                        );
                        if response.hovered() && self.active.is_none() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                            let hint = if text.trim().is_empty() {
                                tr!("markdown.visual_start")
                            } else {
                                tr!("markdown.visual_add_paragraph")
                            };
                            ui.painter().text(
                                rect.left_top() + egui::vec2(0.0, 6.0),
                                egui::Align2::LEFT_TOP,
                                hint,
                                FontId::proportional(13.0),
                                t.text_disabled,
                            );
                        }
                        if response.clicked() {
                            if self.active.is_some() {
                                leave = true;
                            } else {
                                append = true;
                            }
                        }
                    });
            });
        if leave || edit.is_some() || append {
            self.leave(text);
        }
        if let Some((range, caret)) = edit {
            self.active = Some(Active {
                start: range.start,
                buffer: text[range].to_string(),
                added_at: None,
                focus: true,
                caret: Some(caret),
            });
        }
        // An action of the toolbar with no block being edited: a new paragraph gets it.
        if append || (format.is_some() && self.active.is_none()) {
            self.append(text);
        }
        if let (Some(format), Some(a)) = (format, &mut self.active) {
            // Applied at the next frame, by the editor, with the selection it keeps.
            ui.ctx()
                .data_mut(|d| d.insert_temp(id.with("pending"), Some(format)));
            a.focus = true;
            ui.ctx().request_repaint();
        }
    }

    /// Stops editing: a block added and left empty goes away.
    fn leave(&mut self, text: &mut String) {
        let Some(active) = self.active.take() else {
            return;
        };
        if let Some(length) = active.added_at
            && active.buffer.trim().is_empty()
            && text.len() >= length
        {
            text.truncate(length);
        }
    }

    /// Adds an empty paragraph at the end of the document and edits it.
    fn append(&mut self, text: &mut String) {
        let length = text.len();
        let trimmed = text.trim_end_matches(['\n', '\r', ' ', '\t']).len();
        text.truncate(trimmed);
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        self.active = Some(Active {
            start: text.len(),
            buffer: String::new(),
            added_at: Some(length.min(text.len())),
            focus: true,
            caret: None,
        });
    }
}

/// The buttons that format the text being edited. Returns the one clicked.
fn toolbar(ui: &mut egui::Ui, t: &ui::Theme) -> Option<Format> {
    let mut clicked = None;
    egui::Frame::new()
        .fill(t.frame)
        .inner_margin(Margin::symmetric(12, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let mut button =
                    |ui: &mut egui::Ui, label: egui::RichText, tip: &str, f: Format| {
                        let response = ui
                            .add(
                                egui::Button::new(label)
                                    .frame(false)
                                    .min_size(egui::vec2(ui::BUTTON, ui::BUTTON)),
                            )
                            .on_hover_text(tip);
                        if response.clicked() {
                            clicked = Some(f);
                        }
                    };
                let plain = |s: &str| egui::RichText::new(s).size(13.0).color(t.text);
                button(
                    ui,
                    egui::RichText::new("B")
                        .font(ui::semibold(14.0))
                        .color(t.text),
                    tr!("markdown.visual_bold"),
                    Format::Wrap("**", "**"),
                );
                button(
                    ui,
                    plain("I").italics(),
                    tr!("markdown.visual_italic"),
                    Format::Wrap("*", "*"),
                );
                button(
                    ui,
                    plain("S").strikethrough(),
                    tr!("markdown.visual_strike"),
                    Format::Wrap("~~", "~~"),
                );
                button(
                    ui,
                    egui::RichText::new("</>")
                        .font(FontId::monospace(12.0))
                        .color(t.text),
                    tr!("markdown.visual_code"),
                    Format::Wrap("`", "`"),
                );
                button(ui, plain("🔗"), tr!("markdown.visual_link"), Format::Link);
                ui.separator();
                for level in 1..=3 {
                    button(
                        ui,
                        egui::RichText::new(format!("H{level}"))
                            .font(ui::semibold(13.0))
                            .color(t.text),
                        tr!("markdown.visual_heading"),
                        Format::Heading(level),
                    );
                }
                ui.separator();
                button(
                    ui,
                    plain("•"),
                    tr!("markdown.visual_bullets"),
                    Format::Prefix("- "),
                );
                button(
                    ui,
                    plain("1."),
                    tr!("markdown.visual_numbers"),
                    Format::Numbered,
                );
                button(
                    ui,
                    plain("☐"),
                    tr!("markdown.visual_tasks"),
                    Format::Prefix("- [ ] "),
                );
                button(
                    ui,
                    egui::RichText::new("“")
                        .font(ui::semibold(18.0))
                        .color(t.text),
                    tr!("markdown.visual_quote"),
                    Format::Prefix("> "),
                );
                ui.separator();
                button(
                    ui,
                    egui::RichText::new("{ }")
                        .font(FontId::monospace(12.0))
                        .color(t.text),
                    tr!("markdown.visual_code_block"),
                    Format::Insert(CODE),
                );
                button(
                    ui,
                    plain("―"),
                    tr!("markdown.visual_rule"),
                    Format::Insert("---"),
                );
                // A table: a grid drawn, no font has a glyph for it everywhere.
                let response = ui
                    .add(
                        egui::Button::new("")
                            .frame(false)
                            .min_size(egui::vec2(ui::BUTTON, ui::BUTTON)),
                    )
                    .on_hover_text(tr!("markdown.visual_table"));
                let grid = Rect::from_center_size(response.rect.center(), egui::vec2(12.0, 10.0));
                let stroke = Stroke::new(1.0, t.text);
                let painter = ui.painter();
                painter.rect_stroke(grid, 1.0, stroke, egui::StrokeKind::Inside);
                painter.hline(grid.x_range(), grid.center().y, stroke);
                painter.vline(grid.center().x, grid.y_range(), stroke);
                if response.clicked() {
                    clicked = Some(Format::Insert(TABLE));
                }
            });
        });
    ui.add(egui::Separator::default().spacing(0.0));
    clicked
}

/// Draws a block rendered. Returns where, the place of its source that was clicked to edit it,
/// and its new source when a box of it was ticked.
#[allow(clippy::too_many_arguments)]
fn rendered_block(
    ui: &mut egui::Ui,
    cache: &mut CommonMarkCache,
    id: egui::Id,
    source: &str,
    last_rect: Option<Rect>,
    base: &str,
    max_width: f32,
    t: &ui::Theme,
) -> (Rect, Option<usize>, Option<String>) {
    // Registered before the content: a link or a box of the block answers first.
    let response = last_rect.map(|rect| ui.interact(rect.expand(4.0), id, Sense::click()));
    let background = ui.painter().add(egui::Shape::Noop);
    let mut piece = source.to_string();
    let inner = ui.push_id(id, |ui| {
        ui.set_width(ui.available_width());
        match mermaid_source(source) {
            Some(diagram) => mermaid::show(ui, diagram, max_width),
            None => show_markdown(ui, cache, &mut piece, base, max_width),
        }
    });
    let rect = inner.response.rect;
    let hovered = response.as_ref().is_some_and(|r| r.hovered());
    if hovered {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(rect.expand(4.0), 4.0, t.hover.gamma_multiply(0.6)),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    let caret = response
        .filter(|r| r.clicked())
        .map(|r| match r.interact_pointer_pos() {
            Some(pointer) => caret_at(ui, source, pointer - rect.min, rect.size()),
            None => source.len(),
        });
    let changed = (piece != source).then_some(piece);
    (rect, caret, changed)
}

/// The place of the source of a block under `at`, a point of its rendering from its top left,
/// `size` being the size of the rendering: its text is laid out again with the fonts of the
/// rendering, the character under the point found there, and its place in the source.
pub(crate) fn caret_at(ui: &egui::Ui, source: &str, at: egui::Vec2, size: egui::Vec2) -> usize {
    if mermaid_source(source).is_some() {
        return 0;
    }
    let (job, map) = rendered_text(ui, source, size.x.max(10.0));
    if map.is_empty() {
        return source.len();
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    // The rendering spaces its lines more (lists, tables, paragraphs): the same height.
    let scale = galley.rect.height() / size.y.max(1.0);
    let cursor = galley.cursor_from_pos(egui::vec2(at.x, at.y * scale));
    let byte = byte_of(galley.text(), cursor.index.0);
    source_offset(&map, byte, source.len())
}

/// A piece of the rendered text and the source it comes from.
type Mapped = (Range<usize>, Range<usize>);

/// The place in the source of the byte `at` of the rendered text.
fn source_offset(map: &[Mapped], at: usize, length: usize) -> usize {
    for (shown, from) in map {
        if at <= shown.end {
            if at < shown.start {
                return from.start;
            }
            return (from.start + (at - shown.start)).min(from.end);
        }
    }
    map.last().map_or(length, |(_, from)| from.end)
}

/// The text a block renders, laid out with the fonts of the rendering (titles, bold, code),
/// and where each piece of it comes from in the source.
fn rendered_text(ui: &egui::Ui, source: &str, width: f32) -> (LayoutJob, Vec<Mapped>) {
    use pulldown_cmark::{Tag, TagEnd};
    let styles = &ui.style().text_styles;
    let body = styles.get(&egui::TextStyle::Body).map_or(14.0, |f| f.size);
    let biggest = styles
        .get(&egui::TextStyle::Heading)
        .map_or(28.0, |f| f.size);
    let mono = styles
        .get(&egui::TextStyle::Monospace)
        .map_or(13.0, |f| f.size);
    let font = |heading: Option<usize>, strong: bool, code: bool| -> FontId {
        if code {
            return FontId::monospace(mono);
        }
        match heading {
            // The sizes of egui_commonmark.
            Some(level) => {
                let factor = [1.0, 0.835, 0.668, 0.501, 0.334, 0.167][level.min(5)];
                let size = body + (biggest - body) * factor;
                if level <= 3 {
                    ui::semibold(size)
                } else {
                    FontId::proportional(size)
                }
            }
            None if strong => ui::semibold(body),
            None => FontId::proportional(body),
        }
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut map: Vec<Mapped> = Vec::new();
    let mut push = |job: &mut LayoutJob, text: &str, font: FontId, from: Option<Range<usize>>| {
        let start = job.text.len();
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font,
                ..TextFormat::default()
            },
        );
        if let Some(from) = from {
            map.push((start..job.text.len(), from));
        }
    };
    let open_line = |job: &LayoutJob| !job.text.is_empty() && !job.text.ends_with('\n');
    let (mut heading, mut strong, mut code_block, mut depth) = (None, false, false, 0usize);
    for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = Some(level as usize - 1),
            Event::End(TagEnd::Heading(_)) => heading = None,
            Event::Start(Tag::Strong) => strong = true,
            Event::End(TagEnd::Strong) => strong = false,
            Event::Start(Tag::CodeBlock(_)) => code_block = true,
            Event::End(TagEnd::CodeBlock) => code_block = false,
            Event::Start(Tag::List(_)) => depth += 1,
            Event::End(TagEnd::List(_)) => depth = depth.saturating_sub(1),
            Event::Start(Tag::Item) => {
                if open_line(&job) {
                    push(&mut job, "\n", font(None, false, false), None);
                }
                // The room of the bullet and of the indentation.
                let room = " ".repeat(5 + 4 * depth.saturating_sub(1));
                push(&mut job, &room, font(None, false, false), None);
            }
            Event::TaskListMarker(_) => push(&mut job, "    ", font(None, false, false), None),
            Event::End(TagEnd::Paragraph | TagEnd::Item | TagEnd::TableRow | TagEnd::TableHead) => {
                if open_line(&job) {
                    push(&mut job, "\n", font(None, false, false), None);
                }
            }
            Event::End(TagEnd::TableCell) => {
                push(&mut job, "    ", font(None, false, false), None);
            }
            // In the source, the text sits at the start of its range (an escape or an entity
            // makes the source longer).
            Event::Text(text) => {
                push(
                    &mut job,
                    &text,
                    font(heading, strong, code_block),
                    Some(range),
                );
            }
            Event::Code(text) => {
                let inner = source[range.clone()]
                    .find(text.as_ref())
                    .map_or(range.start, |i| range.start + i);
                push(
                    &mut job,
                    &text,
                    font(heading, strong, true),
                    Some(inner..inner + text.len()),
                );
            }
            Event::SoftBreak => push(&mut job, " ", font(heading, strong, false), Some(range)),
            Event::HardBreak => push(&mut job, "\n", font(None, false, false), Some(range)),
            _ => {}
        }
    }
    // The line break that ends the last paragraph would be a row of its own.
    if job.text.ends_with('\n')
        && job
            .sections
            .last()
            .is_some_and(|s| usize::from(s.byte_range.end) - usize::from(s.byte_range.start) == 1)
    {
        job.sections.pop();
        job.text.pop();
    }
    (job, map)
}

/// The diagram of a block that is a fenced `mermaid` block.
fn mermaid_source(source: &str) -> Option<&str> {
    let first = source.lines().next()?.trim();
    let fence = if first.starts_with("```") {
        "```"
    } else {
        "~~~"
    };
    if first.strip_prefix(fence)?.trim() != "mermaid" {
        return None;
    }
    let body = &source[source.find('\n')? + 1..];
    let end = body.trim_end().rfind('\n').map_or(0, |i| i + 1);
    let last = body.trim_end()[end..].trim();
    Some(if last.starts_with(fence) {
        &body[..end]
    } else {
        body
    })
}

/// The editor of the block being edited. Returns true when the user leaves it (Escape).
fn editor(
    ui: &mut egui::Ui,
    visual: egui::Id,
    active: &mut Active,
    text: &mut String,
    format: Option<Format>,
    t: &ui::Theme,
) -> bool {
    let id = visual.with("editor");
    let pending = ui
        .ctx()
        .data_mut(|d| d.remove_temp::<Option<Format>>(visual.with("pending")))
        .flatten();
    let focused = ui.memory(|m| m.has_focus(id));
    // The shortcuts of formatting, while the editor has the keyboard.
    let shortcut = if focused {
        ui.input_mut(|i| {
            let ctrl = egui::Modifiers::COMMAND;
            if i.consume_key(ctrl, Key::B) {
                Some(Format::Wrap("**", "**"))
            } else if i.consume_key(ctrl, Key::I) {
                Some(Format::Wrap("*", "*"))
            } else if i.consume_key(ctrl, Key::K) {
                Some(Format::Link)
            } else {
                None
            }
        })
    } else {
        None
    };
    let before = active.buffer.len();
    if let Some(format) = format.or(pending).or(shortcut) {
        let state = egui::TextEdit::load_state(ui.ctx(), id);
        let chars = state
            .as_ref()
            .and_then(|s| s.cursor.char_range())
            .map(|r| {
                let [a, b] = r.sorted_cursors();
                a.index.0..b.index.0
            })
            .unwrap_or_else(|| {
                let end = active.buffer.chars().count();
                end..end
            });
        let bytes = byte_of(&active.buffer, chars.start)..byte_of(&active.buffer, chars.end);
        let selected = apply(&mut active.buffer, bytes, format);
        let mut state = state.unwrap_or_default();
        let (a, b) = (
            active.buffer[..selected.start].chars().count(),
            active.buffer[..selected.end].chars().count(),
        );
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
        egui::TextEdit::store_state(ui.ctx(), id, state);
        active.focus = true;
    }
    let theme = *t;
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap: f32| {
        let mut job = highlight(buffer.as_str(), &theme);
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let output = egui::TextEdit::multiline(&mut active.buffer)
        .id(id)
        .desired_width(f32::INFINITY)
        .desired_rows(1)
        .lock_focus(true)
        .frame(
            egui::Frame::new()
                .fill(t.field)
                .stroke(Stroke::new(1.0, t.accent))
                .corner_radius(6)
                .inner_margin(Margin::symmetric(10, 8))
                // The text stays where it was rendered, the frame around it.
                .outer_margin(Margin::symmetric(-10, -8)),
        )
        .layouter(&mut layouter)
        .show(ui);
    if active.focus {
        output.response.request_focus();
        active.focus = false;
    }
    // The caret where the block was clicked, once the editor exists.
    if let Some(caret) = active.caret.take() {
        let caret = active.buffer[..caret.min(active.buffer.len())]
            .chars()
            .count();
        let mut state = output.state;
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(caret))));
        egui::TextEdit::store_state(ui.ctx(), id, state);
        ui.ctx().request_repaint();
    }
    // The text follows the editor.
    if active.buffer.len() != before
        || text.get(active.start..active.start + before) != Some(active.buffer.as_str())
    {
        let end = (active.start + before).min(text.len());
        text.replace_range(active.start..end, &active.buffer);
    }
    focused && ui.input(|i| i.key_pressed(Key::Escape))
}

fn byte_of(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(i, _)| i)
}

/// The start of the line of `at`, and the end of the line of `to`.
fn line_span(text: &str, at: usize, to: usize) -> Range<usize> {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let end = text[to..].find('\n').map_or(text.len(), |i| to + i);
    start..end
}

/// The list or quote mark at the start of a line: its length.
fn line_mark(line: &str) -> usize {
    let rest = line.trim_start();
    let indent = line.len() - rest.len();
    for mark in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ ", "> "] {
        if rest.starts_with(mark) {
            return indent + mark.len();
        }
    }
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") ")) {
        return indent + digits + 2;
    }
    0
}

/// Formats `selection` of `text`; returns what is selected after.
pub(crate) fn apply(text: &mut String, selection: Range<usize>, format: Format) -> Range<usize> {
    match format {
        Format::Wrap(left, right) => {
            let wrapped = selection.start >= left.len()
                && text[..selection.start].ends_with(left)
                && text[selection.end..].starts_with(right);
            if wrapped {
                text.replace_range(selection.end..selection.end + right.len(), "");
                text.replace_range(selection.start - left.len()..selection.start, "");
                selection.start - left.len()..selection.end - left.len()
            } else {
                text.insert_str(selection.end, right);
                text.insert_str(selection.start, left);
                selection.start + left.len()..selection.end + left.len()
            }
        }
        Format::Link => {
            if selection.is_empty() {
                let label = tr!("markdown.visual_link_text");
                let inserted = format!("[{label}](https://)");
                text.insert_str(selection.start, &inserted);
                selection.start + 1..selection.start + 1 + label.len()
            } else {
                text.insert_str(selection.end, "](https://)");
                text.insert(selection.start, '[');
                // The address, to be typed over.
                let url = selection.end + 3;
                url..url + "https://".len()
            }
        }
        Format::Heading(level) => {
            let span = line_span(text, selection.start, selection.end);
            let lines: Vec<String> = text[span.clone()]
                .split('\n')
                .map(|line| {
                    let hashes = line.chars().take_while(|c| *c == '#').count();
                    let bare = if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
                        &line[hashes + 1..]
                    } else {
                        line
                    };
                    if hashes == level {
                        bare.to_string()
                    } else {
                        format!("{} {bare}", "#".repeat(level))
                    }
                })
                .collect();
            replace_lines(text, span, &lines)
        }
        Format::Prefix(prefix) => {
            let span = line_span(text, selection.start, selection.end);
            let all = text[span.clone()]
                .split('\n')
                .all(|line| line.trim_start().starts_with(prefix));
            let lines: Vec<String> = text[span.clone()]
                .split('\n')
                .map(|line| {
                    let mark = line_mark(line);
                    if all {
                        line[mark..].to_string()
                    } else {
                        format!("{prefix}{}", &line[mark..])
                    }
                })
                .collect();
            replace_lines(text, span, &lines)
        }
        Format::Numbered => {
            let span = line_span(text, selection.start, selection.end);
            let lines: Vec<String> = text[span.clone()]
                .split('\n')
                .enumerate()
                .map(|(i, line)| format!("{}. {}", i + 1, &line[line_mark(line)..]))
                .collect();
            replace_lines(text, span, &lines)
        }
        Format::Insert(snippet) => {
            let end = line_span(text, selection.end, selection.end).end;
            let before = if end == 0 { "" } else { "\n\n" };
            let inserted = format!("{before}{snippet}");
            text.insert_str(end, &inserted);
            // Inside a code block, or at the first cell of a table.
            let inner = match snippet {
                CODE => 4,
                TABLE => TABLE.rfind("| |").map_or(0, |i| i + 2),
                _ => snippet.len(),
            };
            let caret = end + before.len() + inner;
            caret..caret
        }
    }
}

fn replace_lines(text: &mut String, span: Range<usize>, lines: &[String]) -> Range<usize> {
    let joined = lines.join("\n");
    let start = span.start;
    text.replace_range(span, &joined);
    let end = start + joined.len();
    end..end
}

/// The source being edited, styled as it renders: big titles, bold, italic, code in a fixed
/// font, the marks of Markdown discreet.
pub(crate) fn highlight(text: &str, t: &ui::Theme) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mark = t.text_disabled;
    let mut in_fence = false;
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            job.append("\n", 0.0, style(14.0, t.text, false, false, false, false));
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            job.append(line, 0.0, style(13.0, mark, false, false, false, true));
            continue;
        }
        if in_fence || trimmed.starts_with('|') {
            job.append(line, 0.0, style(13.0, t.text, false, false, false, true));
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
            let size = [26.0, 22.0, 19.0, 17.0, 15.0, 14.0][hashes - 1];
            job.append(
                &line[..=hashes],
                0.0,
                style(size, mark, true, false, false, false),
            );
            inline(&mut job, &line[hashes + 1..], size, true, t.text, t);
            continue;
        }
        if matches!(trimmed, "---" | "***" | "___") {
            job.append(line, 0.0, style(14.0, mark, false, false, false, false));
            continue;
        }
        let marked = line_mark(line);
        let color = if trimmed.starts_with('>') {
            t.text_secondary
        } else {
            t.text
        };
        if marked > 0 {
            let accent = if trimmed.starts_with('>') {
                mark
            } else {
                t.accent
            };
            job.append(
                &line[..marked],
                0.0,
                style(14.0, accent, false, false, false, false),
            );
        }
        inline(&mut job, &line[marked..], 14.0, false, color, t);
    }
    job
}

fn style(
    size: f32,
    color: Color32,
    bold: bool,
    italic: bool,
    strike: bool,
    mono: bool,
) -> TextFormat {
    TextFormat {
        font_id: if mono {
            FontId::monospace(size - 1.0)
        } else if bold {
            ui::semibold(size)
        } else {
            FontId::proportional(size)
        },
        color,
        italics: italic,
        strikethrough: if strike {
            Stroke::new(1.0, color)
        } else {
            Stroke::NONE
        },
        ..TextFormat::default()
    }
}

/// The emphasis, code and links of a line.
fn inline(
    job: &mut LayoutJob,
    line: &str,
    size: f32,
    heading: bool,
    color: Color32,
    t: &ui::Theme,
) {
    let mark = t.text_disabled;
    let (mut bold, mut italic, mut strike) = (heading, false, false);
    let mut run = String::new();
    let flush = |job: &mut LayoutJob, run: &mut String, bold: bool, italic: bool, strike: bool| {
        if !run.is_empty() {
            job.append(run, 0.0, style(size, color, bold, italic, strike, false));
            run.clear();
        }
    };
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        let marker = |job: &mut LayoutJob, m: &str| {
            job.append(m, 0.0, style(size, mark, false, false, false, false));
        };
        if c == '`'
            && let Some(close) = rest[1..].find('`')
        {
            flush(job, &mut run, bold, italic, strike);
            marker(job, "`");
            let mut code = style(size, t.text, false, false, false, true);
            code.background = t.hover;
            job.append(&rest[1..=close], 0.0, code);
            marker(job, "`");
            rest = &rest[close + 2..];
            continue;
        }
        if rest.starts_with("**") || rest.starts_with("__") {
            flush(job, &mut run, bold, italic, strike);
            marker(job, &rest[..2]);
            bold = !bold;
            rest = &rest[2..];
            continue;
        }
        if rest.starts_with("~~") {
            flush(job, &mut run, bold, italic, strike);
            marker(job, "~~");
            strike = !strike;
            rest = &rest[2..];
            continue;
        }
        if c == '*' {
            flush(job, &mut run, bold, italic, strike);
            marker(job, "*");
            italic = !italic;
            rest = &rest[1..];
            continue;
        }
        if c == '['
            && let Some(mid) = rest.find("](")
            && let Some(close) = rest[mid..].find(')')
        {
            flush(job, &mut run, bold, italic, strike);
            marker(job, "[");
            job.append(
                &rest[1..mid],
                0.0,
                style(size, t.link, bold, italic, strike, false),
            );
            marker(job, &rest[mid..mid + close + 1]);
            rest = &rest[mid + close + 1..];
            continue;
        }
        run.push(c);
        rest = &rest[c.len_utf8()..];
    }
    flush(job, &mut run, bold, italic, strike);
}

#[cfg(test)]
#[path = "../../../../test/plugins/builtins/markdown/visual/tests.rs"]
mod tests;
