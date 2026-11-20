use super::*;

#[test]
fn fuzzy_matching_prefers_prefixes_and_word_starts() {
    let score = |q, l| fuzzy_score(q, l);
    assert!(score("con", "console").unwrap() > score("con", "decon").unwrap_or(i32::MIN));
    assert!(score("gEBI", "getElementById").is_some());
    assert!(score("gebi", "getElementById").is_some());
    assert!(score("xyz", "console").is_none());
    assert!(score("log", "console.log").is_some());
    assert_eq!(score("", "anything"), Some(0));
    assert!(score("fe", "forEach").unwrap() > score("fe", "buffer").unwrap());
}

#[test]
fn oneshot_is_read_once() {
    let slot = Oneshot::pending();
    assert!(slot.take().is_none());
    slot.clone().fulfill(3);
    assert_eq!(slot.take(), Some(3));
    assert_eq!(slot.take(), None);
    assert_eq!(Oneshot::ready("x").take(), Some("x"));
}

/// What a fake provider was given: (language, text) of each update, and the keys closed.
type Seen = Arc<Mutex<(Vec<(String, String)>, Vec<u64>)>>;

/// A provider that tells what it was given, for the routing of pages.
struct Fake {
    language: &'static str,
    seen: Seen,
    /// Where its one problem is.
    problem: Range<usize>,
}

impl Provider for Fake {
    fn handles(&self, language: &str) -> bool {
        language == self.language
    }
    fn update(&mut self, doc: &DocRef, text: &str, _version: u64) {
        let mut seen = self.seen.lock().unwrap();
        seen.0.push((doc.language.clone(), text.to_string()));
    }
    fn close(&mut self, key: u64) {
        self.seen.lock().unwrap().1.push(key);
    }
    fn diagnostics(&mut self, _key: u64) -> Arc<Vec<Diagnostic>> {
        Arc::new(vec![Diagnostic {
            range: self.problem.clone(),
            severity: Severity::Error,
            message: self.language.into(),
            source: String::new(),
        }])
    }
    fn complete(
        &mut self,
        doc: &DocRef,
        _text: &str,
        offset: usize,
        _explicit: bool,
    ) -> Oneshot<Option<Completion>> {
        Oneshot::ready(Some(Completion {
            from: offset,
            items: vec![CompletionItem::new(doc.language.clone(), ItemKind::Text)],
        }))
    }
    fn hover(&mut self, _doc: &DocRef, _text: &str, _offset: usize) -> Oneshot<Option<Hover>> {
        Oneshot::ready(None)
    }
    fn definition(&mut self, doc: &DocRef, text: &str, _offset: usize) -> Oneshot<Vec<Location>> {
        Oneshot::ready(vec![Location::in_text(doc.path.clone(), text, 0..1)])
    }
}

fn wait<T>(answer: Oneshot<T>) -> T {
    let started = std::time::Instant::now();
    loop {
        if let Some(value) = answer.take() {
            return value;
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn the_code_of_a_page_goes_to_the_provider_of_its_language() {
    let text = "<p>hi</p>\n<script>\nlet a = 1;\n</script>\n";
    let script = text.find("let").unwrap();
    let html_seen = Arc::default();
    let js_seen: Seen = Arc::default();
    let service = Service::default();
    service.add(Box::new(Fake {
        language: "html",
        seen: html_seen,
        problem: 0..1,
    }));
    service.add(Box::new(Fake {
        language: "js",
        seen: js_seen.clone(),
        // One problem in the script, one where the markup was.
        problem: script..script + 3,
    }));
    let doc = DocRef {
        key: 1,
        path: Some(PathBuf::from("site/index.html")),
        language: "html".into(),
    };
    service.update(&doc, text, 1);
    // The JavaScript provider got a document as long as the page, with the script only.
    {
        let seen = js_seen.lock().unwrap();
        let (language, code) = &seen.0[0];
        assert_eq!(language, "js");
        assert_eq!(code.len(), text.len());
        assert_eq!(&code[script..script + 10], "let a = 1;");
        assert!(code[..script].trim().is_empty());
    }
    let label = |offset: usize| {
        wait(service.complete(&doc, text, offset, true))
            .unwrap()
            .items[0]
            .label
            .clone()
    };
    assert_eq!(label(script + 2), "js");
    assert_eq!(label(1), "html");
    // A place in the document of the script is a place in the page.
    let found = wait(service.definition(&doc, text, script + 1));
    assert_eq!(found[0].path, None);
    // The problems of the page and of its script.
    let problems = service.diagnostics(&doc);
    let messages: Vec<&str> = problems.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, ["html", "js"]);
    // Without a script, its document is closed.
    service.update(&doc, "<p>hi</p>", 2);
    assert_eq!(js_seen.lock().unwrap().1.len(), 1);
}

/// What a fake linter of a plugin was given, and what it answers.
#[derive(Default)]
struct FakeLinter {
    documents: Mutex<Vec<(PathBuf, String, u64)>>,
    closed: Mutex<Vec<PathBuf>>,
    found: Mutex<Option<Arc<Vec<Diagnostic>>>>,
    /// Formats `.md` files: their text in capitals.
    formats_md: bool,
}

impl external::External for FakeLinter {
    fn lints(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "md")
    }
    fn formats(&self, path: &Path) -> bool {
        self.formats_md && self.lints(path)
    }
    fn document(&self, path: &Path, _language: &str, text: &str, version: u64) {
        self.documents
            .lock()
            .unwrap()
            .push((path.to_path_buf(), text.to_string(), version));
    }
    fn closed(&self, path: &Path) {
        self.closed.lock().unwrap().push(path.to_path_buf());
    }
    fn diagnostics(&self, _path: &Path) -> Option<Arc<Vec<Diagnostic>>> {
        self.found.lock().unwrap().clone()
    }
    fn format(
        &self,
        _path: &Path,
        _language: &str,
        text: &str,
    ) -> Option<Oneshot<Result<String, String>>> {
        Some(Oneshot::ready(Ok(text.to_uppercase())))
    }
}

#[test]
fn plugin_linters_read_documents_of_their_files_and_their_problems_show() {
    let service = Service::default();
    let linter = Arc::new(FakeLinter::default());
    service.set_externals(vec![linter.clone()]);
    let doc = DocRef {
        key: 1,
        path: Some(PathBuf::from("notes/README.md")),
        language: "markdown".into(),
    };
    // No provider handles Markdown: the plugin's linter makes the editor ask all the same.
    assert!(service.assists(&doc.path, &doc.language));
    assert!(!service.assists(&Some(PathBuf::from("a.txt")), "text"));
    service.update(&doc, "# Title", 1);
    service.update(&doc, "# Title", 1);
    service.update(&doc, "# Title\n", 2);
    assert_eq!(
        *linter.documents.lock().unwrap(),
        [
            ("notes/README.md".into(), "# Title".into(), 1),
            ("notes/README.md".into(), "# Title\n".into(), 2),
        ]
    );
    assert!(service.diagnostics(&doc).is_empty());
    let found = Diagnostic {
        range: 2..7,
        severity: Severity::Warning,
        message: "Title case".into(),
        source: "markdownlint".into(),
    };
    *linter.found.lock().unwrap() = Some(Arc::new(vec![found.clone()]));
    let first = service.diagnostics(&doc);
    assert_eq!(*first, [found]);
    // Nothing new: the same list, so the editor does not lay it out again.
    assert!(Arc::ptr_eq(&first, &service.diagnostics(&doc)));
    service.close(1);
    assert_eq!(
        *linter.closed.lock().unwrap(),
        [PathBuf::from("notes/README.md")]
    );
    // A file it does not cover, or a document without a file, stays out of it.
    let other = DocRef {
        key: 2,
        path: Some(PathBuf::from("a.txt")),
        language: "text".into(),
    };
    service.update(&other, "x", 1);
    service.update(
        &DocRef {
            key: 3,
            path: None,
            language: "markdown".into(),
        },
        "x",
        1,
    );
    assert_eq!(linter.documents.lock().unwrap().len(), 2);
}

#[test]
fn a_plugin_formatter_comes_first_and_the_ide_keeps_the_rest() {
    let service = Service::default();
    service.set_externals(vec![Arc::new(FakeLinter {
        formats_md: true,
        ..Default::default()
    })]);
    let doc = |path: &str, language: &str| DocRef {
        key: 1,
        path: Some(PathBuf::from(path)),
        language: language.into(),
    };
    let edits = wait(service.format(&doc("a.md", "markdown"), "hello\n")).unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "HELLO\n");
    // JSON is not the plugin's: the IDE formats it itself.
    let edits = wait(service.format(&doc("a.json", "json"), "{\"a\":1}")).unwrap();
    assert!(edits.iter().any(|e| e.new_text.contains("\"a\": 1")));
}
