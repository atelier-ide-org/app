//! A diff in a tab of the editor, as IntelliJ shows one: the file before and after, each in a
//! read-only editor with its colors and line numbers, side by side and scrolling together, or
//! both in one editor (unified), the removed lines above the ones that replaced them. The lines
//! that changed have the background of their change.

use crate::{
    editor::{EditorState, Options},
    plugins::builtins::git::diff::{self, Row, RowKind},
    ui,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use std::ops::Range;

/// Width of the band between the two editors, where each change of the left links to its place
/// on the right.
const LINKS: f32 = 40.0;

/// A run of changed lines: where it is in each file (an empty range where a file has none of
/// it), and what changed.
#[derive(Clone, Debug, PartialEq)]
struct Block {
    old: Range<usize>,
    new: Range<usize>,
    kind: Kind,
}

/// What a diff tab shows: a file at two revisions.
pub struct DiffRequest {
    /// The path of the file, as the repository names it.
    pub path: String,
    /// The two revisions: a commit, HEAD, the working copy…
    pub left_title: String,
    pub right_title: String,
    pub old: String,
    pub new: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Deleted,
    Inserted,
    Changed,
}

/// One side of the side by side view.
struct Side {
    text: String,
    view: EditorState,
    /// The changed lines (0-based) and their change, sorted.
    kinds: Vec<(usize, Kind)>,
    /// The row of the alignment of each line.
    rows: Vec<usize>,
}

pub struct DiffTab {
    pub path: String,
    left_title: String,
    right_title: String,
    language: String,
    pub side_by_side: bool,
    /// Unified view: the unchanged lines far from a change are hidden.
    collapse: bool,
    binary: bool,
    old: Side,
    new: Side,
    /// For each row of the alignment of the two files, the line of each file there, or the
    /// line that comes next in that file when the row has none of it.
    at: Vec<(usize, usize)>,
    /// The rows where a change begins.
    starts: Vec<usize>,
    /// The changes, linked across the band between the two editors.
    blocks: Vec<Block>,
    unified: Unified,
    current: usize,
    /// The side the pointer was last over: the other one follows its scroll.
    leader: Option<bool>,
}

/// The unified view: one text of both files, its colors, and the line numbers of each line in
/// the two files.
#[derive(Default)]
struct Unified {
    text: String,
    view: EditorState,
    kinds: Vec<(usize, Kind)>,
    numbers: String,
    /// The lines where a change begins.
    starts: Vec<usize>,
}

impl DiffTab {
    pub fn new(request: DiffRequest) -> Self {
        let normalize = |t: &str| t.replace("\r\n", "\n");
        let (old, new) = (normalize(&request.old), normalize(&request.new));
        let binary = old.contains('\0') || new.contains('\0');
        let language = crate::project::language(std::path::Path::new(&request.path)).to_string();
        let rows = diff::side_by_side(&old, &new, None);
        let side = |text: String| Side {
            view: read_only_view(&text),
            text,
            kinds: Vec::new(),
            rows: Vec::new(),
        };
        let mut tab = Self {
            path: request.path,
            left_title: request.left_title,
            right_title: request.right_title,
            language,
            side_by_side: true,
            collapse: false,
            binary,
            old: side(old),
            new: side(new),
            at: Vec::with_capacity(rows.len()),
            starts: diff::change_starts(&rows),
            blocks: Vec::new(),
            unified: Unified::default(),
            current: 0,
            leader: None,
        };
        let (mut next_old, mut next_new) = (0, 0);
        for (r, row) in rows.iter().enumerate() {
            let kind = match row.kind {
                RowKind::Deleted => Some(Kind::Deleted),
                RowKind::Inserted => Some(Kind::Inserted),
                RowKind::Changed => Some(Kind::Changed),
                RowKind::Equal | RowKind::Fold => None,
            };
            tab.at.push((next_old, next_new));
            if let Some((number, _)) = &row.old {
                tab.old.rows.push(r);
                if let Some(kind) = kind {
                    tab.old.kinds.push((number - 1, kind));
                }
                next_old = *number;
            }
            if let Some((number, _)) = &row.new {
                tab.new.rows.push(r);
                if let Some(kind) = kind {
                    tab.new.kinds.push((number - 1, kind));
                }
                next_new = *number;
            }
        }
        tab.blocks = blocks(&rows, &tab.at, tab.old.rows.len(), tab.new.rows.len());
        tab.build_unified();
        // It opens on the first change, as IntelliJ's, in both views.
        tab.reveal_current();
        tab.side_by_side = false;
        tab.reveal_current();
        tab.side_by_side = true;
        // Debug builds: `ATELIER_SHOT_DIFF=unified` opens the unified view, for a screenshot.
        if cfg!(debug_assertions) && std::env::var("ATELIER_SHOT_DIFF").as_deref() == Ok("unified")
        {
            tab.side_by_side = false;
        }
        tab
    }

    /// The language of the file, for its colors.
    pub fn language(&self) -> &str {
        &self.language
    }

    /// The title of the tab: the file, and the revisions compared.
    pub fn title(&self) -> String {
        let name = self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path);
        format!("{name} ({} ↔ {})", self.left_title, self.right_title)
    }

    fn build_unified(&mut self) {
        let context = self.collapse.then_some(4);
        let rows = diff::unified(&self.old.text, &self.new.text, context);
        let mut unified = Unified {
            starts: diff::change_starts(&rows).into_iter().collect(),
            ..Unified::default()
        };
        let width = self
            .old
            .rows
            .len()
            .max(self.new.rows.len())
            .max(1)
            .to_string()
            .len();
        for (i, row) in rows.iter().enumerate() {
            let number = |side: &Option<(usize, String)>| {
                side.as_ref()
                    .map_or_else(|| " ".repeat(width), |(n, _)| format!("{n:>width$}"))
            };
            let (text, kind) = match row.kind {
                RowKind::Fold => (
                    format!("⋯ {}", trf!("diff_hidden_lines", count = row.hidden)),
                    None,
                ),
                RowKind::Deleted => (line_text(&row.old), Some(Kind::Deleted)),
                RowKind::Inserted => (line_text(&row.new), Some(Kind::Inserted)),
                RowKind::Changed | RowKind::Equal => (line_text(&row.new), None),
            };
            if i > 0 {
                unified.text.push('\n');
                unified.numbers.push('\n');
            }
            unified.text.push_str(&text);
            unified
                .numbers
                .push_str(&format!("{} {}", number(&row.old), number(&row.new)));
            if let Some(kind) = kind {
                unified.kinds.push((i, kind));
            }
        }
        unified.view = read_only_view(&unified.text);
        self.unified = unified;
    }

    fn change_count(&self) -> usize {
        if self.side_by_side {
            self.starts.len()
        } else {
            self.unified.starts.len()
        }
    }

    /// Shows the next change (or the previous one), three lines under the top of the view.
    fn step(&mut self, forward: bool) {
        let n = self.change_count();
        if n == 0 {
            return;
        }
        self.current = if forward {
            (self.current + 1) % n
        } else {
            (self.current + n - 1) % n
        };
        self.reveal_current();
    }

    fn reveal_current(&mut self) {
        let above = 3.0;
        if self.side_by_side {
            let Some(&row) = self.starts.get(self.current) else {
                return;
            };
            let (old, new) = self.at[row];
            self.old.view.scroll_top_to((old as f32 - above).max(0.0));
            self.new.view.scroll_top_to((new as f32 - above).max(0.0));
        } else if let Some(&line) = self.unified.starts.get(self.current) {
            self.unified
                .view
                .scroll_top_to((line as f32 - above).max(0.0));
        }
    }

    /// The line of the other file across from `line` (with its fraction) of this one.
    fn across(&self, from_old: bool, line: f32) -> f32 {
        let from = if from_old { &self.old } else { &self.new };
        let to = |at: (usize, usize)| if from_old { at.1 } else { at.0 };
        let whole = line.max(0.0) as usize;
        let row = from
            .rows
            .get(whole)
            .copied()
            .unwrap_or_else(|| self.at.len().saturating_sub(1));
        self.at.get(row).map_or(0.0, |&at| to(at) as f32) + line.fract()
    }

    pub fn show(&mut self, ui: &mut egui::Ui, t: &ui::Theme, font: FontId, highlight: bool) {
        self.toolbar(ui, t);
        if self.binary {
            ui.centered_and_justified(|ui| {
                ui.label(tr!("git.binary_file_content_cannot_compared"));
            });
            return;
        }
        let colors = |kinds: &[(usize, Kind)]| -> Vec<(usize, Color32)> {
            kinds
                .iter()
                .map(|(line, kind)| {
                    (
                        *line,
                        match kind {
                            Kind::Deleted => t.diff_deleted,
                            Kind::Inserted => t.diff_inserted,
                            Kind::Changed => t.diff_modified,
                        },
                    )
                })
                .collect()
        };
        let options = |line_colors, annotations| Options {
            language: &self.language,
            highlight,
            font: font.clone(),
            theme: *t,
            marks: &[],
            annotations,
            gutter_commands: &[],
            assist: None,
            search: &[],
            read_only: true,
            inlay_hints: false,
            sticky_lines: false,
            breadcrumbs: false,
            bookmarks: &[],
            spellcheck: false,
            line_colors,
            line_numbers: annotations.is_none(),
        };
        if !self.side_by_side {
            let colors = colors(&self.unified.kinds);
            let options = Options {
                language: &self.language,
                ..options(&colors, Some(&self.unified.numbers))
            };
            self.header(ui, t, None);
            let id = egui::Id::unique(("diff_unified", &self.path, &self.left_title));
            self.unified
                .view
                .show(ui, id, &mut self.unified.text, &options);
            return;
        }
        let area = ui.available_rect_before_wrap();
        let half = (area.width() - LINKS) / 2.0;
        let rects = [
            Rect::from_min_size(area.min, vec2(half, area.height())),
            Rect::from_min_max(pos2(area.left() + half + LINKS, area.top()), area.max),
        ];
        let band = Rect::from_min_max(
            pos2(rects[0].right(), area.top()),
            pos2(rects[1].left(), area.bottom()),
        );
        for x in [band.left() + 0.5, band.right() - 0.5] {
            ui.painter().line_segment(
                [pos2(x, area.top()), pos2(x, area.bottom())],
                Stroke::new(1.0_f32, t.separator),
            );
        }
        if let Some(p) = ui.ctx().pointer_hover_pos() {
            if rects[0].contains(p) {
                self.leader = Some(true);
            } else if rects[1].contains(p) {
                self.leader = Some(false);
            }
        }
        let (old_colors, new_colors) = (colors(&self.old.kinds), colors(&self.new.kinds));
        let mut scrolled = [false; 2];
        for (i, rect) in rects.into_iter().enumerate() {
            let left = i == 0;
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect);
            let title = if left {
                &self.left_title
            } else {
                &self.right_title
            };
            self.header(&mut child, t, Some(title.clone()));
            let colors = if left { &old_colors } else { &new_colors };
            let options = options(colors, None);
            let side = if left { &mut self.old } else { &mut self.new };
            let id = egui::Id::unique(("diff_side", &self.path, &self.left_title, left));
            let output = side.view.show(&mut child, id, &mut side.text, &options);
            scrolled[i] = output.scrolled;
        }
        self.paint_links(ui, t, band);
        // The side under the pointer leads; the other one keeps the same lines across.
        match self.leader {
            Some(true) if scrolled[0] => {
                let line = self.across(true, self.old.view.top_line);
                self.new.view.scroll_top_to(line);
                ui.ctx().request_repaint();
            }
            Some(false) if scrolled[1] => {
                let line = self.across(false, self.new.view.top_line);
                self.old.view.scroll_top_to(line);
                ui.ctx().request_repaint();
            }
            _ => {}
        }
    }

    /// The links of the changes across `band`, as IntelliJ draws them: a band of the color of
    /// the change from its lines on the left to its lines on the right, and a line in the editor
    /// that has none of it, where it would be.
    fn paint_links(&self, ui: &egui::Ui, t: &ui::Theme, band: Rect) {
        let (Some(left), Some(right)) = (self.old.view.text_rect(), self.new.view.text_rect())
        else {
            return;
        };
        let top = left.top().max(right.top());
        let bottom = left.bottom().min(right.bottom());
        let clip = Rect::from_min_max(pos2(band.left(), top), pos2(band.right(), bottom));
        let painter = ui.painter().with_clip_rect(clip);
        for block in &self.blocks {
            let (Some(l0), Some(l1), Some(r0), Some(r1)) = (
                self.old.view.line_y(block.old.start),
                self.old.view.line_y(block.old.end),
                self.new.view.line_y(block.new.start),
                self.new.view.line_y(block.new.end),
            ) else {
                continue;
            };
            if l1.max(r1) < top || l0.min(r0) > bottom {
                continue;
            }
            let (fill, edge) = block_colors(t, block.kind);
            let upper = curve(pos2(band.left(), l0), pos2(band.right(), r0));
            let lower = curve(pos2(band.left(), l1), pos2(band.right(), r1));
            painter.add(band_mesh(&upper, &lower, fill));
            let stroke = Stroke::new(1.0_f32, edge);
            painter.add(Shape::line(upper, stroke));
            painter.add(Shape::line(lower, stroke));
            // The side without the lines: where they would be, across its editor.
            for (lines, y, rect) in [(&block.old, l0, left), (&block.new, r0, right)] {
                if lines.is_empty() && y >= rect.top() && y <= rect.bottom() {
                    ui.painter()
                        .with_clip_rect(rect)
                        .line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
                }
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, t: &ui::Theme) {
        let mut rebuild = false;
        egui::Frame::new()
            .fill(t.frame)
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if ui::icon_button(ui, "up", tr!("git.previous_difference_shift_f7")).clicked()
                    {
                        self.step(false);
                    }
                    if ui::icon_button(ui, "down", tr!("git.next_difference_f7")).clicked() {
                        self.step(true);
                    }
                    let n = self.change_count();
                    let count = if n == 0 {
                        tr!("git.no_differences").to_string()
                    } else {
                        trf!(
                            "diff_position",
                            current = self.current.min(n - 1) + 1,
                            total = n
                        )
                    };
                    ui.label(egui::RichText::new(count).color(t.text_secondary));
                    ui.separator();
                    if ui::icon_toggle(ui, "sideBySide", self.side_by_side, tr!("git.side_by_side"))
                        .clicked()
                        && !self.side_by_side
                    {
                        self.side_by_side = true;
                        self.current = 0;
                    }
                    if ui::icon_toggle(ui, "unified", !self.side_by_side, tr!("git.unified"))
                        .clicked()
                        && self.side_by_side
                    {
                        self.side_by_side = false;
                        self.current = 0;
                    }
                    if !self.side_by_side {
                        ui.separator();
                        rebuild |=
                            ui::checkbox(ui, &mut self.collapse, tr!("git.hide_identical_lines"))
                                .changed();
                    }
                });
            });
        if rebuild {
            self.build_unified();
            self.current = 0;
        }
        // F7 and Shift+F7 go from change to change, as in IntelliJ.
        let (next, previous) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::F7),
                i.consume_key(egui::Modifiers::SHIFT, egui::Key::F7),
            )
        });
        if next {
            self.step(true);
        } else if previous {
            self.step(false);
        }
    }

    /// The bar over an editor: its revision, then the path of the file.
    fn header(&self, ui: &mut egui::Ui, t: &ui::Theme, revision: Option<String>) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, t.island);
        let revision =
            revision.unwrap_or_else(|| format!("{}  →  {}", self.left_title, self.right_title));
        let end = ui::paint_text(
            ui,
            rect,
            rect.left() + 10.0,
            &revision,
            t.text,
            FontId::proportional(12.5),
        );
        ui::paint_text(
            ui,
            rect,
            end + 10.0,
            &self.path,
            t.text_secondary,
            FontId::proportional(12.5),
        );
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, t.separator),
        );
    }
}

/// The runs of changed rows of `rows`, with the lines of each file they span: `at` gives the
/// line of each file at each row (or the next one), and the files have `old_len` and `new_len`
/// lines.
fn blocks(rows: &[Row], at: &[(usize, usize)], old_len: usize, new_len: usize) -> Vec<Block> {
    let line_at = |r: usize| at.get(r).copied().unwrap_or((old_len, new_len));
    let changed = |row: &Row| !matches!(row.kind, RowKind::Equal | RowKind::Fold);
    let mut out = Vec::new();
    let mut r = 0;
    while r < rows.len() {
        if !changed(&rows[r]) {
            r += 1;
            continue;
        }
        let start = r;
        let (mut deleted, mut inserted, mut replaced) = (false, false, false);
        while r < rows.len() && changed(&rows[r]) {
            match rows[r].kind {
                RowKind::Deleted => deleted = true,
                RowKind::Inserted => inserted = true,
                _ => replaced = true,
            }
            r += 1;
        }
        let ((o0, n0), (o1, n1)) = (line_at(start), line_at(r));
        let kind = match (deleted, inserted, replaced) {
            (true, false, false) => Kind::Deleted,
            (false, true, false) => Kind::Inserted,
            _ => Kind::Changed,
        };
        out.push(Block {
            old: o0..o1,
            new: n0..n1,
            kind,
        });
    }
    out
}

/// The background of a change and the color of its edges.
fn block_colors(t: &ui::Theme, kind: Kind) -> (Color32, Color32) {
    use crate::plugins::builtin::MarkKind;
    match kind {
        Kind::Deleted => (t.diff_deleted, MarkKind::Deleted.color()),
        Kind::Inserted => (t.diff_inserted, MarkKind::Added.color()),
        Kind::Changed => (t.diff_modified, MarkKind::Modified.color()),
    }
}

/// Points of an S curve from `a` to `b`, flat where it leaves and where it arrives.
fn curve(a: Pos2, b: Pos2) -> Vec<Pos2> {
    const STEPS: usize = 16;
    (0..=STEPS)
        .map(|i| {
            let s = i as f32 / STEPS as f32;
            let ease = s * s * (3.0 - 2.0 * s);
            pos2(a.x + (b.x - a.x) * s, a.y + (b.y - a.y) * ease)
        })
        .collect()
}

/// The surface between two curves of as many points, filled.
fn band_mesh(upper: &[Pos2], lower: &[Pos2], fill: Color32) -> Shape {
    let mut mesh = egui::Mesh::default();
    for (u, l) in upper.iter().zip(lower) {
        mesh.colored_vertex(*u, fill);
        mesh.colored_vertex(*l, fill);
    }
    for i in 0..upper.len().saturating_sub(1) as u32 {
        let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
        mesh.add_triangle(a, b, c);
        mesh.add_triangle(b, d, c);
    }
    Shape::mesh(mesh)
}

fn line_text(side: &Option<(usize, String)>) -> String {
    side.as_ref()
        .map(|(_, text)| text.clone())
        .unwrap_or_default()
}

/// An editor state with `text` indexed, the caret at its start.
fn read_only_view(text: &str) -> EditorState {
    let mut view = EditorState::default();
    view.sync(text);
    view
}

#[cfg(test)]
#[path = "../test/diff_tab/tests.rs"]
mod tests;
