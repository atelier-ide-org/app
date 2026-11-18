use super::*;
use std::fs;

#[test]
fn the_files_of_the_project_are_listed_without_its_dependencies() {
    let dir = std::env::temp_dir().join(format!("atelier-file-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for folder in [
        "src/deep",
        ".git/objects",
        "node_modules/x",
        "target/debug",
        ".github",
    ] {
        fs::create_dir_all(dir.join(folder)).unwrap();
    }
    for file in [
        "Cargo.toml",
        "src/main.rs",
        "src/deep/Été.rs",
        ".github/ci.yml",
        ".git/HEAD",
        "node_modules/x/index.js",
        "target/debug/app.exe",
    ] {
        fs::write(dir.join(file), "x").unwrap();
    }
    let mut found: Vec<String> = list(&dir)
        .iter()
        .map(|p| {
            p.strip_prefix(&dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            ".github/ci.yml",
            "Cargo.toml",
            "src/deep/Été.rs",
            "src/main.rs"
        ]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// Not a test of what is right: how long the list of this repository takes, the way it was
/// read before (each entry asked again for its kind) and now. `cargo test list_speed -- --ignored --nocapture`.
#[test]
#[ignore]
fn list_speed() {
    fn before(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(children) = project::searchable_children(&dir) else {
                continue;
            };
            for child in children {
                if child.is_dir() {
                    if !child.is_symlink() {
                        stack.push(child);
                    }
                } else {
                    files.push(child);
                }
            }
        }
        files
    }
    let root = std::env::var_os("ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    for round in 0..3 {
        let t = Instant::now();
        let old = before(&root).len();
        let a = t.elapsed();
        let t = Instant::now();
        let new = list(&root).len();
        println!(
            "round {round}: before {old} files in {a:?}, now {new} files in {:?}",
            t.elapsed()
        );
    }
}
