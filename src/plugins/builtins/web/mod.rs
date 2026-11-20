//! Web viewer built-in plugin: web pages in the editor, as in a browser, with the web view of
//! the system. A page is a tab of the editor on a `.url` file (an Internet shortcut, as Windows
//! makes them), and an HTML file shows its page next to its source, reloaded when it is saved.

mod view;

use crate::{
    plugins::builtin::{BuiltinPlugin, Command, EditorLayout, Host},
    ui,
};
use eframe::egui::{self, Align, Layout, Order, Rect, RichText, Sense};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

/// What a web view tells, from its own callbacks.
#[derive(Default)]
pub struct Shared {
    /// The address of the page shown.
    pub url: String,
    pub title: String,
    pub loading: bool,
    /// Pages it asked to open in a new window: they open in new tabs.
    pub new_windows: Vec<String>,
}

struct Page {
    /// The address it opened at.
    start: String,
    /// The text of the address bar.
    address: String,
    shared: Rc<RefCell<Shared>>,
    view: Option<view::View>,
    error: Option<String>,
    /// Drawn this frame: else its web view is hidden, since egui does not draw it.
    drawn: bool,
    /// The address last written to its `.url` file.
    saved: String,
}

impl Page {
    fn new(start: String) -> Self {
        Self {
            address: start.clone(),
            saved: start.clone(),
            start,
            shared: Rc::default(),
            view: None,
            error: None,
            drawn: false,
        }
    }
}

#[derive(Default)]
pub struct WebPlugin {
    engine: view::Engine,
    pages: HashMap<PathBuf, Page>,
    /// The layout of each HTML file: the source alone until the page is asked for.
    layouts: HashMap<PathBuf, EditorLayout>,
    /// The floating windows and menus of the last frame: a web view over them would hide them.
    covers: Vec<Rect>,
}

pub fn is_shortcut(path: &Path) -> bool {
    extension_is(path, &["url"])
}

pub fn is_html(path: &Path) -> bool {
    extension_is(path, &["html", "htm", "xhtml"])
}

fn extension_is(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The address of an Internet shortcut (`URL=` of its `[InternetShortcut]` section).
pub fn read_shortcut(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let (key, value) = line.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("url")
            .then(|| value.trim().to_string())
    })
}

pub fn write_shortcut(url: &str) -> String {
    format!("[InternetShortcut]\r\nURL={url}\r\n")
}

/// What the address bar opens for what was typed: an address as it is, a site without its
/// scheme over HTTPS (HTTP for this computer), a file by its path, and words as a search.
pub fn address(input: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return "about:blank".into();
    }
    let lower = input.to_ascii_lowercase();
    if lower.contains("://")
        || ["about:", "data:", "file:", "mailto:", "javascript:"]
            .iter()
            .any(|scheme| lower.starts_with(scheme))
    {
        return input.into();
    }
    let path = Path::new(input);
    let drive = input.as_bytes().get(1) == Some(&b':') && input.as_bytes()[0].is_ascii_alphabetic();
    if drive || input.starts_with("\\\\") || (input.starts_with('/') && path.exists()) {
        return file_url(path);
    }
    if input.contains(char::is_whitespace) {
        return search(input);
    }
    let host = lower.split(['/', '?', '#']).next().unwrap_or_default();
    let name = host.rsplit_once(':').map_or(host, |(name, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            name
        } else {
            host
        }
    });
    let local = name == "localhost"
        || name.ends_with(".localhost")
        || name.starts_with('[')
        || (name.split('.').count() == 4 && name.split('.').all(|n| n.parse::<u8>().is_ok()));
    if local {
        format!("http://{input}")
    } else if name.contains('.') || name != host {
        format!("https://{input}")
    } else {
        search(input)
    }
}

fn search(words: &str) -> String {
    let mut query = String::new();
    for byte in words.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                query.push(byte as char)
            }
            b' ' => query.push('+'),
            _ => query.push_str(&format!("%{byte:02X}")),
        }
    }
    format!("https://duckduckgo.com/?q={query}")
}

/// The `file:` address of a file, without the `\\?\` of canonical Windows paths.
pub fn file_url(path: &Path) -> String {
    let path = crate::project::simplify(path);
    let text = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file://");
    // `\\server\share\a` is `file://server/share/a`.
    if let Some(share) = text.strip_prefix("//") {
        url.push_str(share);
        return encode_path(&url);
    }
    if !text.starts_with('/') {
        url.push('/');
    }
    url.push_str(&text);
    encode_path(&url)
}

fn encode_path(url: &str) -> String {
    let mut out = String::new();
    for c in url.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            '%' => out.push_str("%25"),
            _ => out.push(c),
        }
    }
    out
}

/// The folder of the pages opened from the palette or from a link: they are the IDE's, and
/// keep the address they are on.
fn pages_dir() -> PathBuf {
    crate::cli::data_dir().join("web").join("pages")
}

fn own_page(path: &Path) -> bool {
    path.parent()
        .is_some_and(|dir| crate::project::simplify(dir) == crate::project::simplify(&pages_dir()))
}

/// A new page of the IDE on `url`, named after the first free number.
fn new_page(url: &str) -> std::io::Result<PathBuf> {
    let dir = pages_dir();
    fs::create_dir_all(&dir)?;
    let name = tr!("web.page");
    let path = (1..)
        .map(|n| dir.join(format!("{name} {n}.url")))
        .find(|p| !p.exists())
        .unwrap_or_default();
    fs::write(&path, write_shortcut(url))?;
    Ok(path)
}

impl WebPlugin {
    fn open_page(&mut self, url: &str, host: &mut dyn Host) {
        match new_page(url) {
            Ok(path) => host.open_file(path),
            Err(e) => host.notify(trf!("error_detail", e = e)),
        }
    }

    /// The address bar and the buttons of a page.
    /// `switch`: the layout switch of the editor floats at the right end of the bar.
    fn toolbar(page: &mut Page, ui: &mut egui::Ui, switch: bool) {
        let (url, title, loading) = {
            let shared = page.shared.borrow();
            (shared.url.clone(), shared.title.clone(), shared.loading)
        };
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let view = page.view.as_ref();
            let back = view.is_some_and(|v| v.can_go_back());
            let forward = view.is_some_and(|v| v.can_go_forward());
            if ui::icon_button_enabled(ui, "arrowLeft", back, tr!("web.back")).clicked()
                && let Some(view) = view
            {
                view.back();
            }
            if ui::icon_button_enabled(ui, "arrowRight", forward, tr!("web.forward")).clicked()
                && let Some(view) = view
            {
                view.forward();
            }
            let (icon, tip) = match loading {
                true => ("stop", tr!("web.stop")),
                false => ("refresh", tr!("web.reload")),
            };
            if ui::icon_button(ui, icon, tip).clicked()
                && let Some(view) = view
            {
                match loading {
                    // Stopping is loading the page that shows again.
                    true => view.load(&url),
                    false => view.reload(),
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(match switch {
                    true => 3.0 * ui::BUTTON + 6.0 + 30.0,
                    false => 4.0,
                });
                if ui::icon_button(ui, "debug", tr!("web.devtools")).clicked()
                    && let Some(view) = view
                {
                    view.devtools();
                }
                if ui::icon_button(ui, "openNewTab", tr!("web.open_in_browser")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(&page.address));
                }
                let field = ui.add(
                    egui::TextEdit::singleline(&mut page.address)
                        .hint_text(tr!("web.address_hint"))
                        .desired_width(ui.available_width()),
                );
                let field = match title.is_empty() {
                    true => field,
                    false => field.on_hover_text(&title),
                };
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let target = address(&page.address);
                    page.address = target.clone();
                    if let Some(view) = &page.view {
                        view.load(&target);
                    }
                } else if !field.has_focus() && !url.is_empty() && page.address != url {
                    // Where the page went, by a link or the history.
                    page.address = url.clone();
                }
            });
        });
    }
}

impl BuiltinPlugin for WebPlugin {
    fn id(&self) -> &'static str {
        "web"
    }
    fn name(&self) -> &'static str {
        tr!("plugin.web.name")
    }
    fn description(&self) -> &'static str {
        tr!("plugin.web.summary")
    }
    fn icon(&self) -> &'static str {
        crate::plugins::builtin::card_icon("web")
    }
    fn extensions_dir(&self) -> Option<&'static str> {
        Some("web")
    }

    fn commands(&self) -> Vec<Command> {
        vec![
            Command {
                id: "web.open",
                label: tr!("web.open_page").into(),
            },
            Command {
                id: "web.split",
                label: tr!("web.html_page").into(),
            },
        ]
    }

    fn run_command(&mut self, id: &str, host: &mut dyn Host) {
        match id {
            "web.open" => self.open_page("about:blank", host),
            "web.split" => match host.active_file().filter(|p| is_html(p)) {
                Some(path) => self.set_editor_layout(&path, EditorLayout::Split),
                None => host.notify(tr!("web.open_html_file").into()),
            },
            _ => {}
        }
    }

    fn tree_commands(&self, paths: &[PathBuf]) -> Vec<Command> {
        match paths {
            [path] if is_html(path) => vec![Command {
                id: "web.tree_open",
                label: tr!("web.open_in_viewer").into(),
            }],
            _ => Vec::new(),
        }
    }

    fn run_tree_command(&mut self, id: &str, paths: &[PathBuf], host: &mut dyn Host) {
        if let ("web.tree_open", [path]) = (id, paths) {
            self.open_page(&file_url(path), host);
        }
    }

    fn update(&mut self, ctx: &egui::Context, host: &mut dyn Host) {
        self.covers = ctx.memory(|m| {
            m.areas()
                .visible_layer_ids()
                .into_iter()
                .filter(|layer| matches!(layer.order, Order::Middle | Order::Foreground))
                .filter_map(|layer| m.area_rect(layer.id))
                .collect()
        });
        // A click in the window: the keyboard comes back to it from the page.
        let pressed = ctx.input(|i| i.pointer.any_pressed());
        let mut opened = Vec::new();
        for (path, page) in &mut self.pages {
            if let Some(view) = &mut page.view {
                if !page.drawn {
                    view.hide();
                }
                if pressed {
                    view.release_keyboard();
                }
            }
            page.drawn = false;
            let mut shared = page.shared.borrow_mut();
            opened.append(&mut shared.new_windows);
            // A page of the IDE opens again where it was left.
            if own_page(path)
                && !shared.loading
                && !shared.url.is_empty()
                && shared.url != page.saved
            {
                page.saved = shared.url.clone();
                let _ = fs::write(path, write_shortcut(&page.saved));
            }
        }
        for url in opened {
            self.open_page(&url, host);
        }
    }

    fn file_saved(&mut self, path: &Path) {
        if let Some(view) = self.pages.get(path).and_then(|p| p.view.as_ref()) {
            view.reload();
        }
    }

    fn file_closed(&mut self, _ctx: &egui::Context, path: &Path) {
        self.pages.remove(path);
        self.layouts.remove(path);
        // A page of the IDE goes with its tab.
        if own_page(path) {
            let _ = fs::remove_file(path);
        }
    }

    fn opens_binary(&self, path: &Path) -> bool {
        is_shortcut(path)
    }

    fn editor_layout(&self, path: &Path) -> Option<EditorLayout> {
        if is_shortcut(path) {
            Some(EditorLayout::Preview)
        } else if is_html(path) {
            Some(
                self.layouts
                    .get(path)
                    .copied()
                    .unwrap_or(EditorLayout::Text),
            )
        } else {
            None
        }
    }

    fn layout_switch(&self, path: &Path) -> bool {
        !is_shortcut(path)
    }

    fn set_editor_layout(&mut self, path: &Path, layout: EditorLayout) {
        self.layouts.insert(path.to_path_buf(), layout);
    }

    fn preview(
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        _text: &mut String,
        _scroll: Option<f32>,
    ) {
        let t = ui::theme(ui.ctx());
        let page = self.pages.entry(path.to_path_buf()).or_insert_with(|| {
            let start = match is_shortcut(path) {
                true => fs::read_to_string(path)
                    .ok()
                    .and_then(|text| read_shortcut(&text))
                    .unwrap_or_else(|| "about:blank".into()),
                false => file_url(path),
            };
            Page::new(start)
        });
        page.drawn = true;
        ui.add_space(4.0);
        Self::toolbar(page, ui, !is_shortcut(path));
        ui.add_space(4.0);
        let (mut rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        // The web view is a child of the main window: a tab in a window of its own shows a
        // notice instead.
        let message = if ui.ctx().viewport_id() != egui::ViewportId::ROOT {
            if let Some(view) = &mut page.view {
                view.hide();
            }
            Some(tr!("web.main_window_only").to_string())
        } else {
            if page.view.is_none() && page.error.is_none() {
                match self
                    .engine
                    .create(&page.start, page.shared.clone(), ui.ctx())
                {
                    Ok(view) => page.view = Some(view),
                    Err(e) => page.error = Some(e),
                }
            }
            page.error.clone()
        };
        if let Some(message) = message {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(24.0)), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.label(RichText::new(message).color(t.text_secondary));
                    ui.add_space(8.0);
                    if ui::button(ui, tr!("web.open_in_browser")).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&page.address));
                    }
                });
            });
            return;
        }
        let Some(view) = &mut page.view else {
            return;
        };
        // What floats over the top of the page, as the layout switch of the editor: the page
        // starts under it. Anything else over it (a menu, the palette) hides it meanwhile.
        for cover in &self.covers {
            if cover.intersects(rect) && cover.top() <= rect.top() + 24.0 {
                rect.min.y = rect.min.y.max(cover.bottom() + 1.0);
            }
        }
        if rect.height() < 24.0 || self.covers.iter().any(|cover| cover.intersects(rect)) {
            view.hide();
        } else {
            let ppp = ui.ctx().pixels_per_point();
            let px = |v: f32| (v * ppp).round() as i32;
            view.place([
                px(rect.min.x),
                px(rect.min.y),
                px(rect.max.x) - px(rect.min.x),
                px(rect.max.y) - px(rect.min.y),
            ]);
        }
    }
}

#[cfg(test)]
#[path = "../../../../test/plugins/builtins/web/mod/tests.rs"]
mod tests;
