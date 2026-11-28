//! Terminals in windows of their own: a terminal tab dragged out of the window, or sent there
//! from the menu of its tab, keeps running in a window of the IDE with its panes; the button of
//! that window, or closing it, puts the tab back in the Terminal panel. The panes of a tab are
//! drawn by [`draw_panes`], for the panel and for these windows alike.

use super::{Drag, Ide, Pane, TerminalTab, drop_zone};
use crate::{terminal_layout::Node, ui};
use eframe::egui::{self, Align2, FontId, Margin, Rect, Sense, Stroke, ViewportId, pos2, vec2};
use std::path::{Path, PathBuf};

/// What drawing the panes of a tab asked for, done once they are drawn.
#[derive(Default)]
pub(super) struct PanesOutput {
    /// AIs started in the panes, to be offered the MCP server.
    pub mcp_offers: Vec<(u64, crate::plugins::builtins::mcp::agents::Agent)>,
    /// Files clicked in the output.
    pub file_links: Vec<(PathBuf, Option<usize>, Option<usize>)>,
    /// The menu of the terminal area (right click).
    pub split: Option<bool>,
    pub close_pane: bool,
    pub close_tab: bool,
    /// Where the dragged tab or pane would land: (pane, stacked, before).
    pub drop: Option<(u64, bool, bool)>,
}

/// The panes of `tab`: each shell, the titles that move them, the dividers between them, and
/// where a dragged tab or pane would land. `salt` keeps the ids of two tabs apart.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_panes(
    ui: &mut egui::Ui,
    tab: &mut TerminalTab,
    drag: &mut Option<Drag>,
    t: &ui::Theme,
    root: &Path,
    ai_watch: bool,
    error: &str,
    salt: egui::Id,
) -> PanesOutput {
    let mut out = PanesOutput::default();
    egui::Frame::new()
        .inner_margin(Margin {
            left: 14,
            right: 10,
            top: 2,
            bottom: 8,
        })
        .show(ui, |ui| {
            if !error.is_empty() {
                ui.colored_label(t.error, error);
            }
            if tab.count() == 1 && tab.root.leaves().first().is_some_and(|p| p.term.exited) {
                ui.colored_label(t.warning, tr!("session_ended_restart_open_new"));
            }
            const GAP: f32 = 6.0;
            let full = ui.available_rect_before_wrap();
            let (rects, dividers) = tab.root.layout(full, GAP);
            let mut focused = None;
            let mut pane_drag = None;
            let split_view = tab.count() > 1;
            const HEAD: f32 = 22.0;
            let active_id = tab.active;
            for (pane, rect) in tab.root.leaves_mut().into_iter().zip(rects.iter()) {
                if !pane.term.has_wake() {
                    let ctx = ui.ctx().clone();
                    pane.term.on_output(move || ctx.request_repaint());
                }
                let mut body = *rect;
                if split_view {
                    // The title of a pane: a handle to drag it elsewhere.
                    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD));
                    body.min.y += HEAD;
                    let response = ui.interact(
                        head,
                        egui::Id::unique(("pane_head", pane.id)),
                        Sense::click_and_drag(),
                    );
                    ui.painter().rect_filled(
                        head,
                        0.0,
                        if pane.id == active_id {
                            t.hover
                        } else {
                            t.frame
                        },
                    );
                    if let Some(usage) = pane.term.pid().and_then(crate::stats::watch) {
                        ui.painter().text(
                            pos2(head.right() - 8.0, head.center().y),
                            Align2::RIGHT_CENTER,
                            crate::stats::format_usage(usage.0, usage.1),
                            FontId::proportional(11.0),
                            t.text_secondary,
                        );
                    }
                    ui.painter().text(
                        pos2(head.left() + 8.0, head.center().y),
                        Align2::LEFT_CENTER,
                        &pane.term.shell.name,
                        FontId::proportional(12.0),
                        if pane.id == active_id {
                            t.text
                        } else {
                            t.text_secondary
                        },
                    );
                    if response.clicked() {
                        focused = Some(pane.id);
                        pane.term.want_focus = true;
                    }
                    if response.dragged() && ui.input(|i| i.pointer.is_decidedly_dragging()) {
                        pane_drag = Some(pane.id);
                    }
                    response.on_hover_cursor(egui::CursorIcon::Grab);
                }
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
                pane.term.ai_watch = ai_watch;
                // A right click on a selection copies it instead of opening the menu.
                let had_selection = pane.selection.any();
                let area = crate::terminal_ui::show(
                    &mut child,
                    &mut pane.term,
                    &mut pane.selection,
                    t,
                    root,
                );
                if let Some(agent) = pane.term.pending_ai.take() {
                    out.mcp_offers.push((pane.id, agent));
                }
                if let Some(place) = pane.term.open_file.take() {
                    out.file_links.push(place);
                }
                if area.secondary_clicked() {
                    focused = Some(pane.id);
                }
                let copied = had_selection && area.secondary_clicked();
                if !copied {
                    area.context_menu(|ui| {
                        ui::menu_begin(ui);
                        if ui::menu_item(ui, None, tr!("paste"), Some("Ctrl+V"), false).clicked() {
                            // The clipboard comes back as a Paste event to the terminal,
                            // which has the keyboard.
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                            ui.close();
                        }
                        ui::menu_separator(ui);
                        if ui::menu_item(ui, Some("sideBySide"), tr!("split_right"), None, false)
                            .clicked()
                        {
                            out.split = Some(false);
                            ui.close();
                        }
                        if ui::menu_item(ui, Some("unified"), tr!("split_down"), None, false)
                            .clicked()
                        {
                            out.split = Some(true);
                            ui.close();
                        }
                        ui::menu_separator(ui);
                        if split_view
                            && ui::menu_item(ui, None, tr!("close_pane"), None, false).clicked()
                        {
                            out.close_pane = true;
                            ui.close();
                        }
                        if ui::menu_item(ui, Some("closeSmall"), tr!("close_tab"), None, false)
                            .clicked()
                        {
                            out.close_tab = true;
                            ui.close();
                        }
                    });
                }
                if pane.term.focused {
                    focused = Some(pane.id);
                }
            }
            if pane_drag.is_some() {
                *drag = pane_drag.map(Drag::Pane);
            }
            if let Some(id) = focused {
                tab.active = id;
            }
            // Dragging a divider moves the split. Interacting after the panes makes it easy to
            // grab.
            let mut moves = Vec::new();
            for (index, divider) in dividers.iter().enumerate() {
                if let Some(ratio) = divider.show(
                    ui,
                    salt.with(("terminal_divider", index)),
                    GAP,
                    t.separator,
                    t.accent,
                ) {
                    moves.push((divider.path.clone(), ratio));
                }
            }
            for (path, ratio) in moves {
                if let Some(slot) = tab.root.ratio_mut(&path) {
                    *slot = ratio;
                }
            }
            // A tab or a pane dragged over a pane shows where it would land.
            if let Some(dragged) = *drag
                && let Some(pointer) = ui.ctx().pointer_interact_pos()
                && let Some((pane, rect)) = tab
                    .root
                    .leaves()
                    .into_iter()
                    .zip(rects.iter())
                    .find(|(_, r)| r.contains(pointer))
                && match dragged {
                    // The tab of this very terminal dragged on its panes splits them with a new shell.
                    Drag::Tab(_) => true,
                    Drag::Pane(id) => id != pane.id,
                }
            {
                let (stacked, before, _) = drop_zone(*rect, pointer);
                super::groups::paint_drop(
                    ui.painter(),
                    t,
                    super::groups::Side::edge(*rect, pointer),
                    *rect,
                );
                out.drop = Some((pane.id, stacked, before));
            }
            ui.allocate_rect(full, Sense::hover());
        });
    out
}

/// A terminal tab in a window of its own.
pub(super) struct TerminalWindow {
    pub id: ViewportId,
    pub tab: TerminalTab,
    /// Where it opens on the screen; only for its first frame.
    position: Option<egui::Pos2>,
    /// What the mouse drags in it: a pane, onto another pane.
    drag: Option<Drag>,
}

/// What a terminal window asked of the IDE.
enum WindowAction {
    /// Back in the Terminal panel.
    Attach,
    Split(bool),
    ClosePane,
    Close,
}

impl Ide {
    /// Does what drawing the panes of a tab asked: of the active tab of the panel, or of the
    /// tab of terminal window `window`.
    pub(super) fn panes_output(&mut self, out: PanesOutput, window: Option<usize>) {
        for (pane, agent) in out.mcp_offers {
            self.offer_mcp(pane, agent);
        }
        for (path, line, column) in out.file_links {
            let (line, column) = (line.unwrap_or(1).max(1) - 1, column.unwrap_or(1).max(1) - 1);
            crate::plugins::builtin::Host::open_at(self, path, line, column, 0);
        }
        match window {
            None => {
                if let Some(stacked) = out.split {
                    self.split_terminal(stacked);
                }
                if out.close_pane {
                    self.close_pane();
                }
                if out.close_tab {
                    self.close_terminal(self.active_terminal);
                }
            }
            Some(w) => {
                if let Some(stacked) = out.split {
                    self.window_action(w, WindowAction::Split(stacked));
                } else if out.close_pane {
                    self.window_action(w, WindowAction::ClosePane);
                } else if out.close_tab {
                    self.window_action(w, WindowAction::Close);
                }
            }
        }
    }

    /// Terminal tab `index` in a window of its own, at `position` on the screen when known.
    pub(super) fn detach_terminal(&mut self, index: usize, position: Option<egui::Pos2>) {
        if index >= self.terminals.len() {
            return;
        }
        let tab = self.terminals.remove(index);
        self.window_count += 1;
        self.terminal_windows.push(TerminalWindow {
            id: ViewportId::from_hash_of(("terminal_window", self.window_count)),
            tab,
            position,
            drag: None,
        });
        if self.terminals.is_empty() {
            self.show_terminal = false;
        }
        self.active_terminal = self
            .active_terminal
            .min(self.terminals.len().saturating_sub(1));
    }

    /// The tab of terminal window `w` back in the Terminal panel, as its active tab.
    pub(super) fn attach_terminal(&mut self, w: usize) {
        if w >= self.terminal_windows.len() {
            return;
        }
        let mut window = self.terminal_windows.remove(w);
        if let Some(pane) = window.tab.pane() {
            pane.term.want_focus = true;
        }
        self.terminals.push(window.tab);
        self.active_terminal = self.terminals.len() - 1;
        self.set_terminal(true);
    }

    fn window_action(&mut self, w: usize, action: WindowAction) {
        match action {
            WindowAction::Attach => self.attach_terminal(w),
            WindowAction::Close => {
                // Its shells end with it.
                self.terminal_windows.remove(w);
            }
            WindowAction::ClosePane => {
                let tab = &mut self.terminal_windows[w].tab;
                if tab.count() < 2 {
                    self.terminal_windows.remove(w);
                    return;
                }
                let target = tab.active;
                tab.root.remove(&|p: &Pane| p.id == target);
                if let Some(first) = tab.root.leaves_mut().into_iter().next() {
                    first.term.want_focus = true;
                    tab.active = first.id;
                }
            }
            WindowAction::Split(stacked) => {
                let Some(shell) = self.terminal_windows[w]
                    .tab
                    .pane()
                    .map(|p| p.term.shell.clone())
                else {
                    return;
                };
                if let Some(pane) = self.new_pane(&shell) {
                    let tab = &mut self.terminal_windows[w].tab;
                    let (target, new) = (tab.active, pane.id);
                    if tab.root.split(&|p: &Pane| p.id == target, stacked, pane) {
                        tab.active = new;
                    }
                }
            }
        }
    }

    /// The terminal windows: a bar with the name of the tab and its buttons, then its panes.
    pub(super) fn terminal_windows(&mut self, ctx: &egui::Context) {
        let t = self.theme(ctx);
        let root = self.root.clone();
        let ai_watch = self.builtin_enabled("mcp");
        let mut actions: Vec<(usize, WindowAction)> = Vec::new();
        let mut outputs: Vec<(usize, PanesOutput)> = Vec::new();
        for (w, window) in self.terminal_windows.iter_mut().enumerate() {
            let name = window.tab.name().to_string();
            let mut builder = egui::ViewportBuilder::default()
                .with_title(format!("{name} – {} – Atelier IDE", tr!("terminal")))
                .with_inner_size([820.0, 520.0])
                .with_min_inner_size([320.0, 200.0]);
            if let Some(position) = window.position.take() {
                builder = builder.with_position(position);
            }
            let salt = egui::Id::unique(("terminal_window", window.id));
            ctx.show_viewport_immediate(window.id, builder, |ui, class| {
                let ctx = ui.ctx().clone();
                let mut draw = |ui: &mut egui::Ui| {
                    // The bar: the name of the tab, then its buttons on the right.
                    let bar =
                        Rect::from_min_size(ui.max_rect().min, vec2(ui.max_rect().width(), 34.0));
                    ui.painter().text(
                        pos2(bar.left() + 14.0, bar.center().y),
                        Align2::LEFT_CENTER,
                        &name,
                        FontId::proportional(13.0),
                        t.text,
                    );
                    let mut bar_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(bar.shrink2(vec2(8.0, 4.0)))
                            .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    );
                    if ui::icon_button(&mut bar_ui, "winRestore", tr!("terminal_back_to_panel"))
                        .clicked()
                    {
                        actions.push((w, WindowAction::Attach));
                    }
                    if ui::icon_button(&mut bar_ui, "sideBySide", tr!("split_right")).clicked() {
                        actions.push((w, WindowAction::Split(false)));
                    }
                    if ui::icon_button(&mut bar_ui, "unified", tr!("split_down")).clicked() {
                        actions.push((w, WindowAction::Split(true)));
                    }
                    ui.painter().line_segment(
                        [bar.left_bottom(), bar.right_bottom()],
                        Stroke::new(1.0_f32, t.separator),
                    );
                    let body =
                        Rect::from_min_max(pos2(bar.left(), bar.bottom() + 4.0), ui.max_rect().max);
                    let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body));
                    let out = draw_panes(
                        &mut body_ui,
                        &mut window.tab,
                        &mut window.drag,
                        &t,
                        &root,
                        ai_watch,
                        "",
                        salt,
                    );
                    // A pane dropped on another one of the window moves there.
                    if ctx.input(|i| i.pointer.any_released())
                        && let Some(Drag::Pane(id)) = window.drag.take()
                        && let Some((target, stacked, before)) = out.drop
                    {
                        move_pane_in(&mut window.tab, id, target, stacked, before);
                    } else if !ctx.input(|i| i.pointer.any_down()) {
                        window.drag = None;
                    }
                    outputs.push((w, out));
                };
                if class == egui::ViewportClass::EmbeddedWindow {
                    // A backend without windows of its own: a window inside the main one.
                    let mut open = true;
                    egui::Window::new(&name)
                        .id(salt)
                        .open(&mut open)
                        .default_size([700.0, 420.0])
                        .show(&ctx, |ui| draw(ui));
                    if !open {
                        actions.push((w, WindowAction::Attach));
                    }
                } else {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().fill(t.island))
                        .show(ui, |ui| draw(ui));
                    // Closing the window puts the terminal back: its shells keep running.
                    if ctx.input(|i| i.viewport().close_requested()) {
                        actions.push((w, WindowAction::Attach));
                    }
                }
            });
        }
        for (w, out) in outputs {
            self.panes_output(out, Some(w));
        }
        // The last windows first: removing one leaves the indexes of the others right.
        actions.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
        actions.dedup_by_key(|(w, _)| *w);
        for (w, action) in actions {
            if w < self.terminal_windows.len() {
                self.window_action(w, action);
            }
        }
    }
}

/// Moves pane `id` of `tab` next to pane `target` of the same tab.
pub(super) fn move_pane_in(
    tab: &mut TerminalTab,
    id: u64,
    target: u64,
    stacked: bool,
    before: bool,
) {
    if id == target || tab.count() < 2 {
        return;
    }
    if let Some(pane) = tab.root.take(&|p: &Pane| p.id == id) {
        tab.root.split_with(
            &|p: &Pane| p.id == target,
            stacked,
            before,
            Node::Leaf(pane),
        );
        tab.active = id;
        if let Some(pane) = tab.pane() {
            pane.term.want_focus = true;
        }
    }
}

#[cfg(test)]
#[path = "../../test/app/terminal_windows/tests.rs"]
mod tests;
