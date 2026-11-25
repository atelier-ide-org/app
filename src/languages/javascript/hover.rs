//! Tooltips of JavaScript code: what a name is (its declaration, type and JSDoc) and what a
//! member of a built-in or local type does.

use super::{
    analysis::{Analysis, Decl, DeclKind, Dialect, Ty, use_dialect},
    builtins,
    complete::MAX_TEXT,
    navigate, usages,
};
use crate::languages::Hover;
use std::path::Path;

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The tooltip at `offset` of the file `path`: [`hover`], and what needs the other files of the
/// project: a name imported from another file is described by its declaration there.
pub fn hover_in(text: &str, offset: usize, path: Option<&Path>) -> Option<Hover> {
    if text.len() > MAX_TEXT || offset > text.len() {
        return None;
    }
    let analysis = Analysis::new(text)?;
    if let Some(info) = default_export(&analysis, offset) {
        return Some(info);
    }
    if let Some(path) = path
        && let Some(info) = imported(&analysis, offset, path)
    {
        return Some(info);
    }
    hover(text, offset)
}

/// `export default …`, the pointer on `export` or `default`: what the module exports.
fn default_export(analysis: &Analysis, offset: usize) -> Option<Hover> {
    let root = analysis.root();
    let node = [offset, offset.saturating_sub(1)]
        .into_iter()
        .filter_map(|at| root.descendant_for_byte_range(at, at))
        .find(|n| matches!(&analysis.text[n.byte_range()], "default" | "export"))?;
    let statement = node.parent().filter(|p| p.kind() == "export_statement")?;
    if !usages::is_default_export(analysis, statement) {
        return None;
    }
    let range = node.byte_range();
    if let Some(decl) = usages::export_decl(analysis, "default") {
        let (title, body) = describe_decl(analysis, decl);
        return Some(Hover {
            range,
            title: format!("export default {title}"),
            body,
        });
    }
    let value = usages::default_value(analysis)?;
    Some(Hover {
        range,
        title: format!("export default {}", value_summary(analysis, value)),
        body: String::new(),
    })
}

/// The type of an expression, or its text when the type is unknown.
fn value_summary(analysis: &Analysis, value: tree_sitter::Node) -> String {
    let ty = analysis.ty_of(value, value.start_byte());
    if ty != Ty::Unknown {
        return analysis.show(&ty);
    }
    let text = collapse(&analysis.text[value.byte_range()]);
    if text.chars().count() > 80 {
        format!("{} …", text.chars().take(80).collect::<String>())
    } else {
        text
    }
}

/// A name imported from a file of the project: its declaration in that file.
fn imported(analysis: &Analysis, offset: usize, path: &Path) -> Option<Hover> {
    let root = analysis.root();
    let leaf = [offset, offset.saturating_sub(1)]
        .into_iter()
        .filter_map(|at| root.descendant_for_byte_range(at, at))
        .find(|n| matches!(n.kind(), "identifier" | "type_identifier"))?;
    let name = &analysis.text[leaf.byte_range()];
    let decl = analysis.resolve(name, leaf.start_byte())?;
    let (source, imported) = decl.import.as_ref().filter(|(_, i)| i != "*")?;
    let file = navigate::resolve_module(path.parent()?, source)?;
    let content = std::fs::read_to_string(&file)
        .ok()
        .filter(|c| c.len() <= MAX_TEXT)?;
    let _dialect = use_dialect(Dialect::of(Some(&file), ""));
    let other = Analysis::new(&content)?;
    let range = leaf.byte_range();
    let from = trf!("javascript.hover_from_module", module = source);
    let with_origin = |doc: String| {
        if doc.is_empty() {
            from.clone()
        } else {
            format!("{doc}\n\n{from}")
        }
    };
    if let Some(declared) = usages::export_decl(&other, imported) {
        let (title, body) = describe_decl(&other, declared);
        return Some(Hover {
            range,
            title: format!("(alias) {title}"),
            body: with_origin(body),
        });
    }
    if imported == "default"
        && let Some(value) = usages::default_value(&other)
    {
        return Some(Hover {
            range,
            title: format!("(alias) {name}: {}", value_summary(&other, value)),
            body: with_origin(String::new()),
        });
    }
    None
}

pub fn hover(text: &str, offset: usize) -> Option<Hover> {
    if text.len() > MAX_TEXT || offset > text.len() {
        return None;
    }
    let analysis = Analysis::new(text)?;
    let leaf = analysis.root().descendant_for_byte_range(offset, offset)?;
    // On the edge of a word, the pointer may be just past its last character.
    let leaf = match leaf.kind() {
        "identifier"
        | "property_identifier"
        | "shorthand_property_identifier"
        | "this"
        | "type_identifier"
        | "predefined_type" => leaf,
        _ => analysis
            .root()
            .descendant_for_byte_range(offset.saturating_sub(1), offset.saturating_sub(1))?,
    };
    // `string` is a token inside the node of the predefined type.
    let leaf = match leaf.parent() {
        Some(parent) if parent.kind() == "predefined_type" => parent,
        _ => leaf,
    };
    let range = leaf.byte_range();
    let name = &text[range.clone()];
    match leaf.kind() {
        "identifier" | "shorthand_property_identifier" => {
            if let Some(decl) = analysis.resolve(name, offset) {
                let (title, body) = describe_decl(&analysis, decl);
                return Some(Hover { range, title, body });
            }
            let (signature, doc) = builtins::global_doc(name)?;
            Some(Hover {
                range,
                title: format!("{name}: {signature}"),
                body: doc.to_string(),
            })
        }
        "type_identifier" | "predefined_type" => {
            if leaf.kind() == "type_identifier"
                && let Some(decl) = analysis.resolve(name, offset)
            {
                let (title, body) = describe_decl(&analysis, decl);
                return Some(Hover { range, title, body });
            }
            let doc = crate::languages::typescript::builtins::type_doc(name)?;
            Some(Hover {
                range,
                title: name.to_string(),
                body: doc.to_string(),
            })
        }
        "this" => {
            let ty = analysis.ty_of(leaf, offset);
            Some(Hover {
                range,
                title: format!("this: {}", analysis.show(&ty)),
                body: String::new(),
            })
        }
        "property_identifier" => {
            let parent = leaf.parent()?;
            if parent.kind() != "member_expression" {
                // The name of a method or a key being declared.
                return declared_member(&analysis, leaf, range);
            }
            let object = parent.child_by_field_name("object")?;
            let ty = analysis.ty_of(object, offset);
            let prop = analysis
                .props_of(&ty, offset)
                .into_iter()
                .find(|p| p.name == name)?;
            let title = if prop.method {
                format!("(method) {}{}", prop.name, short_signature(&prop.detail))
            } else {
                format!("(property) {}: {}", prop.name, prop.detail)
            };
            Some(Hover {
                range,
                title,
                body: prop.doc,
            })
        }
        _ => None,
    }
}

/// `(a, b) => any` shown after a method name as `(a, b)`.
fn short_signature(detail: &str) -> String {
    match detail.split_once(" => ") {
        Some((params, _)) => params.to_string(),
        None => detail.to_string(),
    }
}

fn declared_member(
    analysis: &Analysis,
    leaf: tree_sitter::Node,
    range: std::ops::Range<usize>,
) -> Option<Hover> {
    let parent = leaf.parent()?;
    if !matches!(
        parent.kind(),
        "method_definition"
            | "field_definition"
            | "pair"
            | "public_field_definition"
            | "property_signature"
            | "method_signature"
            | "abstract_method_signature"
            | "enum_assignment"
    ) {
        return None;
    }
    let doc = analysis
        .jsdoc_before(&parent.byte_range())
        .unwrap_or_default();
    let name = &analysis.text[range.clone()];
    let title = match parent.kind() {
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            format!(
                "(method) {name}{}",
                short_signature(&analysis.signature(&parent))
            )
        }
        "public_field_definition" | "property_signature" => {
            match parent.child_by_field_name("type") {
                Some(annotation) => format!(
                    "(property) {name}: {}",
                    analysis.annotation_text(annotation)
                ),
                None => format!("(property) {name}"),
            }
        }
        _ => format!("(property) {name}"),
    };
    Some(Hover {
        range,
        title,
        body: doc,
    })
}

fn describe_decl(analysis: &Analysis, decl: &Decl) -> (String, String) {
    let ty = analysis.decl_ty(decl, 0);
    let doc = analysis.jsdoc_before(&decl.node).unwrap_or_default();
    let title = match decl.kind {
        DeclKind::Function => analysis
            .node_at_range(&decl.node)
            .map(|n| {
                let end = n
                    .child_by_field_name("body")
                    .map_or(n.end_byte(), |b| b.start_byte());
                collapse(&analysis.text[n.start_byte()..end])
            })
            .unwrap_or_else(|| decl.name.clone()),
        DeclKind::Class => analysis
            .node_at_range(&decl.node)
            .map(|n| {
                let end = n
                    .child_by_field_name("body")
                    .map_or(n.end_byte(), |b| b.start_byte());
                collapse(&analysis.text[n.start_byte()..end])
            })
            .unwrap_or_else(|| format!("class {}", decl.name)),
        DeclKind::Interface | DeclKind::Enum | DeclKind::Namespace => analysis
            .node_at_range(&decl.node)
            .map(|n| {
                let end = n
                    .child_by_field_name("body")
                    .map_or(n.end_byte(), |b| b.start_byte());
                collapse(&analysis.text[n.start_byte()..end])
            })
            .unwrap_or_else(|| decl.name.clone()),
        DeclKind::TypeAlias => analysis
            .node_at_range(&decl.node)
            .map(|n| {
                let text = collapse(&analysis.text[n.byte_range()]);
                let text = text.trim_end_matches(';').to_string();
                if text.chars().count() > 160 {
                    format!("{} …", text.chars().take(160).collect::<String>())
                } else {
                    text
                }
            })
            .unwrap_or_else(|| decl.name.clone()),
        DeclKind::Param => format!("(parameter) {}: {}", decl.name, analysis.show(&ty)),
        DeclKind::Catch => format!("(parameter) {}: any", decl.name),
        DeclKind::Import => format!("(import) {}", decl.name),
        DeclKind::Const | DeclKind::Let | DeclKind::Var => {
            let keyword = match decl.kind {
                DeclKind::Const => "const",
                DeclKind::Let => "let",
                _ => "var",
            };
            format!("{keyword} {}: {}", decl.name, analysis.show(&ty))
        }
    };
    let body = match (&decl.import, doc.is_empty()) {
        (Some((module, _)), _) => trf!("javascript.hover_from_module", module = module),
        (None, false) => doc,
        (None, true) => String::new(),
    };
    let _ = Ty::Unknown;
    (title, body)
}

#[cfg(test)]
#[path = "../../../test/languages/javascript/hover/tests.rs"]
mod tests;
