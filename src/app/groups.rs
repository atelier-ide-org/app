//! Editor groups, as in IntelliJ: the editor splits right or down as many times as wanted, each
//! part with tabs of its own, and a tab dragged out of the window opens a window of its own,
//! where the editor splits again.
//!
//! The main group is the editor of the window: every open file is a tab there, with all that
//! the editor does. The other groups show some of those files, each with a view of its own
//! (caret, scroll, folds); the text is the document's, so what is typed in one shows in all.
//! Splitting copies the tab into the new group, as Split Right does in IntelliJ; dragging a tab
//! of another group moves it. Closing a tab of another group leaves the file open; closing the
//! file closes its tabs everywhere.

use super::{Ide, split::SplitPane};
use crate::terminal_layout::Node;
use crate::ui;
use eframe::egui::{
    self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, ViewportId, pos2, vec2,
};
use std::path::{Path, PathBuf};

/// The id of the main group, always in the main window.
pub const MAIN: u64 = 0;
/// Space between two groups, where the divider is dragged.
const GAP: f32 = 5.0;
/// The tab bar of a group: the height and the tabs of the one of the main group.
const BAR: f32 = 40.0;
const TAB: f32 = 28.0;

/// A part of the editor with tabs of its own.
pub struct Group {
    pub id: u64,
    pub tabs: Vec<SplitPane>,
    pub active: usize,
}

/// A window of its own with editor groups in it.
pub struct Window {
    pub id: ViewportId,
    pub layout: Node<u64>,
    /// Where it opens on the screen; only for its first frame.
    position: Option<egui::Pos2>,
}

/// What the mouse drags: a tab of the main group (by document) or of another group.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TabDrag {
    Main(usize),
    Group(u64, usize),
}

/// Where a dragged tab goes when it is dropped on a group.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The side of `rect` the pointer is on: its middle, or the nearest edge.
    pub fn at(rect: Rect, p: egui::Pos2) -> Side {
        let x = (p.x - rect.left()) / rect.width().max(1.0);
        let y = (p.y - rect.top()) / rect.height().max(1.0);
        if (0.3..=0.7).contains(&x) && (0.3..=0.7).contains(&y) {
            return Side::Center;
        }
        Side::edge(rect, p)
    }

    /// The edge of `rect` nearest to the pointer: where a terminal pane dropped there goes, as
    /// a tab dropped on the edge of a group of the editor does.
    pub fn edge(rect: Rect, p: egui::Pos2) -> Side {
        let x = (p.x - rect.left()) / rect.width().max(1.0);
        let y = (p.y - rect.top()) / rect.height().max(1.0);
        let edges = [
            (x, Side::Left),
            (1.0 - x, Side::Right),
            (y, Side::Top),
            (1.0 - y, Side::Bottom),
        ];
        edges
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(Side::Center, |(_, side)| side)
    }

    /// The part of `rect` the dropped tab will take.
    pub fn area(self, rect: Rect) -> Rect {
        let c = rect.center();
        match self {
            Side::Center => rect,
            Side::Left => Rect::from_min_max(rect.min, pos2(c.x, rect.bottom())),
            Side::Right => Rect::from_min_max(pos2(c.x, rect.top()), rect.max),
            Side::Top => Rect::from_min_max(rect.min, pos2(rect.right(), c.y)),
            Side::Bottom => Rect::from_min_max(pos2(rect.left(), c.y), rect.max),
        }
    }
}

impl Ide {
    fn new_group(&mut self, path: PathBuf, cursor: usize) -> u64 {
        self.group_count += 1;
        let id = self.group_count;
        self.groups.push(Group {
            id,
            tabs: vec![SplitPane::new(path, cursor)],
            active: 0,
        });
        id
    }

    fn group_mut(&mut self, id: u64) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| g.id == id)
    }

    /// The layout a group is in: the main window's, or that of a window of its own.
    fn layout_of(&mut self, group: u64) -> Option<&mut Node<u64>> {
        if group == MAIN || self.editor_layout.contains(&|g: &u64| *g == group) {
            return Some(&mut self.editor_layout);
        }
        self.windows
            .iter_mut()
            .find(|w| w.layout.contains(&|g: &u64| *g == group))
            .map(|w| &mut w.layout)
    }

    /// A new group beside `target` (or above, below…) showing `path`.
    pub(super) fn split_group(&mut self, target: u64, path: PathBuf, cursor: usize, side: Side) {
        let id = self.new_group(path, cursor);
        let (stacked, before) = match side {
            Side::Left => (false, true),
            Side::Top => (true, true),
            Side::Bottom => (true, false),
            Side::Right | Side::Center => (false, false),
        };
        let placed = self.layout_of(target).is_some_and(|layout| {
            layout.split_with(&|g: &u64| *g == target, stacked, before, Node::Leaf(id))
        });
        if !placed {
            let _ = self.editor_layout.split_with(
                &|g: &u64| *g == MAIN,
                stacked,
                before,
                Node::Leaf(id),
            );
        }
    }

    /// Split Right (or Down) of the menus: the file of tab `index` of the main group in a new
    /// group beside the main one.
    pub(super) fn split_doc(&mut self, index: usize, down: bool) {
        let Some(doc) = self.documents.get(index) else {
            return;
        };
        let Some(path) = doc.path.clone() else {
            self.status = tr!("split_needs_file").into();
            return;
        };
        let cursor = doc.view.cursor;
        let side = if down { Side::Bottom } else { Side::Right };
        self.split_group(MAIN, path, cursor, side);
    }

    /// Split Right (or Down) of the active file.
    pub(super) fn split_to(&mut self, down: bool) {
        self.split_doc(self.active, down);
    }

    /// Closes the groups other than the main one in the main window (Unsplit All).
    pub(super) fn close_splits(&mut self) {
        let ids: Vec<u64> = self
            .editor_layout
            .leaves()
            .into_iter()
            .copied()
            .filter(|g| *g != MAIN)
            .collect();
        self.editor_layout = Node::Leaf(MAIN);
        self.groups.retain(|g| !ids.contains(&g.id));
    }

    pub(super) fn has_splits(&self) -> bool {
        self.editor_layout.leaves().len() > 1
    }

    /// A window of its own with `path` in it, at `position` on the screen when known.
    pub(super) fn open_window_with(
        &mut self,
        path: PathBuf,
        cursor: usize,
        position: Option<egui::Pos2>,
    ) {
        let id = self.new_group(path, cursor);
        self.window_count += 1;
        self.windows.push(Window {
            id: ViewportId::from_hash_of(("editor_window", self.window_count)),
            layout: Node::Leaf(id),
            position,
        });
    }

    /// The window of its own `group` is in goes back into the main window, its groups as they
    /// were, to the right of the main group.
    pub(super) fn attach_window(&mut self, group: u64) {
        let Some(w) = self
            .windows
            .iter()
            .position(|w| w.layout.contains(&|g: &u64| *g == group))
        else {
            return;
        };
        let window = self.windows.remove(w);
        let ids: Vec<u64> = window.layout.leaves().into_iter().copied().collect();
        if !self
            .editor_layout
            .split_with(&|g: &u64| *g == MAIN, false, false, window.layout)
        {
            // No main group to go beside: its groups go, their files stay open.
            self.groups.retain(|g| !ids.contains(&g.id));
        }
    }

    /// Open in New Window of the menu of a tab of the main group.
    pub(super) fn detach(&mut self, index: usize, position: Option<egui::Pos2>) {
        let Some(doc) = self.documents.get(index) else {
            return;
        };
        let Some(path) = doc.path.clone() else {
            self.status = tr!("split_needs_file").into();
            return;
        };
        let cursor = doc.view.cursor;
        self.open_window_with(path, cursor, position);
    }

    /// Takes tab `index` out of group `id`. A group left without tabs goes away with the next
    /// [`Ide::prune_groups`], before anything is drawn: not here, which may run while its layout
    /// is being drawn, out of its place. Removed then, the group stayed in that layout as an
    /// empty island, and the window being drawn went away under its own feet: closing the last
    /// tab of a window of its own took the whole IDE down (index out of bounds).
    fn take_tab(&mut self, id: u64, index: usize) -> Option<SplitPane> {
        let group = self.group_mut(id)?;
        if index >= group.tabs.len() {
            return None;
        }
        let pane = group.tabs.remove(index);
        group.active = group.active.min(group.tabs.len().saturating_sub(1));
        Some(pane)
    }

    fn remove_group(&mut self, id: u64) {
        self.groups.retain(|g| g.id != id);
        if let Some(layout) = self.layout_of(id) {
            if matches!(layout, Node::Leaf(g) if *g == id) {
                *layout = Node::Empty;
            } else {
                layout.remove(&|g: &u64| *g == id);
            }
        }
        self.windows.retain(|w| !matches!(w.layout, Node::Empty));
    }

    /// Tabs whose file was closed go away, and the groups left without tabs.
    fn prune_groups(&mut self) {
        let open: Vec<PathBuf> = self
            .documents
            .iter()
            .filter_map(|d| d.path.clone())
            .collect();
        let mut empty = Vec::new();
        for group in &mut self.groups {
            group.tabs.retain(|t| open.contains(&t.path));
            group.active = group.active.min(group.tabs.len().saturating_sub(1));
            if group.tabs.is_empty() {
                empty.push(group.id);
            }
        }
        for id in empty {
            self.remove_group(id);
        }
    }

    /// Draws the groups of `layout` in `rect` (the main window's when `window` is None) and the
    /// dividers between them. Returns the place of the main group.
    pub(super) fn layout_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        window: Option<usize>,
    ) -> Option<Rect> {
        if window.is_none() {
            // Files closed since: their tabs go, and the groups left empty.
            self.prune_groups();
        }
        let layout = match window {
            None => std::mem::replace(&mut self.editor_layout, Node::Empty),
            Some(w) => std::mem::replace(&mut self.windows[w].layout, Node::Empty),
        };
        let mut layout = layout;
        let (rects, dividers) = layout.layout(rect, GAP);
        let ids: Vec<u64> = layout.leaves().into_iter().copied().collect();
        let viewport = ui.ctx().viewport_id();
        let mut main = None;
        let t = self.theme(ui.ctx());
        if ids.len() > 1 {
            // Each group is an island of its own, as the panels of the window are: rounded,
            // with the color of the window between them. The editor of the main window keeps
            // a strip under its groups for the rounding of its bottom corners: the groups
            // that reach the bottom take it.
            let radius = f32::from(super::ISLAND_RADIUS);
            let below = if window.is_none() { radius } else { 0.0 };
            let painter = ui.painter();
            painter.rect_filled(
                Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() + below)),
                0.0,
                t.frame,
            );
            for area in &rects {
                let mut island = *area;
                if (island.bottom() - rect.bottom()).abs() < 0.5 {
                    island.max.y += below;
                }
                painter.rect_filled(island, radius, t.island);
            }
        }
        // Where the window is on the screen: a tab dragged from another window finds its groups.
        if let Some(origin) = ui.ctx().input(|i| i.viewport().inner_rect.map(|r| r.min)) {
            self.viewport_origins.insert(viewport, origin);
        }
        let targets = self.drop_targets.entry(viewport).or_default();
        targets.clear();
        targets.extend(ids.iter().copied().zip(rects.iter().copied()));
        for (id, area) in ids.iter().zip(&rects) {
            if *id == MAIN {
                main = Some(*area);
            } else {
                self.group_ui(ui, *id, *area, window.is_some());
            }
        }
        // Dragging a divider changes the share of the two sides.
        let mut moves = Vec::new();
        for divider in &dividers {
            if let Some(ratio) = divider.show(
                ui,
                egui::Id::unique(("editor_divider", viewport, divider.path.clone())),
                GAP,
                Color32::TRANSPARENT,
                t.accent,
            ) {
                moves.push((divider.path.clone(), ratio));
            }
        }
        for (path, ratio) in moves {
            if let Some(slot) = layout.ratio_mut(&path) {
                *slot = ratio;
            }
        }
        match window {
            None => self.editor_layout = layout,
            Some(w) => self.windows[w].layout = layout,
        }
        main
    }

    /// A group other than the main one: its tabs, then the editor of the active one.
    fn group_ui(&mut self, ui: &mut egui::Ui, id: u64, rect: Rect, in_window: bool) {
        let t = self.theme(ui.ctx());
        let Some(gi) = self.groups.iter().position(|g| g.id == id) else {
            return;
        };
        let viewport = ui.ctx().viewport_id();
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        ui.set_clip_rect(rect);
        // The tabs, drawn as the ones of the main group are.
        let bar = Rect::from_min_size(rect.min, vec2(rect.width(), BAR));
        ui.painter().line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0_f32, t.separator),
        );
        let mut x = bar.left() + 6.0;
        let mut select = None;
        let mut close = None;
        let mut action: Option<(usize, GroupAction)> = None;
        let active = self.groups[gi].active;
        let titles: Vec<(PathBuf, String, bool)> = self.groups[gi]
            .tabs
            .iter()
            .map(|tab| {
                let doc = self
                    .documents
                    .iter()
                    .find(|d| d.path.as_ref() == Some(&tab.path));
                let title = doc.map_or_else(|| file_name(&tab.path), |d| d.title());
                (tab.path.clone(), title, doc.is_some_and(|d| d.dirty()))
            })
            .collect();
        for (i, (path, title, dirty)) in titles.iter().enumerate() {
            let galley =
                ui.painter()
                    .layout_no_wrap(title.clone(), FontId::proportional(13.0), t.text);
            let width = 10.0 + 16.0 + 6.0 + galley.size().x + 6.0 + 16.0 + 8.0;
            if x + width > bar.right() - 6.0 && i > active {
                break;
            }
            let tab = Rect::from_min_size(pos2(x, bar.center().y - TAB / 2.0), vec2(width, TAB));
            x += width + 2.0;
            let response = ui.interact(
                tab,
                egui::Id::unique(("group_tab", id, i)),
                Sense::click_and_drag(),
            );
            if i == active {
                ui.painter().rect(
                    tab,
                    6.0,
                    t.frame,
                    Stroke::new(1.0_f32, t.border),
                    StrokeKind::Inside,
                );
            } else if response.hovered() {
                ui.painter().rect_filled(tab, 6.0, t.hover);
            }
            let icon = Rect::from_min_size(
                pos2(tab.left() + 10.0, tab.center().y - 8.0),
                vec2(16.0, 16.0),
            );
            egui::Image::new(self.file_icon(ui.ctx(), path)).paint_at(&ui, icon);
            ui.painter().galley(
                pos2(icon.right() + 6.0, tab.center().y - galley.size().y / 2.0),
                galley,
                t.text,
            );
            let close_rect = Rect::from_min_size(
                pos2(tab.right() - 24.0, tab.center().y - 8.0),
                vec2(16.0, 16.0),
            );
            let close_response = ui.interact(
                close_rect,
                egui::Id::unique(("group_tab_close", id, i)),
                Sense::click(),
            );
            if *dirty && !response.hovered() && !close_response.hovered() {
                ui.painter()
                    .circle_filled(close_rect.center(), 3.5, t.text_secondary);
            } else if i == active || response.hovered() || close_response.hovered() {
                ui::paint_icon(
                    &ui,
                    if close_response.hovered() {
                        "closeSmallHovered"
                    } else {
                        "closeSmall"
                    },
                    close_rect,
                );
            }
            if close_response.clicked() || response.middle_clicked() {
                close = Some(i);
            } else if response.clicked() {
                select = Some(i);
            }
            if response.dragged() && ui.input(|inp| inp.pointer.is_decidedly_dragging()) {
                self.tab_drag = Some((TabDrag::Group(id, i), viewport));
            }
            let response =
                response.on_hover_text(crate::project::simplify(path).display().to_string());
            response.context_menu(|ui| {
                ui::menu_begin(ui);
                for (icon, label, side) in [
                    ("sideBySide", tr!("split_right"), Side::Right),
                    ("unified", tr!("split_down"), Side::Bottom),
                ] {
                    if ui::menu_item(ui, Some(icon), label, None, false).clicked() {
                        action = Some((i, GroupAction::Split(side)));
                        ui.close();
                    }
                }
                if ui::menu_item(
                    ui,
                    Some("openNewTab"),
                    tr!("open_in_new_window"),
                    None,
                    false,
                )
                .clicked()
                {
                    action = Some((i, GroupAction::Window));
                    ui.close();
                }
                if in_window
                    && ui::menu_item(
                        ui,
                        Some("winRestore"),
                        tr!("move_to_main_window"),
                        None,
                        false,
                    )
                    .clicked()
                {
                    action = Some((i, GroupAction::Attach));
                    ui.close();
                }
                ui::menu_separator(ui);
                if ui::menu_item(ui, Some("closeSmall"), tr!("close"), None, false).clicked() {
                    close = Some(i);
                    ui.close();
                }
            });
        }
        if let Some(i) = select {
            self.groups[gi].active = i;
        }
        // The editor of the active tab.
        let body = Rect::from_min_max(pos2(rect.left(), bar.bottom() + 1.0), rect.max);
        let active = self.groups[gi].active;
        let main_path = self.documents.get(self.active).and_then(|d| d.path.clone());
        let uncolored = self.uncolored_languages();
        let font = FontId::monospace(self.settings.font_size);
        if let Some(path) = self.groups[gi].tabs.get(active).map(|p| p.path.clone())
            && let Some(index) = self
                .documents
                .iter()
                .position(|d| d.path.as_ref() == Some(&path))
        {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            let highlight = !uncolored.contains(&self.documents[index].language.as_str());
            // The language service follows one view of a file: not the one of the main group.
            let service = (main_path.as_ref() != Some(&path)).then_some(&*self.languages);
            let pane = &mut self.groups[gi].tabs[active];
            let doc = &mut self.documents[index];
            super::split::show(
                &mut child,
                pane,
                doc,
                service,
                false,
                highlight,
                &self.settings,
                t,
                font,
            );
        }
        if let Some(i) = close {
            self.take_tab(id, i);
        } else if let Some((i, what)) = action
            && let Some(tab) = self.groups[gi].tabs.get(i)
        {
            let (path, cursor) = (tab.path.clone(), tab.view.cursor);
            match what {
                GroupAction::Split(side) => self.split_group(id, path, cursor, side),
                GroupAction::Window => {
                    self.take_tab(id, i);
                    self.open_window_with(path, cursor, None);
                }
                // Its window is being drawn: it goes back once all are.
                GroupAction::Attach => self.attach_requests.push(id),
            }
        }
    }

    /// Ends or shows the drag of a tab in the window of `viewport`: the side of the group under
    /// the pointer, in this window or in another window of the IDE the pointer is over, or a
    /// window of its own when the pointer is over none of them.
    pub(super) fn group_drag_ui(&mut self, ctx: &egui::Context) {
        let viewport = ctx.viewport_id();
        let t = self.theme(ctx);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique("group_drop"),
        ));
        let Some((drag, from)) = self.tab_drag else {
            return;
        };
        if from != viewport {
            // A tab dragged in another window over this one: where it would land here.
            if let Some((v, group, side)) = self.cross_drop
                && v == viewport
                && let Some(rect) = self.target_rect(viewport, group)
            {
                paint_drop(&painter, &t, side, rect);
            }
            return;
        }
        let latest = ctx.input(|i| i.pointer.latest_pos());
        let screen = ctx.content_rect();
        let outside = latest.is_some_and(|p| !screen.shrink(2.0).contains(p));
        // A tab of the main group dragged along its own bar is moved there, not dropped here.
        let over_bar = matches!(drag, TabDrag::Main(_))
            && self
                .main_bar
                .zip(latest)
                .is_some_and(|(b, p)| b.contains(p));
        let target = latest.filter(|_| !outside && !over_bar).and_then(|p| {
            self.drop_targets
                .get(&viewport)?
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(g, r)| (*g, *r, Side::at(*r, p)))
        });
        // Out of this window, over another one of the IDE: a group of that window.
        let origin = ctx.input(|i| i.viewport().inner_rect.map(|r| r.min));
        let cross = if outside {
            origin
                .zip(latest)
                .and_then(|(o, p)| self.window_target(o + p.to_vec2(), viewport))
        } else {
            None
        };
        if cross != self.cross_drop {
            for v in [cross, self.cross_drop].into_iter().flatten().map(|c| c.0) {
                ctx.request_repaint_of(v);
            }
            self.cross_drop = cross;
        }
        if let Some((group, rect, side)) = target
            && !stays(drag, group, side)
        {
            paint_drop(&painter, &t, side, rect);
        }
        if outside
            && cross.is_none()
            && let Some(p) = latest
        {
            let rect = Rect::from_min_size(
                p.clamp(screen.min + vec2(8.0, 8.0), screen.max - vec2(230.0, 40.0)),
                vec2(220.0, 30.0),
            );
            painter.rect_filled(rect, 6.0, t.accent);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                tr!("open_in_new_window"),
                ui::semibold(13.0),
                Color32::WHITE,
            );
        }
        let released = ctx.input(|i| i.pointer.any_released());
        if !released && ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        self.tab_drag = None;
        if let Some((v, _, _)) = self.cross_drop.take() {
            ctx.request_repaint_of(v);
        }
        if !released && !outside {
            return;
        }
        // What is dragged: its file and caret.
        let source = match drag {
            TabDrag::Main(i) => self
                .documents
                .get(i)
                .and_then(|d| Some((d.path.clone()?, d.view.cursor))),
            TabDrag::Group(g, i) => self
                .groups
                .iter()
                .find(|x| x.id == g)
                .and_then(|x| x.tabs.get(i))
                .map(|p| (p.path.clone(), p.view.cursor)),
        };
        let Some((path, cursor)) = source else { return };
        if let Some((_, group, side)) = cross {
            self.drop_tab(drag, path, cursor, group, side);
        } else if outside {
            let position = origin
                .zip(latest)
                .map(|(o, p)| o + p.to_vec2() - vec2(40.0, 20.0));
            if let TabDrag::Group(g, i) = drag {
                self.take_tab(g, i);
            }
            self.open_window_with(path, cursor, position);
        } else if let Some((group, _, side)) = target {
            self.drop_tab(drag, path, cursor, group, side);
        }
    }

    /// The tab `drag` (showing `path` at `cursor`) dropped on `side` of `group`, in any window.
    fn drop_tab(&mut self, drag: TabDrag, path: PathBuf, cursor: usize, group: u64, side: Side) {
        if stays(drag, group, side) {
            return;
        }
        // A tab of another group moves: it leaves its group.
        let take = |ide: &mut Ide| {
            if let TabDrag::Group(g, i) = drag {
                ide.take_tab(g, i);
            }
        };
        match (side, group) {
            (Side::Center, MAIN) => {
                take(self);
                if let Some(i) = self
                    .documents
                    .iter()
                    .position(|d| d.path.as_ref() == Some(&path))
                {
                    self.active = i;
                }
            }
            (Side::Center, g) => {
                take(self);
                if let Some(target) = self.group_mut(g) {
                    match target.tabs.iter().position(|p| p.path == path) {
                        Some(i) => target.active = i,
                        None => {
                            target.tabs.push(SplitPane::new(path, cursor));
                            target.active = target.tabs.len() - 1;
                        }
                    }
                }
            }
            (side, g) => {
                // A group alone in its own layout cannot be split by its only tab.
                if matches!(drag, TabDrag::Group(from, i) if from == g
                    && self.group_mut(g).is_some_and(|x| x.tabs.len() == 1 && i == 0))
                {
                    return;
                }
                take(self);
                self.split_group(g, path, cursor, side);
            }
        }
    }

    /// The group of a window of the IDE other than `except` under `point` (on the screen), and
    /// the side of it the point is on. The windows of their own first: they are over the main
    /// one.
    fn window_target(
        &self,
        point: egui::Pos2,
        except: ViewportId,
    ) -> Option<(ViewportId, u64, Side)> {
        let mut viewports: Vec<ViewportId> = self.windows.iter().map(|w| w.id).collect();
        viewports.push(ViewportId::ROOT);
        viewports
            .into_iter()
            .filter(|v| *v != except)
            .find_map(|v| {
                let origin = *self.viewport_origins.get(&v)?;
                let local = point - origin.to_vec2();
                self.drop_targets
                    .get(&v)?
                    .iter()
                    .find(|(_, r)| r.contains(local))
                    .map(|(g, r)| (v, *g, Side::at(*r, local)))
            })
    }

    /// Where group `group` is drawn in the window of `viewport`.
    fn target_rect(&self, viewport: ViewportId, group: u64) -> Option<Rect> {
        self.drop_targets
            .get(&viewport)?
            .iter()
            .find(|(g, _)| *g == group)
            .map(|(_, r)| *r)
    }

    /// The windows of their own, each with its groups.
    pub(super) fn editor_windows(&mut self, ctx: &egui::Context) {
        self.prune_groups();
        let t = self.theme(ctx);
        let mut closed = Vec::new();
        for w in 0..self.windows.len() {
            let id = self.windows[w].id;
            // Named after the file of its first group.
            let title = self.windows[w]
                .layout
                .leaves()
                .first()
                .and_then(|g| self.groups.iter().find(|x| x.id == **g))
                .and_then(|g| g.tabs.get(g.active))
                .map_or_else(
                    || "Atelier IDE".to_string(),
                    |p| format!("{} – Atelier IDE", file_name(&p.path)),
                );
            let mut builder = egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([900.0, 700.0])
                .with_min_inner_size([320.0, 200.0]);
            if let Some(position) = self.windows[w].position.take() {
                builder = builder.with_position(position);
            }
            let mut close = false;
            ctx.show_viewport_immediate(id, builder, |ui, class| {
                let ctx = &ui.ctx().clone();
                let draw = |ide: &mut Ide, ui: &mut egui::Ui| {
                    let rect = ui.max_rect();
                    ide.layout_ui(ui, rect, Some(w));
                };
                if class == egui::ViewportClass::EmbeddedWindow {
                    // A backend without windows of its own: a window inside the main one.
                    let mut open = true;
                    egui::Window::new("Atelier IDE")
                        .id(egui::Id::unique(("editor_window", w)))
                        .open(&mut open)
                        .default_size([700.0, 500.0])
                        .show(ctx, |ui| draw(self, ui));
                    close |= !open;
                } else {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().fill(t.island))
                        .show(ui, |ui| draw(self, ui));
                    self.group_drag_ui(ctx);
                    close |= ctx.input(|i| i.viewport().close_requested());
                }
            });
            if close {
                closed.push(w);
            }
        }
        for w in closed.into_iter().rev() {
            let window = self.windows.remove(w);
            let ids: Vec<u64> = window.layout.leaves().into_iter().copied().collect();
            self.groups.retain(|g| !ids.contains(&g.id));
        }
        for group in std::mem::take(&mut self.attach_requests) {
            self.attach_window(group);
        }
        // The places of the windows closed since: a tab dropped there finds nothing.
        let open: Vec<ViewportId> = self
            .windows
            .iter()
            .map(|w| w.id)
            .chain([ViewportId::ROOT])
            .collect();
        self.drop_targets.retain(|v, _| open.contains(v));
        self.viewport_origins.retain(|v, _| open.contains(v));
    }
}

/// Whether a tab dropped on `side` of `group` stays where it is: in the middle of its own group.
fn stays(drag: TabDrag, group: u64, side: Side) -> bool {
    side == Side::Center
        && match drag {
            TabDrag::Group(g, _) => g == group,
            TabDrag::Main(_) => group == MAIN,
        }
}

/// Shows where a dropped tab will go: the part of `rect` it takes, and what it does there.
pub(super) fn paint_drop(painter: &egui::Painter, t: &ui::Theme, side: Side, rect: Rect) {
    let area = side.area(rect).shrink(4.0);
    painter.rect(
        area,
        6.0,
        t.accent.gamma_multiply(0.18),
        Stroke::new(2.0_f32, t.accent),
        StrokeKind::Inside,
    );
    let label = match side {
        Side::Center => tr!("drop_here"),
        Side::Left => tr!("drop_split_left"),
        Side::Right => tr!("drop_split_right"),
        Side::Top => tr!("drop_split_up"),
        Side::Bottom => tr!("drop_split_down"),
    };
    let galley = painter.layout_no_wrap(label.into(), ui::semibold(13.0), Color32::WHITE);
    let chip = Rect::from_center_size(area.center(), galley.size() + vec2(20.0, 10.0));
    painter.rect_filled(chip, 6.0, t.accent);
    painter.galley(chip.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

enum GroupAction {
    Split(Side),
    Window,
    /// The whole window of the group back into the main window.
    Attach,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "../../test/app/groups/tests.rs"]
mod tests;
