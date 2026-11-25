use super::*;

fn project(files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "atelier-usages-{}-{}",
        std::process::id(),
        files.len() * 31 + files[0].1.len()
    ));
    let _ = std::fs::remove_dir_all(&root);
    for (name, text) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

fn places(found: &[Location]) -> Vec<String> {
    let mut out: Vec<String> = found
        .iter()
        .map(|l| {
            format!(
                "{}:{}:{}",
                l.path
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy(),
                l.line,
                l.column
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn a_default_export_is_found_in_every_file_that_imports_it() {
    let util = "export default function greet(name) { return name; }\n";
    let root = project(&[
        ("src/util.ts", util),
        ("src/a.ts", "import greet from './util';\ngreet('a');\n"),
        (
            "src/b.ts",
            "import hello from './util';\nhello('b');\nhello('c');\n",
        ),
        ("src/c.ts", "import other from './other';\nother();\n"),
        ("src/other.ts", "export default function other() {}\n"),
    ]);
    let path = root.join("src/util.ts");
    // On the keyword `default`, and on the name.
    for at in [
        util.find("default").unwrap() + 1,
        util.find("greet").unwrap() + 1,
    ] {
        let found = project_references(util, at, &path, &root);
        assert_eq!(
            places(&found),
            ["a.ts:0:7", "a.ts:1:0", "b.ts:0:7", "b.ts:1:0", "b.ts:2:0"],
            "{found:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn from_an_import_the_declaration_and_the_other_importers_are_found() {
    let a = "import { helper as h } from './lib';\nh();\n";
    let root = project(&[
        ("lib.js", "export function helper() {}\nhelper();\n"),
        ("a.js", a),
        (
            "b.js",
            "import * as lib from './lib';\nlib.helper();\nlib.other();\n",
        ),
        ("index.js", "export { helper as tool } from './lib';\n"),
        ("c.js", "import { tool } from './index';\ntool();\n"),
    ]);
    let found = project_references(a, a.find("h()").unwrap(), &root.join("a.js"), &root);
    assert_eq!(
        places(&found),
        [
            "b.js:1:4",
            "c.js:0:9",
            "c.js:1:0",
            "index.js:0:9",
            "lib.js:0:16",
            "lib.js:1:0"
        ],
        "{found:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_anonymous_default_export_is_found_from_export() {
    let map = "import { define } from './spec';\nexport default define({ id: 1 });\n";
    let root = project(&[
        ("maps/arena.ts", map),
        ("maps/spec.ts", "export function define(x) { return x; }\n"),
        (
            "maps/index.ts",
            "import arena from './arena';\nexport const all = [arena];\n",
        ),
    ]);
    let found = project_references(
        map,
        map.find("export").unwrap(),
        &root.join("maps/arena.ts"),
        &root,
    );
    assert_eq!(
        places(&found),
        ["index.ts:0:7", "index.ts:1:20"],
        "{found:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
