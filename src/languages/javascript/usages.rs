//! Where an exported name is used in the other files of the project: the files that import it
//! (by default, by name or as a namespace), and the files that import it again from a file
//! that re-exports it. It answers without a language server, and with one when the server
//! does not know the whole project (no `tsconfig.json` or `jsconfig.json`: tsserver then only
//! knows the open files).

use super::{
    analysis::{Analysis, DeclKind, Dialect, use_dialect},
    navigate::{export_range, references, resolve_module},
    rename::project_files,
};
use crate::languages::Location;
use std::{
    collections::HashSet,
    ops::Range,
    path::{Path, PathBuf},
};
use tree_sitter::Node;

const MAX_TEXT: usize = 1_000_000;
/// Re-exports followed from a file to the next: `a.ts` → `index.ts` → `../index.ts`.
const MAX_TARGETS: usize = 32;
const MAX_PLACES: usize = 2000;

/// A path in one form, to compare paths of the analysis, of the server and of the file system.
pub(crate) fn path_key(path: &Path) -> String {
    let text = crate::project::simplify(path)
        .components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .replace('\\', "/");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// The places of the other files of the project that use what the name at `offset` of `text`
/// (the file `path`) stands for, when it is exported, or imported from a file of the project.
/// The places of `path` itself are not in it: the analysis of the document gives them.
pub fn project_references(text: &str, offset: usize, path: &Path, root: &Path) -> Vec<Location> {
    if text.len() > MAX_TEXT {
        return Vec::new();
    }
    let Some(analysis) = Analysis::new(text) else {
        return Vec::new();
    };
    let Some(dir) = path.parent() else {
        return Vec::new();
    };
    let mut targets: Vec<(PathBuf, String)> = Vec::new();
    let mut places: Vec<Location> = Vec::new();
    match subject(&analysis, offset) {
        Some(Subject::DefaultExport) => targets.push((path.to_path_buf(), "default".into())),
        Some(Subject::Local(name_range)) => {
            for name in exported_names(&analysis, &name_range) {
                targets.push((path.to_path_buf(), name));
            }
        }
        Some(Subject::Imported { source, imported }) => {
            if let Some(file) = resolve_module(dir, &source)
                .filter(|f| is_source(f))
                .map(|f| f.components().collect::<PathBuf>())
            {
                places.extend(declaration_places(&file, &imported));
                targets.push((file, imported));
            }
        }
        None => {}
    }
    if targets.is_empty() {
        return places;
    }
    let files: Vec<PathBuf> = project_files(root);
    let here = path_key(path);
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut index = 0;
    while index < targets.len() && index < MAX_TARGETS && places.len() < MAX_PLACES {
        let (target, name) = targets[index].clone();
        index += 1;
        let target_key = path_key(&target);
        if !seen.insert((target_key.clone(), name.clone())) {
            continue;
        }
        let hint = file_hint(&target);
        for file in &files {
            let key = path_key(file);
            if key == target_key {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(file) else {
                continue;
            };
            if content.len() > MAX_TEXT || !hint.iter().any(|h| content.contains(h.as_str())) {
                continue;
            }
            let found = uses_in(file, &content, &target_key, &name);
            if key != here {
                places.extend(found.places);
            }
            targets.extend(found.reexported.into_iter().map(|n| (file.clone(), n)));
        }
    }
    // The places in the document come from its own analysis.
    places.retain(|l| l.path.as_deref().is_none_or(|p| path_key(p) != here));
    let mut unique: HashSet<(String, usize, usize)> = HashSet::new();
    places.retain(|l| {
        unique.insert((
            l.path.as_deref().map(path_key).unwrap_or_default(),
            l.line,
            l.column,
        ))
    });
    places.truncate(MAX_PLACES);
    places
}

fn is_source(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "mts" | "cts"
        )
    }) && !path.components().any(|c| c.as_os_str() == "node_modules")
}

/// Words one of which every file that imports `target` contains: its name, and for an index
/// its folder's (`'../maps'`) or the current folder (`'.'`, `'./'`).
fn file_hint(target: &Path) -> Vec<String> {
    let Some(stem) = target.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    // `a.d.ts`, `a.test.ts`: the specifier names `a`.
    let stem = stem.split('.').next().unwrap_or(stem);
    let mut hints = vec![stem.to_string()];
    if stem == "index" {
        hints.extend(
            target
                .parent()
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .map(str::to_string),
        );
        hints.extend(["'.'", "\".\"", "'./'", "\"./\""].map(String::from));
    }
    hints
}

/// What the name under the caret is.
enum Subject {
    /// `export default …`, the caret on `export` or `default`.
    DefaultExport,
    /// A name declared in the module scope of the document.
    Local(Range<usize>),
    /// A name imported from another module.
    Imported { source: String, imported: String },
}

fn subject(analysis: &Analysis, offset: usize) -> Option<Subject> {
    let root = analysis.root();
    let at = root.descendant_for_byte_range(offset, offset)?;
    let before = root.descendant_for_byte_range(offset.saturating_sub(1), offset.saturating_sub(1));
    for node in [Some(at), before].into_iter().flatten() {
        let word = &analysis.text[node.byte_range()];
        if matches!(word, "default" | "export")
            && let Some(statement) = node.parent().filter(|p| p.kind() == "export_statement")
            && is_default_export(analysis, statement)
        {
            return Some(Subject::DefaultExport);
        }
    }
    let name = [Some(at), before].into_iter().flatten().find(|n| {
        matches!(
            n.kind(),
            "identifier"
                | "type_identifier"
                | "shorthand_property_identifier"
                | "shorthand_property_identifier_pattern"
        )
    })?;
    let decl = analysis.resolve(&analysis.text[name.byte_range()], name.start_byte())?;
    let module_scope = analysis
        .scopes
        .first()
        .is_some_and(|s| s.decls.iter().any(|d| d.name_range == decl.name_range));
    if !module_scope {
        return None;
    }
    match (&decl.kind, &decl.import) {
        (DeclKind::Import, Some((source, imported))) if imported != "*" => {
            Some(Subject::Imported {
                source: source.clone(),
                imported: imported.clone(),
            })
        }
        (DeclKind::Import, _) => None,
        _ => Some(Subject::Local(decl.name_range.clone())),
    }
}

pub(crate) fn is_default_export(analysis: &Analysis, statement: Node) -> bool {
    let mut cursor = statement.walk();
    statement
        .children(&mut cursor)
        .any(|c| !c.is_named() && &analysis.text[c.byte_range()] == "default")
}

fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// The names under which the module exports the declaration whose name is at `name_range`.
fn exported_names(analysis: &Analysis, name_range: &Range<usize>) -> Vec<String> {
    let text = &analysis.text;
    let name = &text[name_range.clone()];
    let mut names = Vec::new();
    let root = analysis.root();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if statement.kind() != "export_statement"
            || statement.child_by_field_name("source").is_some()
        {
            continue;
        }
        let default = is_default_export(analysis, statement);
        if let Some(declaration) = statement.child_by_field_name("declaration") {
            if contains(&declaration.byte_range(), name_range) {
                names.push(if default {
                    "default".into()
                } else {
                    name.into()
                });
            }
            continue;
        }
        if default {
            // `export default name;`
            if let Some(value) = statement.child_by_field_name("value")
                && value.kind() == "identifier"
                && &text[value.byte_range()] == name
            {
                names.push("default".into());
            }
            continue;
        }
        let mut inner = statement.walk();
        for clause in statement
            .named_children(&mut inner)
            .filter(|c| c.kind() == "export_clause")
        {
            let mut specifiers = clause.walk();
            for specifier in clause.named_children(&mut specifiers) {
                let Some(local) = specifier.child_by_field_name("name") else {
                    continue;
                };
                if &text[local.byte_range()] != name {
                    continue;
                }
                let exported = specifier.child_by_field_name("alias").unwrap_or(local);
                names.push(
                    text[exported.byte_range()]
                        .trim_matches(['"', '\''])
                        .to_string(),
                );
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The declaration of the export `name` of `file` and its uses there.
fn declaration_places(file: &Path, name: &str) -> Vec<Location> {
    let Ok(content) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    if content.len() > MAX_TEXT {
        return Vec::new();
    }
    let _dialect = use_dialect(Dialect::of(Some(file), ""));
    let Some(analysis) = Analysis::new(&content) else {
        return Vec::new();
    };
    let located =
        |range: Range<usize>| Location::in_text(Some(file.to_path_buf()), &content, range);
    let mut places = Vec::new();
    let local = if name == "default" {
        let Some(range) = export_range(&analysis, "default") else {
            return places;
        };
        places.push(located(range.clone()));
        // `export default function name` or `export default name`: the uses of the name too.
        let named = &content[range.clone()];
        (!named.starts_with("export"))
            .then(|| named.to_string())
            .or_else(|| default_value_name(&analysis))
    } else {
        local_name_of_export(&analysis, name)
    };
    if let Some(local) = local
        && let Some(decl) = analysis.scopes.first().and_then(|s| {
            s.decls
                .iter()
                .find(|d| d.name == local && d.kind != DeclKind::Import)
        })
    {
        places.push(located(decl.name_range.clone()));
        places.extend(
            references(&content, decl.name_range.start)
                .into_iter()
                .map(|l| Location {
                    path: Some(file.to_path_buf()),
                    ..l
                }),
        );
    }
    places
}

/// The declaration of the module scope that the module exports as `name` (`default` too),
/// when the export is a declaration of the module.
pub(crate) fn export_decl<'a>(
    analysis: &'a Analysis,
    name: &str,
) -> Option<&'a super::analysis::Decl> {
    let local = if name == "default" {
        let range = export_range(analysis, "default")?;
        let named = &analysis.text[range];
        if named.starts_with("export") {
            default_value_name(analysis)?
        } else {
            named.to_string()
        }
    } else {
        local_name_of_export(analysis, name)?
    };
    analysis
        .scopes
        .first()?
        .decls
        .iter()
        .find(|d| d.name == local && d.kind != DeclKind::Import)
}

/// The value of `export default <expression>;`.
pub(crate) fn default_value<'a>(analysis: &'a Analysis) -> Option<Node<'a>> {
    let root = analysis.root();
    let mut cursor = root.walk();
    let statement = root
        .named_children(&mut cursor)
        .find(|s| s.kind() == "export_statement" && is_default_export(analysis, *s))?;
    statement.child_by_field_name("value")
}

/// `export default name;`: the name.
fn default_value_name(analysis: &Analysis) -> Option<String> {
    let root = analysis.root();
    let mut cursor = root.walk();
    let statement = root
        .named_children(&mut cursor)
        .find(|s| s.kind() == "export_statement" && is_default_export(analysis, *s))?;
    let value = statement
        .child_by_field_name("value")
        .filter(|v| v.kind() == "identifier")?;
    Some(analysis.text[value.byte_range()].to_string())
}

/// The local name the module exports as `exported`: `export { local as exported }`, or the
/// name itself.
fn local_name_of_export(analysis: &Analysis, exported: &str) -> Option<String> {
    let text = &analysis.text;
    let root = analysis.root();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if statement.kind() != "export_statement"
            || statement.child_by_field_name("source").is_some()
        {
            continue;
        }
        let mut inner = statement.walk();
        for clause in statement
            .named_children(&mut inner)
            .filter(|c| c.kind() == "export_clause")
        {
            let mut specifiers = clause.walk();
            for specifier in clause.named_children(&mut specifiers) {
                let (Some(local), Some(alias)) = (
                    specifier.child_by_field_name("name"),
                    specifier.child_by_field_name("alias"),
                ) else {
                    continue;
                };
                if text[alias.byte_range()].trim_matches(['"', '\'']) == exported {
                    return Some(text[local.byte_range()].to_string());
                }
            }
        }
    }
    Some(exported.to_string())
}

struct Found {
    places: Vec<Location>,
    /// The names under which the file exports the target again.
    reexported: Vec<String>,
}

/// The uses in `file` of the export `name` of the file whose key is `target`.
fn uses_in(file: &Path, content: &str, target: &str, name: &str) -> Found {
    let mut found = Found {
        places: Vec::new(),
        reexported: Vec::new(),
    };
    let _dialect = use_dialect(Dialect::of(Some(file), ""));
    let Some(analysis) = Analysis::new(content) else {
        return found;
    };
    let Some(dir) = file.parent() else {
        return found;
    };
    let resolves_to_target = |source: &str| {
        resolve_module(dir, source).is_some_and(|resolved| path_key(&resolved) == target)
    };
    let located = |range: Range<usize>| Location::in_text(Some(file.to_path_buf()), content, range);
    // The imports.
    if let Some(scope) = analysis.scopes.first() {
        for decl in &scope.decls {
            let Some((source, imported)) = &decl.import else {
                continue;
            };
            if imported != name && imported != "*" {
                continue;
            }
            if !resolves_to_target(source) {
                continue;
            }
            if imported == "*" {
                // `ns.name`
                visit(analysis.root(), &mut |node| {
                    if node.kind() == "member_expression"
                        && let (Some(object), Some(property)) = (
                            node.child_by_field_name("object"),
                            node.child_by_field_name("property"),
                        )
                        && object.kind() == "identifier"
                        && content[object.byte_range()] == decl.name
                        && content[property.byte_range()] == *name
                        && analysis
                            .resolve(&decl.name, object.start_byte())
                            .is_some_and(|d| d.name_range == decl.name_range)
                    {
                        found.places.push(located(property.byte_range()));
                    }
                });
                continue;
            }
            found.places.push(located(decl.name_range.clone()));
            found.places.extend(
                references(content, decl.name_range.start)
                    .into_iter()
                    .map(|l| Location {
                        path: Some(file.to_path_buf()),
                        ..l
                    }),
            );
        }
    }
    // The re-exports: `export { name as other } from './target'`, `export * from './target'`.
    let root = analysis.root();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if statement.kind() != "export_statement" {
            continue;
        }
        let Some(source) = statement.child_by_field_name("source") else {
            continue;
        };
        let specifier_text = content[source.byte_range()].trim_matches(['"', '\'', '`']);
        if !resolves_to_target(specifier_text) {
            continue;
        }
        let mut inner = statement.walk();
        let mut clause_found = false;
        for clause in statement
            .named_children(&mut inner)
            .filter(|c| c.kind() == "export_clause")
        {
            clause_found = true;
            let mut specifiers = clause.walk();
            for specifier in clause.named_children(&mut specifiers) {
                let Some(local) = specifier.child_by_field_name("name") else {
                    continue;
                };
                if content[local.byte_range()].trim_matches(['"', '\'']) != name {
                    continue;
                }
                found.places.push(located(local.byte_range()));
                let exported = specifier.child_by_field_name("alias").unwrap_or(local);
                found.reexported.push(
                    content[exported.byte_range()]
                        .trim_matches(['"', '\''])
                        .to_string(),
                );
            }
        }
        // `export * from`: every name but the default one goes on. `export * as ns from` names
        // a namespace, followed no further.
        let namespace = {
            let mut c = statement.walk();
            statement
                .named_children(&mut c)
                .any(|n| n.kind() == "namespace_export")
        };
        if !clause_found && !namespace && name != "default" {
            found.reexported.push(name.to_string());
        }
    }
    found
}

fn visit<'t>(node: Node<'t>, f: &mut impl FnMut(Node<'t>)) {
    f(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit(child, f);
    }
}

#[cfg(test)]
#[path = "../../../test/languages/javascript/usages/tests.rs"]
mod tests;
