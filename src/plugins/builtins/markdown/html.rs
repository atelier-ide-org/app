//! HTML inside Markdown, as GitHub renders it: the blocks (a centered logo, badges, a title) are
//! drawn by a small renderer of the common tags, and the tags inside a paragraph become their
//! Markdown equivalent before the Markdown viewer reads the text.

use crate::ui;
use eframe::egui::{
    self, Align, FontId, RichText, Sense, Stroke, pos2,
    text::{LayoutJob, TextFormat},
    vec2,
};
use pulldown_cmark::{Event, Options, Parser};
use std::ops::Range;

/// The options of the Markdown viewer: the same text must be seen the same way.
fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_MATH
}

/// A tag, `<a href="x">` or `</a>`.
#[derive(Debug, PartialEq)]
struct Tag {
    name: String,
    closing: bool,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The tag written in `s` (from `<` to `>`); None for a comment, a doctype or a broken tag.
fn parse_tag(s: &str) -> Option<Tag> {
    let inner = s.strip_prefix('<')?.strip_suffix('>')?;
    let inner = inner.strip_suffix('/').unwrap_or(inner);
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let end = inner
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inner.len());
    let name = inner[..end].to_ascii_lowercase();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let mut attrs = Vec::new();
    let mut rest = inner[end..].trim_start();
    while !rest.is_empty() {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(rest.len());
        let key = rest[..end].to_ascii_lowercase();
        rest = rest[end..].trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let (v, next) = match after.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let close = after[1..].find(q).map_or(after.len(), |i| i + 1);
                    (&after[1..close], after.get(close + 1..).unwrap_or(""))
                }
                _ => {
                    let close = after
                        .find(|c: char| c.is_whitespace())
                        .unwrap_or(after.len());
                    (&after[..close], &after[close..])
                }
            };
            value = decode_entities(v);
            rest = next.trim_start();
        }
        if !key.is_empty() {
            attrs.push((key, value));
        }
    }
    Some(Tag {
        name,
        closing,
        attrs,
    })
}

/// `&amp;`, `&#39;`… replaced by their character.
pub(super) fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let entity = rest[1..]
            .find(';')
            .filter(|&end| end <= 10)
            .map(|end| &rest[1..end + 1]);
        let decoded = entity.and_then(|e| match e {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            "copy" => Some('©'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            _ => {
                let number = e.strip_prefix('#')?;
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => number.parse().ok()?,
                };
                char::from_u32(code)
            }
        });
        match (entity, decoded) {
            (Some(e), Some(c)) => {
                out.push(c);
                rest = &rest[e.len() + 2..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Markdown text whose inline HTML was replaced by Markdown, and where each replacement was.
pub(super) struct Converted {
    pub text: String,
    /// (range in the source, length of what replaced it), in order.
    replaced: Vec<(Range<usize>, usize)>,
}

impl Converted {
    /// The offset in the source of `offset` in the converted text (outside a replacement).
    pub fn source_offset(&self, offset: usize) -> usize {
        let mut shift: isize = 0;
        for (range, len) in &self.replaced {
            let start = (range.start as isize + shift) as usize;
            if offset < start + len {
                break;
            }
            shift += *len as isize - range.len() as isize;
        }
        (offset as isize - shift) as usize
    }
}

/// The text with the tags of its paragraphs (`<br>`, `<b>`, `<img>`, `<a href>`…) written in
/// Markdown, or None when it has none. HTML blocks are left to [`show`].
pub(super) fn inline_to_markdown(text: &str) -> Option<Converted> {
    let tags: Vec<Range<usize>> = Parser::new_ext(text, parser_options())
        .into_offset_iter()
        .filter(|(event, _)| matches!(event, Event::InlineHtml(_)))
        .map(|(_, range)| range)
        .collect();
    if tags.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut replaced = Vec::new();
    let mut links: Vec<Option<String>> = Vec::new();
    let mut last = 0;
    for range in tags {
        out.push_str(&text[last..range.start]);
        let source = &text[range.clone()];
        let after = &text[range.end..];
        let markdown = match parse_tag(source) {
            None => String::new(),
            Some(tag) => match (tag.name.as_str(), tag.closing) {
                ("br", _) => {
                    // A break at the end of a paragraph would show its backslash.
                    let next_line = after.strip_prefix('\n');
                    let ends = after.trim().is_empty()
                        || next_line
                            .is_some_and(|r| r.trim_start_matches([' ', '\t']).starts_with('\n'));
                    match next_line {
                        _ if ends => String::new(),
                        Some(_) => "\\".into(),
                        None => "\\\n".into(),
                    }
                }
                ("b" | "strong", _) => "**".into(),
                ("i" | "em", _) => "*".into(),
                ("s" | "del" | "strike", _) => "~~".into(),
                ("code" | "kbd" | "tt" | "samp", _) => "`".into(),
                ("img", false) => tag.attr("src").map_or(String::new(), |src| {
                    format!("![{}](<{}>)", tag.attr("alt").unwrap_or(""), src)
                }),
                ("a", false) => {
                    let href = tag.attr("href").map(str::to_string);
                    let open = if href.is_some() { "[" } else { "" };
                    links.push(href);
                    open.into()
                }
                ("a", true) => match links.pop().flatten() {
                    Some(href) => format!("](<{href}>)"),
                    None => String::new(),
                },
                _ => String::new(),
            },
        };
        replaced.push((range.clone(), markdown.len()));
        out.push_str(&markdown);
        last = range.end;
    }
    out.push_str(&text[last..]);
    Some(Converted {
        text: out,
        replaced,
    })
}

/// A piece of a line of an HTML block.
#[derive(Debug, Clone, PartialEq)]
enum Inline {
    Text {
        text: String,
        strong: bool,
        italic: bool,
        code: bool,
        strike: bool,
        link: Option<String>,
    },
    Image {
        src: String,
        alt: String,
        width: Option<f32>,
        height: Option<f32>,
        link: Option<String>,
    },
    Break,
}

#[derive(Debug, PartialEq)]
enum Line {
    Rule,
    Content {
        align: Align,
        heading: Option<u8>,
        indent: usize,
        items: Vec<Inline>,
    },
    /// A table: its rows, and the pieces of each of their cells.
    Table {
        align: Align,
        rows: Vec<Vec<Vec<Inline>>>,
    },
}

/// The table being read.
struct Table {
    align: Align,
    rows: Vec<Vec<Vec<Inline>>>,
    /// A cell is open: what is read goes into it.
    in_cell: bool,
    /// Tables inside this one: their cells are lines of the cell they are in.
    nested: usize,
}

#[derive(Default)]
struct Builder {
    lines: Vec<Line>,
    items: Vec<Inline>,
    aligns: Vec<(String, Align)>,
    heading: Option<u8>,
    strong: usize,
    italic: usize,
    code: usize,
    strike: usize,
    pre: usize,
    hidden: usize,
    links: Vec<Option<String>>,
    /// Lists being written: the number of the next item, None for a bulleted list.
    lists: Vec<Option<usize>>,
    table: Option<Table>,
}

impl Builder {
    fn align(&self) -> Align {
        self.aligns.last().map_or(Align::Min, |(_, a)| *a)
    }

    fn flush(&mut self) {
        if let Some(table) = &self.table {
            // In a cell, a block starts a line of the cell; between the cells, nothing is kept.
            if !table.in_cell {
                self.items.clear();
            } else if !matches!(self.items.last(), None | Some(Inline::Break)) {
                self.items.push(Inline::Break);
            }
            return;
        }
        self.trim_items();
        if self.items.is_empty() {
            return;
        }
        self.lines.push(Line::Content {
            align: self.align(),
            heading: self.heading,
            indent: self.lists.len(),
            items: std::mem::take(&mut self.items),
        });
    }

    /// Closes the cell being read, if one is.
    fn end_cell(&mut self) {
        if !self.table.as_ref().is_some_and(|t| t.in_cell) {
            return;
        }
        self.trim_items();
        let items = std::mem::take(&mut self.items);
        if let Some(table) = &mut self.table {
            table.in_cell = false;
            match table.rows.last_mut() {
                Some(row) => row.push(items),
                None => table.rows.push(vec![items]),
            }
        }
    }

    /// The tags of a table. Those of a table inside a cell are not followed: its cells are
    /// lines of the cell they are in.
    fn table_tag(&mut self, name: &str, closing: bool) {
        let nested = self.table.as_ref().map(|t| t.nested);
        match (name, closing, nested) {
            ("table", false, None) => {
                self.table = Some(Table {
                    align: self.align(),
                    rows: Vec::new(),
                    in_cell: false,
                    nested: 0,
                });
            }
            ("table", false, Some(_)) => {
                if let Some(table) = &mut self.table {
                    table.nested += 1;
                }
            }
            ("table", true, Some(0)) => {
                self.end_cell();
                if let Some(table) = self.table.take() {
                    let rows: Vec<_> = (table.rows.into_iter())
                        .filter(|row| !row.is_empty())
                        .collect();
                    if !rows.is_empty() {
                        self.lines.push(Line::Table {
                            align: table.align,
                            rows,
                        });
                    }
                }
            }
            ("table", true, Some(_)) => {
                if let Some(table) = &mut self.table {
                    table.nested -= 1;
                }
            }
            ("tr", false, Some(0)) => {
                self.end_cell();
                if let Some(table) = &mut self.table {
                    table.rows.push(Vec::new());
                }
            }
            ("td" | "th", false, Some(0)) => {
                self.end_cell();
                self.items.clear();
                if let Some(table) = &mut self.table {
                    table.in_cell = true;
                }
            }
            ("tr" | "td" | "th", true, Some(0)) => self.end_cell(),
            _ => {}
        }
    }

    /// Takes the breaks and the spaces off the end of what was read.
    fn trim_items(&mut self) {
        while matches!(self.items.last(), Some(Inline::Break)) {
            self.items.pop();
        }
        if let Some(Inline::Text { text, .. }) = self.items.last_mut() {
            let trimmed = text.trim_end().len();
            text.truncate(trimmed);
        }
    }

    fn at_line_start(&self) -> bool {
        match self.items.last() {
            None | Some(Inline::Break) => true,
            Some(Inline::Text { text, .. }) => text.ends_with(' ') || text.ends_with('\n'),
            Some(Inline::Image { .. }) => false,
        }
    }

    fn text(&mut self, raw: &str) {
        if self.hidden > 0 {
            return;
        }
        let decoded = decode_entities(raw);
        let text = if self.pre > 0 {
            decoded
        } else {
            let mut collapsed = String::with_capacity(decoded.len());
            for c in decoded.chars() {
                if c.is_whitespace() && c != '\u{a0}' {
                    if !collapsed.ends_with(' ') {
                        collapsed.push(' ');
                    }
                } else {
                    collapsed.push(c);
                }
            }
            if self.at_line_start() {
                collapsed.trim_start().to_string()
            } else {
                collapsed
            }
        };
        if text.is_empty() {
            return;
        }
        let item = Inline::Text {
            text,
            strong: self.strong > 0 || self.heading.is_some(),
            italic: self.italic > 0,
            code: self.code > 0 || self.pre > 0,
            strike: self.strike > 0,
            link: self.links.iter().rev().find_map(Clone::clone),
        };
        // Pieces of the same style make one label, wrapped as a whole.
        if let (
            Some(Inline::Text {
                text: last,
                strong,
                italic,
                code,
                strike,
                link,
            }),
            Inline::Text {
                text: new,
                strong: s,
                italic: i,
                code: c,
                strike: k,
                link: l,
            },
        ) = (self.items.last_mut(), &item)
            && (*strong, *italic, *code, *strike, link.as_deref()) == (*s, *i, *c, *k, l.as_deref())
        {
            last.push_str(new);
            return;
        }
        self.items.push(item);
    }

    fn tag(&mut self, tag: Tag) {
        let name = tag.name.as_str();
        if matches!(name, "script" | "style" | "template") {
            if tag.closing {
                self.hidden = self.hidden.saturating_sub(1);
            } else {
                self.hidden += 1;
            }
            return;
        }
        if self.hidden > 0 {
            return;
        }
        let block = matches!(
            name,
            "p" | "div"
                | "center"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "ul"
                | "ol"
                | "li"
                | "pre"
                | "blockquote"
                | "details"
                | "summary"
                | "section"
                | "header"
                | "footer"
                | "table"
                | "thead"
                | "tbody"
                | "tfoot"
                | "tr"
                | "td"
                | "th"
                | "dl"
                | "dt"
                | "dd"
                | "figure"
                | "figcaption"
        );
        if block {
            self.flush();
        }
        let toggle = |count: &mut usize| {
            if tag.closing {
                *count = count.saturating_sub(1);
            } else {
                *count += 1;
            }
        };
        match name {
            "b" | "strong" | "th" | "summary" | "dt" => toggle(&mut self.strong),
            "i" | "em" | "cite" => toggle(&mut self.italic),
            "code" | "kbd" | "tt" | "samp" => toggle(&mut self.code),
            "s" | "del" | "strike" => toggle(&mut self.strike),
            "pre" => toggle(&mut self.pre),
            _ => {}
        }
        match (name, tag.closing) {
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                self.heading = name[1..].parse().ok();
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => self.heading = None,
            ("br", _) => self.items.push(Inline::Break),
            ("hr", _) => {
                self.flush();
                self.lines.push(Line::Rule);
            }
            ("a", false) => self.links.push(tag.attr("href").map(str::to_string)),
            ("a", true) => {
                self.links.pop();
            }
            ("img", false) => {
                let size = |name: &str| {
                    tag.attr(name)
                        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
                };
                if let Some(src) = tag.attr("src") {
                    self.items.push(Inline::Image {
                        src: src.to_string(),
                        alt: tag.attr("alt").unwrap_or("").to_string(),
                        width: size("width"),
                        height: size("height"),
                        link: self.links.iter().rev().find_map(Clone::clone),
                    });
                }
            }
            ("ul", false) => self.lists.push(None),
            ("ol", false) => self.lists.push(Some(
                tag.attr("start").and_then(|s| s.parse().ok()).unwrap_or(1),
            )),
            ("ul" | "ol", true) => {
                self.lists.pop();
            }
            ("li", false) => {
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{}. ", *n - 1)
                    }
                    _ => "• ".into(),
                };
                self.text(&marker);
            }
            ("table" | "tr" | "td" | "th", closing) => {
                self.table_tag(name, closing);
            }
            _ => {}
        }
        if block && !tag.closing {
            let align = match tag.attr("align").map(str::to_ascii_lowercase).as_deref() {
                Some("center") => Some(Align::Center),
                Some("right") => Some(Align::Max),
                Some("left") => Some(Align::Min),
                _ if name == "center" => Some(Align::Center),
                _ => None,
            };
            if let Some(align) = align {
                self.aligns.push((tag.name.clone(), align));
            }
        } else if block
            && tag.closing
            && let Some(at) = self.aligns.iter().rposition(|(n, _)| *n == tag.name)
        {
            self.aligns.truncate(at);
        }
    }
}

/// The lines of an HTML block.
fn layout(html: &str) -> Vec<Line> {
    let mut builder = Builder::default();
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let tag = rest
            .strip_prefix('<')
            .filter(|r| r.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!'))
            .and_then(|_| rest.find('>'));
        match tag {
            Some(end) => {
                if let Some(tag) = parse_tag(&rest[..=end]) {
                    builder.tag(tag);
                }
                rest = &rest[end + 1..];
            }
            None => {
                // Past the first character, a `<` that opens no tag or a letter of any length.
                let first = rest.chars().next().map_or(1, char::len_utf8);
                let end = rest[first..].find('<').map_or(rest.len(), |i| i + first);
                builder.text(&rest[..end]);
                rest = &rest[end..];
            }
        }
    }
    // A table that was never closed is shown all the same.
    if builder.table.is_some() {
        if let Some(table) = &mut builder.table {
            table.nested = 0;
        }
        builder.table_tag("table", true);
    }
    builder.flush();
    builder.lines
}

/// The address of an image or a link relative to the document.
fn resolve(src: &str, base: &str) -> String {
    if src.contains("://") || src.starts_with("data:") || src.starts_with('#') {
        src.to_string()
    } else {
        format!("{base}{}", src.trim_start_matches("./"))
    }
}

/// Draws an HTML block of a Markdown document. `base` is the folder of the document (the base
/// of relative images), `max_width` the width of the widest image.
pub(super) fn show(ui: &mut egui::Ui, html: &str, base: &str, max_width: f32) {
    let lines = layout(html);
    if lines.is_empty() {
        return;
    }
    // The Markdown viewer writes its blocks in one wrapped row: the block takes the whole
    // width, under what is already written, and its lines go one under the other.
    if ui.cursor().left() > ui.max_rect().left() + 1.0 {
        ui.end_row();
    }
    let width = ui.max_rect().width();
    let page = Page {
        base,
        max_width: max_width.min(width),
        theme: ui::theme(ui.ctx()),
    };
    // What is remembered of a row goes by what the block says, not by where it is drawn: the
    // viewer does not give the same place its name from one frame to the next.
    let block = egui::Id::unique(("markdown_html", html));
    let top_down = egui::Layout::top_down(Align::Min);
    ui.allocate_ui_with_layout(vec2(width, 0.0), top_down, |ui| {
        ui.set_width(width);
        for (index, line) in lines.iter().enumerate() {
            match line {
                Line::Rule => {
                    ui.separator();
                }
                Line::Table { align, rows } => {
                    let id = block.with(("table", index));
                    aligned(ui, id, *align, 0.0, |ui| table(ui, id, rows, &page));
                }
                Line::Content {
                    align,
                    heading,
                    indent,
                    items,
                } => {
                    let size = match heading {
                        Some(1) => 28.0,
                        Some(2) => 22.0,
                        Some(3) => 18.0,
                        Some(4) => 16.0,
                        _ => 14.0,
                    };
                    let indent = *indent as f32 * 18.0;
                    // The rows of one line (`<br>`) are closer than two lines are.
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        for (row, pieces) in items.split(|i| *i == Inline::Break).enumerate() {
                            if pieces.is_empty() {
                                // Two breaks in a row: an empty row.
                                ui.add_space(size * 0.6);
                            } else if pieces.iter().all(|i| matches!(i, Inline::Text { .. })) {
                                text_row(ui, pieces, *align, size, indent, &page);
                            } else {
                                let id = block.with(("row", index, row));
                                aligned(ui, id, *align, indent, |ui| {
                                    pieces_row(ui, pieces, size, &page)
                                });
                            }
                        }
                    });
                }
            }
        }
    });
}

/// What every piece of a block is drawn with.
struct Page<'a> {
    base: &'a str,
    max_width: f32,
    theme: ui::Theme,
}

/// The font and colors of a piece of text.
fn text_format(ui: &egui::Ui, piece: &Inline, size: f32, page: &Page) -> TextFormat {
    let Inline::Text {
        strong,
        italic,
        code,
        strike,
        link,
        ..
    } = piece
    else {
        return TextFormat::default();
    };
    let font = if *code {
        FontId::monospace(size * 0.93)
    } else if *strong {
        ui::semibold(size)
    } else {
        FontId::proportional(size)
    };
    let color = if link.is_some() {
        page.theme.link
    } else {
        page.theme.text
    };
    let mut format = TextFormat::simple(font, color);
    format.italics = *italic;
    if *strike {
        format.strikethrough = Stroke::new(1.0, color);
    }
    if *code {
        format.background = ui.visuals().code_bg_color;
    }
    format
}

/// A row of text only, wrapped and aligned as one paragraph: each of its lines is centered (or
/// pushed to the right) by the layout of the text itself, so it is right at the first frame
/// and at any width. The links are found under the pointer.
fn text_row(
    ui: &mut egui::Ui,
    pieces: &[Inline],
    align: Align,
    size: f32,
    indent: f32,
    page: &Page,
) {
    let width = (ui.available_width() - indent).max(40.0);
    let mut job = LayoutJob {
        halign: align,
        ..Default::default()
    };
    job.wrap.max_width = width;
    // Where each link is, in characters.
    let mut links: Vec<(std::ops::Range<usize>, &str)> = Vec::new();
    let mut count = 0;
    for piece in pieces {
        let Inline::Text { text, link, .. } = piece else {
            continue;
        };
        job.append(text, 0.0, text_format(ui, piece, size, page));
        let end = count + text.chars().count();
        if let Some(href) = link {
            links.push((count..end, href));
        }
        count = end;
    }
    let galley = ui.painter().layout_job(job);
    let sense = if links.is_empty() {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width + indent, galley.size().y), sense);
    // The text is laid out around the point it is aligned on.
    let anchor = pos2(
        match align {
            Align::Min => rect.left() + indent,
            Align::Center => rect.left() + indent + width / 2.0,
            Align::Max => rect.left() + indent + width,
        },
        rect.top(),
    );
    ui.painter().galley(anchor, galley.clone(), page.theme.text);
    let Some(pointer) = response.hover_pos().filter(|_| !links.is_empty()) else {
        return;
    };
    // The character under the pointer: the one after the nearest gap, or the one before.
    let local = pointer - anchor;
    let cursor = galley.cursor_from_pos(local);
    let gap = galley.pos_from_cursor(cursor);
    if local.y < gap.top() || local.y > gap.bottom() || (local.x - gap.left()).abs() > size {
        return;
    }
    let index = if local.x < gap.left() {
        cursor.index.0.saturating_sub(1)
    } else {
        cursor.index.0
    };
    if let Some((_, href)) = links.iter().find(|(range, _)| range.contains(&index)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if response.clicked() {
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab(resolve(href, page.base)));
        }
    }
}

/// Draws `add` in a row, centered or pushed to the right from the width it had at the last
/// frame (a picture has no width before it is loaded). `add` gives the width of what it drew,
/// side by side: a row too long for the place wraps from the left, and stays there.
fn aligned(
    ui: &mut egui::Ui,
    id: egui::Id,
    align: Align,
    indent: f32,
    add: impl FnOnce(&mut egui::Ui) -> f32,
) {
    let known = ui.data(|d| d.get_temp::<f32>(id));
    let available = (ui.available_width() - indent).max(0.0);
    let offset = indent
        + match (align, known) {
            (Align::Center, Some(w)) => ((available - w) / 2.0).max(0.0),
            (Align::Max, Some(w)) => (available - w).max(0.0),
            _ => 0.0,
        };
    let measured = ui
        .horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(offset);
            add(ui)
        })
        .inner;
    if align != Align::Min && known.is_none_or(|w| (w - measured).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, measured));
        ui.ctx().request_repaint();
    }
}

/// The pieces of a row that has pictures, one after the other. Their width, side by side.
fn pieces_row(ui: &mut egui::Ui, pieces: &[Inline], size: f32, page: &Page) -> f32 {
    let mut total = 0.0;
    for piece in pieces {
        match piece {
            Inline::Break => ui.end_row(),
            Inline::Text { text, link, .. } => {
                let format = text_format(ui, piece, size, page);
                let mut rich = RichText::new(text)
                    .font(format.font_id.clone())
                    .color(format.color);
                if format.italics {
                    rich = rich.italics();
                }
                if format.strikethrough != Stroke::NONE {
                    rich = rich.strikethrough();
                }
                if format.background != egui::Color32::TRANSPARENT {
                    rich = rich.background_color(format.background);
                }
                let response = match link {
                    Some(href) => ui.hyperlink_to(rich, resolve(href, page.base)),
                    None => ui.label(rich),
                };
                total += response.rect.width();
            }
            Inline::Image {
                src,
                alt,
                width,
                height,
                link,
            } => {
                let uri = resolve(src, page.base);
                let mut image = egui::Image::new(uri).max_width(page.max_width);
                // `width` and `height` are the size of the picture, not only its limit: a small
                // one is enlarged. With one of them, the other follows the proportions.
                if width.is_some() || height.is_some() {
                    image = image.fit_to_exact_size(vec2(
                        width.unwrap_or(f32::INFINITY).min(page.max_width),
                        height.unwrap_or(f32::INFINITY),
                    ));
                }
                let failed = image.load_for_size(ui.ctx(), ui.available_size()).is_err();
                let response = if failed {
                    // A picture that cannot be loaded shows its text.
                    if alt.is_empty() {
                        continue;
                    }
                    ui.label(
                        RichText::new(alt)
                            .size(size)
                            .color(page.theme.text_secondary),
                    )
                } else {
                    ui.add(image.sense(Sense::click()))
                };
                total += response.rect.width();
                if let Some(href) = link {
                    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(resolve(href, page.base)));
                    }
                }
            }
        }
    }
    total
}

/// A table: its cells in columns, each as wide as its widest cell. Its width.
fn table(ui: &mut egui::Ui, id: egui::Id, rows: &[Vec<Vec<Inline>>], page: &Page) -> f32 {
    egui::Frame::new()
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            egui::Grid::new(id)
                .striped(true)
                .spacing(vec2(20.0, 8.0))
                .show(ui, |ui| {
                    for row in rows {
                        for cell in row {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                for pieces in cell.split(|i| *i == Inline::Break) {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.0;
                                        pieces_row(ui, pieces, 14.0, page);
                                    });
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
        })
        .response
        .rect
        .width()
}

#[cfg(test)]
#[path = "../../../../test/plugins/builtins/markdown/html/tests.rs"]
mod tests;
