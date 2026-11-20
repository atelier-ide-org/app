//! Linters and formatters that plugins bring (ESLint, Ruff, Prettier…), on top of the provider
//! of a language: they read the documents of the files they cover, whatever their language, and
//! their problems join the provider's. Their answers come later, from their own threads.

use super::{Diagnostic, Oneshot, QuickFix};
use std::{path::Path, sync::Arc};

pub trait External: Send + Sync {
    /// Whether it reads the documents of `path` to find problems in them.
    fn lints(&self, path: &Path) -> bool;
    /// Whether it may format `path` (it may still leave it to the IDE: see [`External::format`]).
    fn formats(&self, path: &Path) -> bool;
    /// The text of an open document, when it is seen first and after each change. Returns at
    /// once: the problems come later, through [`External::diagnostics`].
    fn document(&self, path: &Path, language: &str, text: &str, version: u64);
    /// The document of `path` closed.
    fn closed(&self, path: &Path);
    /// The problems found in the document of `path`, as byte ranges of its text; the same
    /// list (the same `Arc`) while nothing new was found.
    fn diagnostics(&self, path: &Path) -> Option<Arc<Vec<Diagnostic>>>;
    /// The fixes of the problems at the byte `offset` of the document of `path` (Alt+Enter),
    /// while its text is still the one they were found in.
    fn fixes(&self, _path: &Path, _offset: usize) -> Vec<QuickFix> {
        Vec::new()
    }
    /// The new text of the document, or why it could not be formatted; None when it leaves the
    /// document to the IDE's own formatter.
    fn format(
        &self,
        path: &Path,
        language: &str,
        text: &str,
    ) -> Option<Oneshot<Result<String, String>>>;
}
