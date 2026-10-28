//! Keep in project (Specification 35 §17.17): the parameter values Control Studio applied to the
//! running PLC become the initial values of their record in the project, so a restart or a new
//! download keeps them. Apply never edits ST; this is the separate action the user chooses after it.
//!
//! A record written by hand is edited in its own declaration: the kept fields of its initializer
//! change, the others stay as written. A record Create PLC code declared belongs to the
//! equipment design (`plant.toml`, specification 39 §10): the answer names the loop instance and the
//! settings the HMI Builder's equipment edit (`trust-lsp.hmiEquipment`) keeps, so the generated
//! declarations stay the generator's own.
//!
//! A REAL or LREAL variable takes its value as its initial value, written `: <type> := <value>;`. A
//! program's own law keeps its gain variables this way (specification 35, C2 and B6.5): the request names
//! the program instance, and its values name the variables. Nothing is compiled, written or run here.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Map, Value};
use tower_lsp::lsp_types::{Range, TextEdit, Url};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::control_links::code_text;
use super::hmi_files::{refused, single_argument};
use super::{HmiBindingsCommandArgs, CONTROL_KEEP_PARAMETERS_COMMAND};
use crate::handlers::context::ServerContext;
use crate::handlers::lsp_utils::offset_to_position;

/// The most fields one keep writes: every field of a parameter record is far fewer.
const MAX_FIELDS: usize = 32;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    root_uri: Option<Url>,
    /// Unsaved editor texts by project-relative path.
    #[serde(default)]
    open: BTreeMap<String, String>,
    /// The record as Control Studio binds it, `P1.Parameters` or `Main.TIC101_Parameters`; or a program
    /// instance, `P1`, whose variables the values name (a program's own law, C2).
    record: String,
    /// The applied value of each kept field, or of each variable the program instance names. A
    /// `root.variable` that is a REAL or LREAL variable takes its one value, keyed by its own name.
    values: BTreeMap<String, f64>,
}

/// `trust-lsp.controlKeepParameters`: `{ ok, edit: { changes }, expected, kept }` for a record
/// written by hand, or for a program's REAL or LREAL variables; `{ ok: false, generated: { instance,
/// settings } }` for a record Create PLC code declared; `{ ok: false, errors }` when the values
/// cannot be kept safely.
pub(super) fn keep<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    keep_result(context, args).unwrap_or_else(refused)
}

fn keep_result<C: ServerContext>(context: &C, args: Vec<Value>) -> Result<Value, String> {
    let args: Args = single_argument(args, CONTROL_KEEP_PARAMETERS_COMMAND)?;
    if args.values.is_empty() || args.values.len() > MAX_FIELDS {
        return Err(format!(
            "Keep between 1 and {MAX_FIELDS} values of {}.",
            args.record
        ));
    }
    if let Some((field, _)) = args
        .values
        .iter()
        .find(|(field, value)| !is_identifier(field) || !value.is_finite())
    {
        return Err(format!(
            "{}.{field} has no value that can be kept.",
            args.record
        ));
    }
    // `root.variable` names a record, or a REAL or LREAL variable that takes its own name's value; `root` names
    // a program instance, and the values name its variables (C2).
    let (root, variable) = match args.record.split('.').collect::<Vec<_>>().as_slice() {
        [root, variable] if is_identifier(root) && is_identifier(variable) => {
            (root.to_string(), Some(variable.to_string()))
        }
        [root] if is_identifier(root) => (root.to_string(), None),
        _ => {
            return Err(format!(
                "Keep in project writes a record declared in a program; copy the values into {} by hand.",
                args.record
            ))
        }
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open,
        root_uri: args.root_uri,
        text_document: None,
    };
    let inputs = super::project_inputs(context, &selection, CONTROL_KEEP_PARAMETERS_COMMAND)?;
    let texts = inputs
        .sources
        .iter()
        .map(|source| source.text.as_str())
        .collect::<Vec<_>>();
    // A configured program instance is named after its program type (`PROGRAM P1 … : Main`).
    let program = super::control_links::program_types(&texts)
        .into_iter()
        .find(|(instance, _)| instance.eq_ignore_ascii_case(&root))
        .map_or_else(|| root.clone(), |(_, program)| program);
    for source in &inputs.sources {
        let tree = parse(&source.text).syntax();
        let Some(pou) = program_node(&tree, &program) else {
            continue;
        };
        let path = if source.path.is_absolute() {
            source.path.clone()
        } else {
            inputs.root.join(&source.path)
        };
        let Some(variable) = variable.as_deref() else {
            return program_variables_answer(
                &pou,
                &source.text,
                &path,
                &root,
                &program,
                &args.values,
            );
        };
        let Some(declaration) = declaration(&pou, variable) else {
            continue;
        };
        if generated(&source.text, &declaration) {
            return generated_answer(&pou, &root, variable, &args.values, &args.record);
        }
        let edits = declaration_edits(&source.text, &declaration, &args.values, &args.record)?;
        return edit_answer(
            &path,
            &source.text,
            edits,
            args.values.keys().cloned().collect(),
        );
    }
    Err(format!(
        "{} is not declared in PROGRAM {program} of this project.",
        args.record
    ))
}

/// The record in a Create PLC code region: its loop instance and the settings that keep it.
fn generated_answer(
    pou: &SyntaxNode,
    root: &str,
    variable: &str,
    values: &BTreeMap<String, f64>,
    record: &str,
) -> Result<Value, String> {
    let instance = configured_from(pou, variable).ok_or_else(|| {
        format!("Create PLC code declared {record}, but no controller in its program is configured from it.")
    })?;
    // The generator names a loop's record after the loop (`TIC101_Parameters` of `TIC101`); the
    // equipment's settings name it relative to the instance (`{instance}_Parameters.<field>`).
    let suffix = variable
        .get(..instance.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(&instance))
        .map(|_| &variable[instance.len()..])
        .filter(|suffix| !suffix.is_empty())
        .ok_or_else(|| {
            format!("Create PLC code declared {record} for a controller it does not name; keep the values in its equipment settings.")
        })?;
    let settings = values
        .iter()
        .map(|(field, value)| (format!("{{instance}}{suffix}.{field}"), json!(value)))
        .collect::<Map<_, _>>();
    Ok(json!({
        "ok": false,
        "generated": { "instance": format!("{root}.{instance}"), "settings": settings },
    }))
}

/// The answer that edits a file: its edits, the text they were prepared from, and the fields they keep.
fn edit_answer(
    path: &Path,
    text: &str,
    edits: Vec<TextEdit>,
    kept: Vec<String>,
) -> Result<Value, String> {
    let uri = Url::from_file_path(path)
        .map_err(|_| format!("{} is not a valid file path", path.display()))?;
    let mut changes = Map::new();
    changes.insert(uri.to_string(), json!(edits));
    let mut expected = Map::new();
    expected.insert(uri.to_string(), json!(text));
    Ok(json!({
        "ok": true,
        "edit": { "changes": changes },
        "expected": expected,
        "kept": kept,
    }))
}

/// The edit of a program's own variables (C2, B6.5): each named variable must be a REAL or LREAL variable of the program, and
/// its value becomes its initial value.
fn program_variables_answer(
    pou: &SyntaxNode,
    text: &str,
    path: &Path,
    root: &str,
    program: &str,
    values: &BTreeMap<String, f64>,
) -> Result<Value, String> {
    let mut edits = Vec::new();
    for (name, value) in values {
        let record = format!("{root}.{name}");
        let declaration = declaration(pou, name).ok_or_else(|| {
            format!("{record} is not declared in PROGRAM {program} of this project.")
        })?;
        if generated(text, &declaration) {
            return Err(format!(
                "{record} is declared by Create PLC code; copy its value into the PLC code by hand."
            ));
        }
        if !is_real(&declaration) {
            return Err(format!(
                "{record} is not a REAL or LREAL variable; copy its value into the program by hand."
            ));
        }
        let one = BTreeMap::from([(name.clone(), *value)]);
        edits.extend(declaration_edits(text, &declaration, &one, &record)?);
    }
    edit_answer(path, text, edits, values.keys().cloned().collect())
}

/// The edits setting each value in the declaration's initializer: a field present takes the new
/// value, written as the old one was (`LREAL#…` stays typed); a missing field is added; a
/// declaration without an initializer gets one. A REAL or LREAL declaration takes its one value
/// instead (`scalar_edits`).
fn declaration_edits(
    text: &str,
    declaration: &SyntaxNode,
    values: &BTreeMap<String, f64>,
    record: &str,
) -> Result<Vec<TextEdit>, String> {
    if is_real(declaration) {
        return scalar_edits(text, declaration, values, record);
    }
    if !is_record(declaration) {
        return Err(format!(
            "{record} is not a record, so its fields cannot be kept; copy its value into the program by hand."
        ));
    }
    let range = |start: usize, end: usize| lsp_range(text, start, end);
    let Some(list) = declaration
        .children()
        .find(|child| child.kind() == SyntaxKind::InitializerList)
    else {
        if declaration
            .children()
            .any(|child| child.kind().is_expression_node())
        {
            return Err(format!(
                "{record} is initialized from an expression; copy the values into it by hand."
            ));
        }
        let entries = values
            .iter()
            .map(|(field, value)| format!("{field} := {}", st_real(*value)))
            .collect::<Vec<_>>()
            .join(", ");
        let at = declaration
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::Semicolon)
            .map(|token| usize::from(token.text_range().start()))
            .ok_or_else(|| format!("The declaration of {record} has no closing ';'."))?;
        return Ok(vec![TextEdit {
            range: range(at, at),
            new_text: format!(" := ({entries})"),
        }]);
    };
    // The list holds each field's name followed by its value.
    let mut present = BTreeMap::new();
    let mut elements = list.children().peekable();
    while let Some(element) = elements.next() {
        if element.kind() != SyntaxKind::Name {
            continue;
        }
        let name = code_text(&element).to_ascii_uppercase();
        if let Some(value) = elements.next_if(|node| node.kind() != SyntaxKind::Name) {
            present.insert(name, value);
        }
    }
    let mut edits = Vec::new();
    let mut missing = Vec::new();
    for (field, value) in values {
        let Some(node) = present.get(&field.to_ascii_uppercase()) else {
            missing.push(format!("{field} := {}", st_real(*value)));
            continue;
        };
        if node.kind().is_aggregate_initializer_node() {
            return Err(format!(
                "{record}.{field} holds a structure, not a number; it cannot be kept."
            ));
        }
        let written = code_text(node);
        let prefix = typed_prefix(&written).unwrap_or_default();
        let (start, end) =
            code_range(node).ok_or_else(|| format!("{record}.{field} has no value to replace."))?;
        edits.push(TextEdit {
            range: range(start, end),
            new_text: format!("{prefix}{}", st_real(*value)),
        });
    }
    if !missing.is_empty() {
        // After the last value, before any comment, line break and the closing parenthesis.
        let (_, at) = list
            .children()
            .last()
            .and_then(|last| code_range(&last))
            .ok_or_else(|| format!("The initializer of {record} has no fields."))?;
        edits.push(TextEdit {
            range: range(at, at),
            new_text: format!(", {}", missing.join(", ")),
        });
    }
    Ok(edits)
}

/// The edit of a REAL or LREAL variable's initial value (B6.5, C2), written `: <type> := <value>;`: an initializer takes the value
/// and keeps its typed prefix (`LREAL#`), and a variable without one gets ` := <value>` before its `;`. A declaration that names
/// other variables, or whose initializer is not one expression, cannot be edited safely and is refused by the variable's name.
fn scalar_edits(
    text: &str,
    declaration: &SyntaxNode,
    values: &BTreeMap<String, f64>,
    record: &str,
) -> Result<Vec<TextEdit>, String> {
    let Ok([(key, &value)]) = <[(&String, &f64); 1]>::try_from(values.iter().collect::<Vec<_>>())
    else {
        return Err(format!("{record} keeps one value, its own."));
    };
    let names = declaration
        .children()
        .filter(|child| child.kind() == SyntaxKind::Name)
        .count();
    if names != 1 {
        return Err(format!(
            "{record} shares its declaration with other variables; copy its value into the program by hand."
        ));
    }
    let declared = declaration
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| code_text(&name))
        .unwrap_or_default();
    if !key.eq_ignore_ascii_case(&declared) {
        return Err(format!("{record} keeps only its own value, not {key}."));
    }
    let tokens = declaration
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .collect::<Vec<_>>();
    let semicolon = tokens
        .iter()
        .find(|token| token.kind() == SyntaxKind::Semicolon)
        .map(|token| usize::from(token.text_range().start()))
        .ok_or_else(|| format!("The declaration of {record} has no closing ';'."))?;
    let assigned = tokens
        .iter()
        .any(|token| token.kind() == SyntaxKind::Assign);
    let initializer = declaration
        .children()
        .find(|child| child.kind().is_initializer_expression_node());
    let range = |start: usize, end: usize| lsp_range(text, start, end);
    match (assigned, initializer) {
        (false, None) => Ok(vec![TextEdit {
            range: range(semicolon, semicolon),
            new_text: format!(" := {}", st_real(value)),
        }]),
        (true, Some(initializer)) if initializer.kind().is_expression_node() => {
            let (start, end) = code_range(&initializer)
                .ok_or_else(|| format!("{record} has no value to replace."))?;
            let written = code_text(&initializer);
            let prefix = typed_prefix(&written).unwrap_or_default();
            Ok(vec![TextEdit {
                range: range(start, end),
                new_text: format!("{prefix}{}", st_real(value)),
            }])
        }
        _ => Err(format!(
            "{record} has an initializer that Keep cannot read; copy its value into the program by hand."
        )),
    }
}

/// Whether a declaration's type is REAL or LREAL: the scalar variables whose initial value Keep in project writes (C2, B6.5).
fn is_real(declaration: &SyntaxNode) -> bool {
    declaration
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .is_some_and(|type_ref| {
            let written = code_text(&type_ref);
            written.eq_ignore_ascii_case("REAL") || written.eq_ignore_ascii_case("LREAL")
        })
}

/// Whether a declaration's type is a named type, such as a record (`Control.PIDParameters`), rather than an elementary, array or
/// string type, whose initializer is not a record's field list.
fn is_record(declaration: &SyntaxNode) -> bool {
    declaration
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .is_some_and(|type_ref| {
            type_ref
                .children()
                .any(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
        })
}

/// The LSP range of the bytes `start..end` of `text`.
fn lsp_range(text: &str, start: usize, end: usize) -> Range {
    Range {
        start: offset_to_position(text, start as u32),
        end: offset_to_position(text, end as u32),
    }
}

/// Whether a declaration lies inside a region Create PLC code owns (`(* truST plant: …
/// declarations` up to its end marker). The declaration starts at its name: its node also holds the
/// comments before it, the region's begin marker among them.
fn generated(text: &str, declaration: &SyntaxNode) -> bool {
    let Some((at, _)) = code_range(declaration) else {
        return false;
    };
    let mut inside = false;
    for line in text[..at.min(text.len())].lines() {
        let line = line.trim();
        if line.starts_with("(* truST plant:") && line.contains("declarations") {
            inside = true;
        } else if line.starts_with("(* end of truST plant") && line.contains("declarations") {
            inside = false;
        }
    }
    inside
}

/// The instance the program configures from the record: `<instance>.Configure(Parameters := <record>)`.
fn configured_from(pou: &SyntaxNode, record: &str) -> Option<String> {
    pou.descendants()
        .filter(|node| node.kind() == SyntaxKind::CallExpr)
        .find_map(|call| {
            let mut children = call.children();
            let callee = children.next()?;
            if callee.kind() != SyntaxKind::FieldExpr {
                return None;
            }
            let method = callee
                .children()
                .filter(|child| child.kind() == SyntaxKind::Name)
                .last()?;
            if !code_text(&method).eq_ignore_ascii_case("Configure") {
                return None;
            }
            let instance = callee.children().next()?;
            if instance.kind() != SyntaxKind::NameRef {
                return None;
            }
            let arguments = children.find(|child| child.kind() == SyntaxKind::ArgList)?;
            arguments
                .children()
                .filter(|argument| argument.kind() == SyntaxKind::Arg)
                .any(|argument| {
                    argument
                        .children()
                        .find(|child| child.kind() != SyntaxKind::Name)
                        .is_some_and(|value| {
                            value.kind() == SyntaxKind::NameRef
                                && code_text(&value).eq_ignore_ascii_case(record)
                        })
                })
                .then(|| code_text(&instance))
        })
}

/// `PROGRAM <name>` in a parsed source.
fn program_node(root: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::Program)
        .find(|node| {
            node.children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
                .is_some_and(|child| code_text(&child).eq_ignore_ascii_case(name))
        })
}

/// The declaration of a program's own variable, outside its methods and actions.
fn declaration(pou: &SyntaxNode, variable: &str) -> Option<SyntaxNode> {
    pou.children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .flat_map(|block| block.descendants().collect::<Vec<_>>())
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|declaration| {
            let owner = declaration
                .children()
                .find(|child| child.kind() == SyntaxKind::VarList)
                .unwrap_or_else(|| declaration.clone());
            owner
                .children()
                .filter(|child| child.kind() == SyntaxKind::Name)
                .any(|name| code_text(&name).eq_ignore_ascii_case(variable))
        })
}

/// The byte range of a node's code: its first to its last token that is not trivia.
fn code_range(node: &SyntaxNode) -> Option<(usize, usize)> {
    let mut tokens = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia());
    let first = tokens.next()?;
    let last = tokens.last().unwrap_or_else(|| first.clone());
    Some((
        usize::from(first.text_range().start()),
        usize::from(last.text_range().end()),
    ))
}

/// `LREAL#` of a typed literal.
fn typed_prefix(text: &str) -> Option<&str> {
    let hash = text.find('#')?;
    let prefix = &text[..hash];
    is_identifier(prefix).then(|| &text[..=hash])
}

/// A REAL literal of `value`: its shortest exact decimal, with a decimal point.
fn st_real(value: f64) -> String {
    let text = format!("{value}");
    if text.contains(['.', 'e', 'E']) {
        text
    } else {
        format!("{text}.0")
    }
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
