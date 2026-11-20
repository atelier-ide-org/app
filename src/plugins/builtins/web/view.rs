//! The web view of the system, placed over the editor as a child of the main window: WebView2
//! on Windows, WKWebView on macOS. egui cannot draw it: it is a native window of its own, on
//! top of what egui paints, which the plugin moves, sizes and hides with the pane it fills.

use super::Shared;
use eframe::egui;
use std::{cell::RefCell, rc::Rc};

/// The browser engine: its profile (cookies, cache, storage) is the IDE's, in its data folder.
#[derive(Default)]
pub struct Engine {
    #[cfg(any(windows, target_os = "macos"))]
    context: Option<wry::WebContext>,
}

/// One page: a web view of its own, with its history.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub struct View {
    #[cfg(any(windows, target_os = "macos"))]
    web: wry::WebView,
    /// Where it was last put, in pixels of the window, and whether it shows.
    #[cfg(any(windows, target_os = "macos"))]
    placed: Option<[i32; 4]>,
}

impl Engine {
    /// A web view on `url`, hidden until it is placed. What it does (the address, the title,
    /// the pages it opens) goes to `shared`, and asks egui for a new frame.
    pub fn create(
        &mut self,
        url: &str,
        shared: Rc<RefCell<Shared>>,
        ctx: &egui::Context,
    ) -> Result<View, String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            let parent = crate::window::main_window().ok_or_else(|| tr!("web.unsupported"))?;
            let context = self.context.get_or_insert_with(|| {
                wry::WebContext::new(Some(crate::cli::data_dir().join("web").join("profile")))
            });
            let loads = (shared.clone(), ctx.clone());
            let titles = (shared.clone(), ctx.clone());
            let windows = (shared, ctx.clone());
            let web = wry::WebViewBuilder::new_with_web_context(context)
                .with_url(url)
                .with_visible(false)
                .with_focused(false)
                .with_devtools(true)
                .with_on_page_load_handler(move |event, url| {
                    let mut shared = loads.0.borrow_mut();
                    shared.loading = matches!(event, wry::PageLoadEvent::Started);
                    shared.url = url;
                    loads.1.request_repaint();
                })
                .with_document_title_changed_handler(move |title| {
                    titles.0.borrow_mut().title = title;
                    titles.1.request_repaint();
                })
                // A link to a new window opens in a new tab of the editor.
                .with_new_window_req_handler(move |url, _| {
                    windows.0.borrow_mut().new_windows.push(url);
                    windows.1.request_repaint();
                    wry::NewWindowResponse::Deny
                })
                .build_as_child(&parent)
                .map_err(|e| e.to_string())?;
            Ok(View { web, placed: None })
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (url, shared, ctx);
            Err(tr!("web.unsupported").into())
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl View {
    /// Shows the page over `rect`, in pixels of the window.
    pub fn place(&mut self, rect: [i32; 4]) {
        if self.placed == Some(rect) {
            return;
        }
        let [x, y, w, h] = rect;
        let _ = self.web.set_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(x, y).into(),
            size: wry::dpi::PhysicalSize::new(w.max(1) as u32, h.max(1) as u32).into(),
        });
        if self.placed.is_none() {
            let _ = self.web.set_visible(true);
        }
        self.placed = Some(rect);
    }
    pub fn hide(&mut self) {
        if self.placed.take().is_some() {
            let _ = self.web.set_visible(false);
        }
    }
    pub fn load(&self, url: &str) {
        let _ = self.web.load_url(url);
    }
    pub fn back(&self) {
        let _ = self.web.go_back();
    }
    pub fn forward(&self) {
        let _ = self.web.go_forward();
    }
    pub fn can_go_back(&self) -> bool {
        self.web.can_go_back().unwrap_or(false)
    }
    pub fn can_go_forward(&self) -> bool {
        self.web.can_go_forward().unwrap_or(false)
    }
    pub fn reload(&self) {
        let _ = self.web.reload();
    }
    pub fn devtools(&self) {
        if self.web.is_devtools_open() {
            self.web.close_devtools();
        } else {
            self.web.open_devtools();
        }
    }
    /// Gives the keyboard back to the window: the page keeps it once clicked, else.
    pub fn release_keyboard(&self) {
        let _ = self.web.focus_parent();
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
#[allow(dead_code)]
impl View {
    pub fn place(&mut self, _rect: [i32; 4]) {}
    pub fn hide(&mut self) {}
    pub fn load(&self, _url: &str) {}
    pub fn back(&self) {}
    pub fn forward(&self) {}
    pub fn can_go_back(&self) -> bool {
        false
    }
    pub fn can_go_forward(&self) -> bool {
        false
    }
    pub fn reload(&self) {}
    pub fn devtools(&self) {}
    pub fn release_keyboard(&self) {}
}
