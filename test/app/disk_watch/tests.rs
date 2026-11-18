use super::*;
use std::fs;

#[test]
fn only_what_changed_is_reported() {
    let dir = std::env::temp_dir().join(format!("atelier-disk-watch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("same.txt"), "x").unwrap();
    fs::write(dir.join("other.txt"), "x").unwrap();
    let stamp = crate::project::modified;
    let old = SystemTime::UNIX_EPOCH;
    let ask = || Ask {
        files: vec![
            (dir.join("same.txt"), stamp(&dir.join("same.txt"))),
            (dir.join("other.txt"), Some(old)),
            // Deleted by another program: the document keeps its text.
            (dir.join("gone.txt"), Some(old)),
        ],
        folders: vec![
            (dir.join("sub"), Some(stamp(&dir.join("sub")))),
            (dir.clone(), Some(Some(old))),
            (dir.join("never-listed"), None),
            // Listed, then deleted.
            (dir.join("gone"), Some(Some(old))),
            (dir.join("still-gone"), Some(None)),
        ],
    };
    let expected = Changes {
        files: vec![dir.join("other.txt")],
        folders: vec![dir.clone(), dir.join("never-listed"), dir.join("gone")],
    };
    assert_eq!(changes(ask()), expected);

    // The same answer comes from the thread, once.
    let mut watch = Watch::default();
    assert!(!watch.busy());
    assert_eq!(watch.take(), None);
    watch.ask(ask(), || {});
    assert!(watch.busy());
    assert_eq!(watch.wait(), Some(expected));
    assert!(!watch.busy());
    fs::remove_dir_all(&dir).unwrap();
}
