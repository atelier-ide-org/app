//! The Web viewer for an AI, through the MCP server of the IDE: the `web_*` tools open pages,
//! read them, click, type, run scripts, read the console and take pictures of them.
//!
//! The MCP plugin hands their calls over ([`submit`]); the Web viewer answers them from its
//! own `update`, once the page has loaded or its script has answered, within the time the MCP
//! server waits.

use serde_json::{Value, json};
use std::{cell::RefCell, sync::mpsc::Sender};

/// A call of a `web_*` tool.
pub struct Request {
    pub tool: String,
    pub args: Value,
    pub reply: Sender<Result<String, String>>,
}

thread_local! {
    static QUEUE: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
}

/// Whether a tool is one of the Web viewer.
pub fn is_web_tool(name: &str) -> bool {
    name.starts_with("web_")
}

/// Hands a call over to the Web viewer, on the interface thread.
pub fn submit(tool: String, args: Value, reply: Sender<Result<String, String>>) {
    QUEUE.with_borrow_mut(|queue| queue.push(Request { tool, args, reply }));
}

pub fn take() -> Vec<Request> {
    QUEUE.with_borrow_mut(std::mem::take)
}

/// An answer of the MCP server that is a picture: the server sends it as an image.
pub const IMAGE_MARK: &str = "\u{1}image/png;base64,";

/// A picture and a line of text, as one answer.
pub fn image_answer(png: &[u8], text: &str) -> String {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    format!("{IMAGE_MARK}{data}\u{1}{text}")
}

/// The picture and the text of an answer made by [`image_answer`].
pub fn split_image(answer: &str) -> Option<(&str, &str)> {
    answer.strip_prefix(IMAGE_MARK)?.split_once('\u{1}')
}

/// The tools, as the `tools/list` answer describes them.
pub fn specs() -> Vec<Value> {
    let page = json!({"type": "string", "description": "The page: its id from web_list_pages. By default the page shown in the editor, else the last one used."});
    let target = |what: &str| {
        json!({
            "type": "object",
            "properties": {
                "page": page,
                "ref": {"type": "integer", "description": "The number of the element in the last web_snapshot."},
                "selector": {"type": "string", "description": "A CSS selector."},
                "text": {"type": "string", "description": "The visible text of the element (a link, a button, a label)."},
            },
            "description": what,
        })
    };
    let mut click = target("The element: ref, selector or text.");
    click["properties"]["waitForNavigation"] = json!({"type": "boolean", "description": "Waits for the page it opens, if any (true by default)."});
    let mut typing = target("The field: ref, selector or text.");
    typing["properties"]["value"] = json!({"type": "string", "description": "The text to type."});
    typing["properties"]["clear"] = json!({"type": "boolean", "description": "Replaces what the field holds (true by default)."});
    typing["properties"]["submit"] =
        json!({"type": "boolean", "description": "Presses Enter and submits the form afterwards."});
    typing["required"] = json!(["value"]);
    let mut scroll = target("The element to bring into view, or none to scroll the page.");
    scroll["properties"]["to"] = json!({"type": "string", "enum": ["top", "bottom"]});
    scroll["properties"]["by"] = json!({"type": "integer", "description": "Pixels down (negative: up); most of a screen by default."});
    vec![
        json!({
            "name": "web_open",
            "description": "Opens a web page in the Web viewer of the IDE (a tab of the editor, with the web engine of the system), and waits for it to load. An address without scheme goes over HTTPS (HTTP for localhost), a path opens the file. Answers the address and the title.",
            "inputSchema": {"type": "object", "properties": {
                "url": {"type": "string"},
                "newTab": {"type": "boolean", "description": "In a new tab (true by default); false loads it in the current page."},
                "page": page,
            }, "required": ["url"]},
        }),
        json!({
            "name": "web_list_pages",
            "description": "The pages open in the Web viewer: id, address, title, whether they load and whether they show.",
            "inputSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "name": "web_navigate",
            "description": "Back, forward or reload in a page of the Web viewer, or to an address, and waits for the page.",
            "inputSchema": {"type": "object", "properties": {
                "page": page,
                "action": {"type": "string", "enum": ["back", "forward", "reload"]},
                "url": {"type": "string"},
            }},
        }),
        json!({
            "name": "web_snapshot",
            "description": "What a page shows, for acting on it: title, address, scroll, headings, and its visible interactive elements numbered ([n] link \"text\" -> address, button, input…). The numbers are the `ref` of web_click, web_type and web_scroll, until the next snapshot.",
            "inputSchema": {"type": "object", "properties": {"page": page}},
        }),
        json!({
            "name": "web_read",
            "description": "The text of a page (or of an element), or its HTML.",
            "inputSchema": {"type": "object", "properties": {
                "page": page,
                "selector": {"type": "string", "description": "A CSS selector: that element only."},
                "format": {"type": "string", "enum": ["text", "html"], "description": "text by default."},
                "maxChars": {"type": "integer", "minimum": 100, "description": "20000 by default."},
            }},
        }),
        json!({
            "name": "web_click",
            "description": "Clicks an element of a page (found by ref, selector or text), then waits for the page it opens, if any.",
            "inputSchema": click,
        }),
        json!({
            "name": "web_type",
            "description": "Types a text in a field of a page (input, text area, select, editable element), as the page sees typing; can submit its form.",
            "inputSchema": typing,
        }),
        json!({
            "name": "web_scroll",
            "description": "Scrolls a page, or brings an element into view.",
            "inputSchema": scroll,
        }),
        json!({
            "name": "web_wait",
            "description": "Waits until a page has an element (selector) or a text, or has loaded.",
            "inputSchema": {"type": "object", "properties": {
                "page": page,
                "selector": {"type": "string"},
                "text": {"type": "string"},
                "timeoutMs": {"type": "integer", "minimum": 0, "maximum": 25000, "description": "10000 by default."},
            }},
        }),
        json!({
            "name": "web_eval",
            "description": "Runs JavaScript in a page and answers its value as JSON: an expression, or statements with `return`. `await` works.",
            "inputSchema": {"type": "object", "properties": {
                "page": page,
                "script": {"type": "string"},
            }, "required": ["script"]},
        }),
        json!({
            "name": "web_console",
            "description": "The console of a page: its logs, warnings and errors, and the uncaught errors, since it was opened (the last 500).",
            "inputSchema": {"type": "object", "properties": {
                "page": page,
                "clear": {"type": "boolean", "description": "Empties it afterwards."},
                "level": {"type": "string", "enum": ["log", "info", "warn", "error", "debug"], "description": "Only these."},
            }},
        }),
        json!({
            "name": "web_screenshot",
            "description": "A picture of a page as it shows in the IDE (the visible part). The page is brought to the front first.",
            "inputSchema": {"type": "object", "properties": {"page": page}},
        }),
    ]
}

/// Helpers of the scripts of the tools, defined in the page at each call.
const PRELUDE: &str = r#"
const A = {};
A.visible = el => {
  const r = el.getBoundingClientRect(), s = getComputedStyle(el);
  return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none';
};
A.label = el => (el.getAttribute('aria-label') || el.innerText || el.value || el.placeholder
  || el.title || el.name || el.alt || '').trim().replace(/\s+/g, ' ');
A.find = q => {
  if (q.ref != null) {
    const el = document.querySelector(`[data-atelier-ref="${q.ref}"]`);
    if (!el) throw new Error(`No element [${q.ref}]: take a new web_snapshot`);
    return el;
  }
  if (q.selector) {
    const el = document.querySelector(q.selector);
    if (!el) throw new Error(`No element matches ${q.selector}`);
    return el;
  }
  if (q.text) {
    const t = q.text.toLowerCase();
    const all = [...document.querySelectorAll('a,button,input,textarea,select,summary,label,option,[role],[onclick],[contenteditable=true]')]
      .filter(A.visible);
    const el = all.find(e => A.label(e).toLowerCase() === t)
      || all.find(e => A.label(e).toLowerCase().includes(t))
      || [...document.querySelectorAll('body *')].find(e => e.children.length === 0 && A.visible(e)
        && (e.innerText || '').toLowerCase().includes(t));
    if (!el) throw new Error(`No visible element with the text “${q.text}”`);
    return el;
  }
  throw new Error('Give ref, selector or text');
};
A.describe = el => {
  const label = A.label(el).slice(0, 80);
  return el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + (label ? ` “${label}”` : '');
};
A.snapshot = () => {
  document.querySelectorAll('[data-atelier-ref]').forEach(e => e.removeAttribute('data-atelier-ref'));
  const items = [];
  const interactive = 'a[href],button,input:not([type=hidden]),textarea,select,summary,[role=button],[role=link],[role=checkbox],[role=radio],[role=tab],[role=menuitem],[role=option],[role=switch],[contenteditable=true],[onclick]';
  let n = 0;
  for (const el of document.querySelectorAll(interactive)) {
    if (!A.visible(el)) continue;
    el.setAttribute('data-atelier-ref', ++n);
    const tag = el.tagName.toLowerCase();
    let line = `[${n}] ${el.getAttribute('role') || tag}`;
    if (tag === 'input') line += `[${el.type || 'text'}]`;
    const label = A.label(el).slice(0, 100);
    if (label) line += ` “${label}”`;
    if (tag === 'input' && el.value && !['checkbox', 'radio', 'submit', 'button'].includes(el.type)) line += ` = “${el.value.slice(0, 60)}”`;
    if (tag === 'a') line += ` -> ${el.href}`;
    if (el.checked) line += ' (checked)';
    if (el.disabled) line += ' (disabled)';
    items.push(line);
    if (n >= 400) { items.push('…'); break; }
  }
  const headings = [...document.querySelectorAll('h1,h2,h3')].filter(A.visible).slice(0, 40)
    .map(h => `${h.tagName.toLowerCase()}: ${A.label(h).slice(0, 100)}`);
  const end = Math.max(0, document.documentElement.scrollHeight - innerHeight);
  return `Title: ${document.title}\nURL: ${location.href}\nScroll: ${Math.round(scrollY)} of ${end}\n\nHeadings:\n${headings.join('\n') || '(none)'}\n\nInteractive elements:\n${items.join('\n') || '(none)'}`;
};
"#;

/// The script of a tool: `body` runs in an async function with the arguments as `args`, and
/// its value or its error comes back through the IPC of the page, tagged with `id`.
pub fn script(id: u64, args: &Value, body: &str) -> String {
    format!(
        r#"(() => {{
{PRELUDE}
const args = {args};
const send = (ok, value) => {{
  let message;
  try {{ message = JSON.stringify({{atelier: 'reply', id: {id}, ok, value: value === undefined ? null : value}}); }}
  catch (_) {{ message = JSON.stringify({{atelier: 'reply', id: {id}, ok, value: String(value)}}); }}
  window.ipc.postMessage(message);
}};
(async () => {{
{body}
}})().then(v => send(true, v), e => send(false, String(e && e.message || e)));
}})();"#
    )
}

pub const SNAPSHOT: &str = "return A.snapshot();";

pub const READ: &str = r#"
const root = args.selector ? A.find({selector: args.selector}) : (document.body || document.documentElement);
return args.format === 'html' ? root.outerHTML : root.innerText;"#;

pub const CLICK: &str = r#"
const el = A.find(args);
el.scrollIntoView({block: 'center', inline: 'center'});
const r = el.getBoundingClientRect();
const at = {bubbles: true, cancelable: true, view: window, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 0};
el.dispatchEvent(new PointerEvent('pointerdown', at));
el.dispatchEvent(new MouseEvent('mousedown', at));
if (el.focus) el.focus();
el.dispatchEvent(new PointerEvent('pointerup', at));
el.dispatchEvent(new MouseEvent('mouseup', at));
el.click();
return 'Clicked ' + A.describe(el);"#;

pub const TYPE: &str = r#"
const el = A.find(args);
el.scrollIntoView({block: 'center'});
el.focus();
const text = String(args.value ?? '');
if (el.isContentEditable) {
  if (args.clear !== false) document.execCommand('selectAll', false);
  document.execCommand('insertText', false, text);
} else {
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype
    : el instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
  const value = (args.clear === false ? el.value : '') + text;
  Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, value);
  el.dispatchEvent(new InputEvent('input', {bubbles: true, data: text, inputType: 'insertText'}));
  el.dispatchEvent(new Event('change', {bubbles: true}));
}
if (args.submit) {
  const key = {key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true, cancelable: true};
  const go = el.dispatchEvent(new KeyboardEvent('keydown', key));
  el.dispatchEvent(new KeyboardEvent('keypress', key));
  el.dispatchEvent(new KeyboardEvent('keyup', key));
  if (go && el.form) el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit();
}
return 'Typed in ' + A.describe(el);"#;

pub const SCROLL: &str = r#"
if (args.ref != null || args.selector || args.text) {
  const el = A.find(args);
  el.scrollIntoView({block: 'center'});
} else if (args.to === 'top') scrollTo(0, 0);
else if (args.to === 'bottom') scrollTo(0, document.documentElement.scrollHeight);
else scrollBy(0, args.by ?? Math.round(innerHeight * 0.8));
await new Promise(r => setTimeout(r, 100));
return `Scroll: ${Math.round(scrollY)} of ${Math.max(0, document.documentElement.scrollHeight - innerHeight)}`;"#;

pub const WAIT: &str = r#"
const end = Date.now() + Math.min(args.timeoutMs ?? 10000, 25000);
for (;;) {
  if (args.selector && document.querySelector(args.selector)) return 'Found ' + args.selector;
  if (args.text && document.body && document.body.innerText.includes(args.text)) return `Found “${args.text}”`;
  if (!args.selector && !args.text && document.readyState === 'complete') return 'Loaded';
  if (Date.now() > end) throw new Error('Timed out');
  await new Promise(r => setTimeout(r, 150));
}"#;

/// The body of `web_eval`: the script as it is when it returns, else its value.
pub fn eval_body(script: &str) -> String {
    let script = script.trim();
    let returns = script
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
        .any(|word| word == "return");
    if returns {
        script.to_string()
    } else {
        format!("return ({}\n);", script.trim_end_matches(';'))
    }
}

/// Run in every page: its console and uncaught errors go to the IDE, for `web_console`.
pub const CONSOLE: &str = r#"(() => {
  if (window.__atelierConsole) return;
  window.__atelierConsole = true;
  const show = a => {
    if (typeof a === 'string') return a;
    if (a instanceof Error) return a.stack || String(a);
    try { return JSON.stringify(a); } catch (_) { return String(a); }
  };
  const send = (level, items) => {
    try {
      window.ipc.postMessage(JSON.stringify({atelier: 'console', level, text: items.map(show).join(' ').slice(0, 4000)}));
    } catch (_) {}
  };
  for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
    const original = console[level];
    console[level] = function (...items) { send(level, items); return original.apply(this, items); };
  }
  addEventListener('error', e => send('error', [`${e.message} (${e.filename}:${e.lineno})`]));
  addEventListener('unhandledrejection', e => send('error', ['Unhandled rejection: ' + show(e.reason)]));
})();"#;
