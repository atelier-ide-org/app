//! The answers of the Web viewer to the `web_*` tools of the MCP server ([`super::remote`]):
//! each call waits, if it must, for its page to show, to load or for its script to answer,
//! and is answered from `update`, frame after frame.

use super::{Page, WebPlugin, address, remote};
use crate::plugins::builtin::Host;
use eframe::egui;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    time::{Duration, Instant},
};

/// Under the 30 seconds the MCP server waits for an answer.
const DEADLINE: Duration = Duration::from_secs(27);
/// How long a click waits for the page it may open to start loading.
const SETTLE: Duration = Duration::from_millis(600);
/// The longest text answered by default.
const MAX_CHARS: usize = 20_000;
/// The console lines kept per page.
pub const CONSOLE_LINES: usize = 500;
/// The widest picture sent, in pixels: wider ones are shrunk.
const MAX_WIDTH: u32 = 1600;

pub struct Pending {
    page: PathBuf,
    reply: Sender<Result<String, String>>,
    deadline: Instant,
    since: Instant,
    step: Step,
}

enum Step {
    /// Waits for a page to load: more pages loaded than `after`, and none loading.
    Load { after: u64 },
    /// Waits for the page to be ready, then runs a script.
    Ready {
        args: Value,
        body: String,
        max: usize,
        settle: bool,
    },
    /// Waits for the answer of the script `id`.
    Script { id: u64, max: usize, settle: bool },
    /// After a click: waits for the page it opens, if it opens one.
    Settle {
        after: u64,
        since: Instant,
        text: String,
    },
    /// Waits for the page to show for a moment, then takes its picture.
    Shot { shown: Option<Instant> },
}

fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

fn flag(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn page_id(path: &Path) -> String {
    crate::project::simplify(path).display().to_string()
}

/// What a page is at: its address and its title.
fn state(page: &Page) -> String {
    let shared = page.shared.borrow();
    let mut out = format!("URL: {}\nTitle: {}", shared.url, shared.title);
    if shared.loading {
        out.push_str("\n(still loading)");
    }
    out
}

/// A text cut at `max` characters, saying so.
fn cut(text: String, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((at, _)) => format!(
            "{}\n… (cut at {max} characters of {})",
            &text[..at],
            text.chars().count()
        ),
        None => text,
    }
}

/// The answer of a script: a string as it is, anything else as JSON.
fn shown(value: Value) -> String {
    match value {
        Value::String(text) => text,
        Value::Null => "(no value)".into(),
        value => serde_json::to_string_pretty(&value).unwrap_or_default(),
    }
}

impl WebPlugin {
    /// The page a call is about: the one it names, else the one shown, else the last used.
    fn target(&self, args: &Value, host: &dyn Host) -> Result<PathBuf, String> {
        if let Some(id) = text(args, "page") {
            return self
                .pages
                .iter()
                .find(|(path, page)| {
                    page_id(path) == id
                        || path.file_name().is_some_and(|n| n.to_string_lossy() == id)
                        || page.shared.borrow().url == id
                })
                .map(|(path, _)| path.clone())
                .ok_or_else(|| format!("No page {id}: see web_list_pages"));
        }
        if let Some(active) = host.active_file().filter(|p| self.pages.contains_key(p)) {
            return Ok(active);
        }
        if let Some(last) = self.last.clone().filter(|p| self.pages.contains_key(p)) {
            return Ok(last);
        }
        match self.pages.keys().next() {
            Some(path) if self.pages.len() == 1 => Ok(path.clone()),
            _ if self.pages.is_empty() => Err("No web page is open: use web_open".into()),
            _ => Err("Several pages are open: name one (see web_list_pages)".into()),
        }
    }

    /// Takes the calls handed over by the MCP plugin.
    pub(super) fn take_calls(&mut self, host: &mut dyn Host) {
        for request in remote::take() {
            if let Err(e) = self.start(&request.tool, &request.args, &request.reply, host) {
                let _ = request.reply.send(Err(e));
            }
        }
    }

    fn wait(&mut self, page: PathBuf, reply: &Sender<Result<String, String>>, step: Step) {
        let now = Instant::now();
        self.last = Some(page.clone());
        self.pending.push(Pending {
            page,
            reply: reply.clone(),
            deadline: now + DEADLINE,
            since: now,
            step,
        });
    }

    fn start(
        &mut self,
        tool: &str,
        args: &Value,
        reply: &Sender<Result<String, String>>,
        host: &mut dyn Host,
    ) -> Result<(), String> {
        let answer = |text: String| {
            let _ = reply.send(Ok(text));
        };
        match tool {
            "web_list_pages" => {
                let active = host.active_file();
                let pages: Vec<Value> = self
                    .pages
                    .iter()
                    .map(|(path, page)| {
                        let shared = page.shared.borrow();
                        json!({
                            "id": page_id(path),
                            "url": shared.url,
                            "title": shared.title,
                            "loading": shared.loading,
                            "shown": page.view.as_ref().is_some_and(|v| v.placed().is_some()),
                            "active": active.as_deref() == Some(path.as_path()),
                        })
                    })
                    .collect();
                answer(serde_json::to_string_pretty(&pages).unwrap_or_default());
            }
            "web_open" => {
                let url = address(text(args, "url").ok_or("“url” is required")?);
                let current = match flag(args, "newTab", true) {
                    true => None,
                    false => self.target(args, host).ok(),
                };
                match current.and_then(|path| self.pages.get(&path).map(|p| (path, p))) {
                    Some((path, page)) if page.view.is_some() => {
                        let after = page.shared.borrow().loads;
                        if let Some(view) = &page.view {
                            view.load(&url);
                        }
                        host.open_file(path.clone());
                        self.wait(path, reply, Step::Load { after });
                    }
                    _ => {
                        let path = super::new_page(&url).map_err(|e| e.to_string())?;
                        // As the editor keeps it, and draws it under.
                        let path = path.canonicalize().unwrap_or(path);
                        host.open_file(path.clone());
                        self.wait(path, reply, Step::Load { after: 0 });
                    }
                }
            }
            "web_navigate" => {
                let path = self.target(args, host)?;
                let page = &self.pages[&path];
                let view = page.view.as_ref().ok_or("The page has not shown yet")?;
                let after = page.shared.borrow().loads;
                match (text(args, "action"), text(args, "url")) {
                    (_, Some(url)) => view.load(&address(url)),
                    (Some("back"), _) => view.back(),
                    (Some("forward"), _) => view.forward(),
                    (Some("reload"), _) => view.reload(),
                    _ => return Err("Give “action” (back, forward, reload) or “url”".into()),
                }
                self.wait(path, reply, Step::Load { after });
            }
            "web_console" => {
                let path = self.target(args, host)?;
                let page = self.pages.get_mut(&path).ok_or("The page is closed")?;
                let level = text(args, "level");
                let lines: Vec<String> = page
                    .console
                    .iter()
                    .filter(|(l, _)| level.is_none_or(|level| level == l))
                    .map(|(level, text)| format!("[{level}] {text}"))
                    .collect();
                if flag(args, "clear", false) {
                    page.console.clear();
                }
                self.last = Some(path);
                answer(match lines.is_empty() {
                    true => "(the console is empty)".into(),
                    false => cut(lines.join("\n"), MAX_CHARS),
                });
            }
            "web_screenshot" => {
                let path = self.target(args, host)?;
                host.open_file(path.clone());
                self.wait(path, reply, Step::Shot { shown: None });
            }
            _ => {
                let (body, settle) = match tool {
                    "web_snapshot" => (remote::SNAPSHOT.to_string(), false),
                    "web_read" => (remote::READ.to_string(), false),
                    "web_click" => (
                        remote::CLICK.to_string(),
                        flag(args, "waitForNavigation", true),
                    ),
                    "web_type" => (remote::TYPE.to_string(), flag(args, "submit", false)),
                    "web_scroll" => (remote::SCROLL.to_string(), false),
                    "web_wait" => (remote::WAIT.to_string(), false),
                    "web_eval" => (
                        remote::eval_body(text(args, "script").ok_or("“script” is required")?),
                        false,
                    ),
                    _ => return Err(format!("Unknown tool {tool}")),
                };
                let path = self.target(args, host)?;
                if self.pages[&path].view.is_none() {
                    host.open_file(path.clone());
                }
                let max = args
                    .get("maxChars")
                    .and_then(Value::as_u64)
                    .map_or(MAX_CHARS, |n| n.max(100) as usize);
                self.wait(
                    path,
                    reply,
                    Step::Ready {
                        args: args.clone(),
                        body,
                        max,
                        settle,
                    },
                );
            }
        }
        Ok(())
    }

    /// Moves the waiting calls on, with the answers of the scripts of this frame.
    pub(super) fn advance(&mut self, ctx: &egui::Context, replies: &mut Vec<(u64, bool, Value)>) {
        if self.pending.is_empty() {
            replies.clear();
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(50));
        let now = Instant::now();
        let mut kept = Vec::new();
        for mut call in std::mem::take(&mut self.pending) {
            let Some(page) = self.pages.get_mut(&call.page) else {
                // The page is not drawn yet (just opened), or its tab closed.
                if now > call.deadline || now - call.since > Duration::from_secs(5) {
                    let _ = call.reply.send(Err("The page did not show".into()));
                } else {
                    kept.push(call);
                }
                continue;
            };
            let (loading, loads) = {
                let shared = page.shared.borrow();
                (shared.loading, shared.loads)
            };
            let late = now > call.deadline;
            let outcome: Option<Result<String, String>> = match &mut call.step {
                Step::Load { after } => {
                    if page.view.is_some() && !loading && loads > *after {
                        Some(Ok(state(page)))
                    } else if late {
                        Some(Ok(format!("{}\n(timed out waiting)", state(page))))
                    } else {
                        None
                    }
                }
                Step::Ready {
                    args,
                    body,
                    max,
                    settle,
                } => {
                    let ready = page.view.is_some()
                        && !loading
                        && (loads > 0 || now - call.since > Duration::from_millis(1500));
                    if ready {
                        self.next_id += 1;
                        let id = self.next_id;
                        let script = remote::script(id, args, body);
                        match page.view.as_ref().map(|v| v.run(&script)) {
                            Some(Ok(())) => {
                                call.step = Step::Script {
                                    id,
                                    max: *max,
                                    settle: *settle,
                                };
                                None
                            }
                            Some(Err(e)) => Some(Err(e)),
                            None => None,
                        }
                    } else if late {
                        Some(Err(format!("The page is not ready\n{}", state(page))))
                    } else {
                        None
                    }
                }
                Step::Script { id, max, settle } => {
                    match replies.iter().position(|(r, _, _)| r == id) {
                        Some(at) => {
                            let (_, ok, value) = replies.remove(at);
                            let text = cut(shown(value), *max);
                            match (ok, *settle) {
                                (false, _) => Some(Err(text)),
                                (true, true) => {
                                    call.step = Step::Settle {
                                        after: loads,
                                        since: now,
                                        text,
                                    };
                                    None
                                }
                                (true, false) => Some(Ok(text)),
                            }
                        }
                        // A page that went elsewhere meanwhile drops the script.
                        None if late => Some(Err(format!(
                            "The script did not answer (the page may have changed)\n{}",
                            state(page)
                        ))),
                        None => None,
                    }
                }
                Step::Settle { after, since, text } => {
                    if loads > *after && !loading {
                        Some(Ok(format!("{text}\nThe page changed:\n{}", state(page))))
                    } else if (!loading && now - *since > SETTLE) || late {
                        Some(Ok(text.clone()))
                    } else {
                        None
                    }
                }
                Step::Shot { shown } => {
                    let placed = page.view.as_ref().and_then(|v| v.placed());
                    match (placed, *shown) {
                        (Some(_), None) => {
                            *shown = Some(now);
                            None
                        }
                        // A moment for the page to paint where it now is.
                        (Some(rect), Some(at))
                            if now - at > Duration::from_millis(250) && (!loading || late) =>
                        {
                            Some(picture(rect).map(|png| {
                                remote::image_answer(&png, &state(page).replace('\n', " · "))
                            }))
                        }
                        (None, _) if late => Some(Err(
                            "The page does not show: a menu or a window may cover it".into(),
                        )),
                        (None, _) => {
                            *shown = None;
                            None
                        }
                        _ => None,
                    }
                }
            };
            match outcome {
                Some(outcome) => {
                    let _ = call.reply.send(outcome);
                }
                None => kept.push(call),
            }
        }
        self.pending = kept;
        // Answers of scripts nobody waits for anymore.
        replies.clear();
    }
}

/// A picture of the part of the window where a page shows, as PNG.
fn picture(rect: [i32; 4]) -> Result<Vec<u8>, String> {
    let (width, height, pixels) =
        crate::window::capture_main(rect).ok_or("The system did not give the picture")?;
    let mut image = image::RgbaImage::from_raw(width, height, pixels).ok_or("Bad picture")?;
    if width > MAX_WIDTH {
        let h = (height as f32 * MAX_WIDTH as f32 / width as f32).round() as u32;
        image = image::imageops::resize(
            &image,
            MAX_WIDTH,
            h.max(1),
            image::imageops::FilterType::Triangle,
        );
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}
