//! The linters and formatters of a native plugin, as the language service sees them
//! ([`External`]): the documents go to the plugin ([`protocol::DOCUMENT`]), its problems
//! ([`protocol::DIAGNOSTICS`]) and its formatted texts ([`protocol::FORMATTED`]) come back on
//! its own threads, into [`Published`].

use super::Instance;
use crate::languages::{Diagnostic, Oneshot, Severity, external::External, lsp};
use atelier_plugin_api::protocol;
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// The texts of a document kept to place the problems found in them: a plugin answers for a
/// version that may be a few edits old.
const KEPT_VERSIONS: usize = 4;

/// The problems of a file as the plugin sent them: the version of the document they were found
/// in, the list, and a number that changes with each list.
type Sent = (u64, Vec<protocol::Diagnostic>, u64);
/// The last texts given for a file, with their versions, the newest last.
type Kept = VecDeque<(u64, Arc<str>)>;
/// The problems of a file as the editor reads them, and the number of the list they come from.
type Placed = (u64, Arc<Vec<Diagnostic>>);

/// What the plugin sent back, from any thread.
#[derive(Default)]
pub(super) struct Published {
    diagnostics: Mutex<HashMap<PathBuf, Sent>>,
    lists: AtomicU64,
    /// The formatting the plugin took and has not answered yet, by request.
    formatting: Mutex<HashMap<u64, Oneshot<Result<String, String>>>>,
}

impl Published {
    pub(super) fn diagnostics(&self, found: protocol::Diagnostics) {
        let list = self.lists.fetch_add(1, Ordering::Relaxed) + 1;
        let mut diagnostics = self.diagnostics.lock().unwrap();
        if found.items.is_empty() {
            diagnostics.remove(&found.path);
        } else {
            diagnostics.insert(found.path, (found.version, found.items, list));
        }
    }

    pub(super) fn formatted(&self, answer: protocol::FormatAnswer) {
        if let Some(slot) = self.formatting.lock().unwrap().remove(&answer.id) {
            slot.fulfill(match answer.formatted {
                protocol::Formatted::Text(text) => Ok(text),
                protocol::Formatted::Error(e) => Err(e),
            });
        }
    }
}

pub(super) struct NativeTools {
    instance: Arc<Instance>,
    lints: Vec<String>,
    formats: Vec<String>,
    texts: Mutex<HashMap<PathBuf, Kept>>,
    placed: Mutex<HashMap<PathBuf, Placed>>,
    requests: AtomicU64,
}

impl NativeTools {
    pub(super) fn new(instance: Arc<Instance>, info: &protocol::Info) -> Self {
        Self {
            instance,
            lints: info.lints.clone(),
            formats: info.formats.clone(),
            texts: Mutex::default(),
            placed: Mutex::default(),
            requests: AtomicU64::new(1),
        }
    }

    fn published(&self) -> &Published {
        &self.instance.shared.published
    }
}

/// The problems of the plugin as byte ranges of `text`.
pub(super) fn place(text: &str, items: &[protocol::Diagnostic]) -> Vec<Diagnostic> {
    let mut placed: Vec<Diagnostic> = items
        .iter()
        .map(|d| {
            let start = lsp::from_position(text, d.line as usize, d.column as usize);
            let end = lsp::from_position(text, d.end_line as usize, d.end_column as usize);
            Diagnostic {
                range: start..end.max(start),
                severity: match d.severity {
                    protocol::Severity::Error => Severity::Error,
                    protocol::Severity::Warning => Severity::Warning,
                    protocol::Severity::Info => Severity::Info,
                    protocol::Severity::Hint => Severity::Hint,
                },
                message: match &d.code {
                    Some(code) if !code.is_empty() => format!("{} ({code})", d.message),
                    _ => d.message.clone(),
                },
                source: d.source.clone(),
            }
        })
        .collect();
    placed.sort_by_key(|d| (d.range.start, d.severity));
    placed
}

impl External for NativeTools {
    fn lints(&self, path: &Path) -> bool {
        protocol::Info::covers(&self.lints, path)
    }

    fn formats(&self, path: &Path) -> bool {
        protocol::Info::covers(&self.formats, path)
    }

    fn document(&self, path: &Path, language: &str, text: &str, version: u64) {
        {
            let mut texts = self.texts.lock().unwrap();
            let kept = texts.entry(path.to_path_buf()).or_default();
            kept.push_back((version, Arc::from(text)));
            while kept.len() > KEPT_VERSIONS {
                kept.pop_front();
            }
        }
        self.instance.call_json(
            protocol::DOCUMENT,
            &protocol::Document {
                path: path.to_path_buf(),
                language: language.to_string(),
                text: text.to_string(),
                version,
            },
        );
    }

    fn closed(&self, path: &Path) {
        self.texts.lock().unwrap().remove(path);
        self.placed.lock().unwrap().remove(path);
        self.published().diagnostics.lock().unwrap().remove(path);
    }

    fn diagnostics(&self, path: &Path) -> Option<Arc<Vec<Diagnostic>>> {
        let (version, items, list) = self
            .published()
            .diagnostics
            .lock()
            .unwrap()
            .get(path)
            .cloned()?;
        let mut placed = self.placed.lock().unwrap();
        if let Some((made_from, found)) = placed.get(path)
            && *made_from == list
        {
            return Some(found.clone());
        }
        // The text they were found in, else the newest one.
        let text = {
            let texts = self.texts.lock().unwrap();
            let kept = texts.get(path)?;
            kept.iter()
                .find(|(v, _)| *v == version)
                .or(kept.back())
                .map(|(_, text)| text.clone())?
        };
        let found = Arc::new(place(&text, &items));
        placed.insert(path.to_path_buf(), (list, found.clone()));
        Some(found)
    }

    fn format(
        &self,
        path: &Path,
        language: &str,
        text: &str,
    ) -> Option<Oneshot<Result<String, String>>> {
        let id = self.requests.fetch_add(1, Ordering::Relaxed);
        let answer = Oneshot::pending();
        // Ready before the call: the plugin may answer before it returns.
        self.published()
            .formatting
            .lock()
            .unwrap()
            .insert(id, answer.clone());
        let taken = self.instance.call_json(
            protocol::FORMAT,
            &protocol::FormatRequest {
                id,
                path: path.to_path_buf(),
                language: language.to_string(),
                text: text.to_string(),
            },
        );
        if serde_json::from_slice::<bool>(&taken).unwrap_or(false) {
            Some(answer)
        } else {
            self.published().formatting.lock().unwrap().remove(&id);
            None
        }
    }
}
