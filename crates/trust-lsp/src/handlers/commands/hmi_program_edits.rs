//! Add missing variables to program (specification 39, §10): the variables a page's value
//! bindings need in the target program, as one workspace edit for the editor to show for approval.
//! Only declarations are inserted; existing code is never rewritten. Equipment is declared and
//! called by Create PLC code (`generate_project`, §10.2), never here.

use super::hmi_authoring::pairs::{parse_pair, Pair as HmiPair};

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{Range, TextDocumentIdentifier, TextEdit, Url};
use trust_runtime::hmi::catalogue::{Catalogue, ValueKind};
use trust_runtime::hmi::member_paths::{resolve_member, MemberPath};
use trust_runtime::hmi::page_files::{HmiObject, PLACED_FACEPLATE};
use trust_runtime::memory::{InstanceId, VariableStorage};
use trust_runtime::value::Value as StValue;
use trust_syntax::{SyntaxKind, SyntaxNode};

use super::hmi_files::{file_errors, refused, single_argument};
use super::{
    compile_project, CompiledProject, HmiBindingsCommandArgs, HMI_ADD_MISSING_VARIABLES_COMMAND,
};
use crate::handlers::context::ServerContext;
use crate::handlers::lsp_utils::offset_to_position;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddArgs {
    #[serde(default)]
    open: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    text: String,
    /// Create PLC code for the whole project's design (§10).
    #[serde(default)]
    generate_project: bool,
    #[serde(default)]
    screen_task: Option<String>,
    #[serde(default)]
    cycle_ms: Option<u32>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
}

/// What one program receives: declarations (name and type), in the page's order.
#[derive(Default)]
struct ProgramPlan {
    declarations: Vec<(String, String)>,
}

impl ProgramPlan {
    fn declares(&self, name: &str) -> bool {
        self.declarations
            .iter()
            .any(|(declared, _)| declared.eq_ignore_ascii_case(name))
    }
}

/// `trust-lsp.hmiAddMissingVariables`: `{ text, file?, root_uri?, text_document? }` to `{ ok,
/// edit, added, skipped, notes }`. `added` lists each new variable with the object that needs it,
/// `skipped` each binding left out with the reason. With `generate_project` it is Create PLC code.
pub(super) fn add_missing_variables_value<C: ServerContext>(
    context: &C,
    args: Vec<Value>,
) -> Value {
    let args: AddArgs = match single_argument(args, HMI_ADD_MISSING_VARIABLES_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: args.root_uri.clone(),
        text_document: args.text_document.clone(),
    };
    if args.generate_project {
        return super::hmi_generate::create(
            context,
            super::hmi_generate::Request {
                selection,
                screen_task: args.screen_task.clone(),
                cycle_ms: args.cycle_ms,
            },
        );
    }
    let file = args.file.as_deref().unwrap_or("(unsaved)");
    let (page, _) = match super::hmi_authoring::documents::page(
        file,
        &args.text,
        file.replace('\\', "/").contains("hmi/popups/"),
    ) {
        Ok(page) => page,
        Err(errors) => return file_errors(&errors),
    };
    let project = match compile_project(context, &selection, HMI_ADD_MISSING_VARIABLES_COMMAND) {
        Ok(project) => project,
        Err(message) => return refused(message),
    };
    let texts: Vec<&str> = project
        .sources
        .iter()
        .map(|source| source.text.as_str())
        .collect();
    // Compiled storage names a program instance after itself; the configuration names its type.
    let configured = super::control_links::program_types(&texts);
    let mut planner = Planner {
        project: &project,
        configured: &configured,
        plans: BTreeMap::new(),
        added: Vec::new(),
        skipped: Vec::new(),
    };
    for object in &page.objects {
        if object.block.is_some() || object.kind == PLACED_FACEPLATE {
            if object.block.is_some() && object.instance.is_some() {
                planner.skip(
                    &object.id,
                    "equipment is declared and called by Create PLC code".into(),
                );
            }
            continue;
        }
        planner.values(object);
    }
    for pipe in &page.pipes {
        for binding in pipe.bind.values() {
            planner.value(
                &pipe_label(pipe.from.as_str()),
                &binding.path,
                Some(ValueKind::Number),
            );
        }
    }
    let Planner {
        plans,
        added,
        skipped,
        ..
    } = planner;
    let mut changes: BTreeMap<String, Vec<TextEdit>> = BTreeMap::new();
    for (program, plan) in &plans {
        match program_edits(&project, program, plan) {
            Some((uri, edits)) => changes.entry(uri).or_default().extend(edits),
            None => {
                return refused(format!(
                    "{HMI_ADD_MISSING_VARIABLES_COMMAND}: the source of PROGRAM {program} was not found"
                ))
            }
        }
    }
    json!({
        "ok": true,
        "edit": { "changes": changes },
        "added": added,
        "skipped": skipped,
        "notes": [],
    })
}

struct Planner<'a> {
    project: &'a CompiledProject,
    /// `(instance, program type)` of the configuration's program entries.
    configured: &'a [(String, String)],
    /// By program type.
    plans: BTreeMap<String, ProgramPlan>,
    added: Vec<Value>,
    skipped: Vec<Value>,
}

impl Planner<'_> {
    fn skip(&mut self, object: &str, message: String) {
        self.skipped
            .push(json!({ "object": object, "message": message }));
    }

    /// Every binding of an object that is not equipment (§10 item 4).
    fn values(&mut self, object: &HmiObject) {
        let component = Catalogue::shipped().component(&object.kind);
        for (property, binding) in &object.bind {
            let kind = component
                .and_then(|component| component.properties.get(property))
                .and_then(|definition| definition.accepts.first().copied());
            self.value(&object.id, &binding.path, kind);
        }
    }

    /// One value binding: a variable of the binding's type when the program lacks it.
    fn value(&mut self, object: &str, path: &str, kind: Option<ValueKind>) {
        let storage = self.project.runtime.storage();
        if MemberPath::parse(path).is_ok_and(|parsed| resolve_member(&parsed, storage).is_some()) {
            return;
        }
        let Some((program_id, program, name)) = self.place(object, path) else {
            return;
        };
        if member(storage, program_id, &name).is_some() {
            return;
        }
        let Some(type_name) = type_for(kind) else {
            self.skip(
                object,
                format!("the type of `{path}` cannot be told from its object"),
            );
            return;
        };
        let plan = self.plans.entry(program).or_default();
        if !plan.declares(&name) {
            plan.declarations.push((name, type_name.to_string()));
            self.added
                .push(json!({ "object": object, "variable": path, "type": type_name }));
        }
    }

    /// The program instance and the variable a path names directly in it (§10 item 5).
    fn place(&mut self, object: &str, path: &str) -> Option<(InstanceId, String, String)> {
        let segments: Vec<&str> = path.split('.').collect();
        if segments.len() != 2 || !segments.iter().all(|segment| is_identifier(segment)) {
            self.skip(
                object,
                format!("`{path}` is not a variable directly in a program; only those are added"),
            );
            return None;
        }
        let runtime = &self.project.runtime;
        let found = runtime
            .storage()
            .globals()
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(segments[0]))
            .and_then(|(_, value)| match value {
                StValue::Instance(id) => Some(*id),
                _ => None,
            })
            .and_then(|id| {
                let type_name = runtime.storage().get_instance(id)?.type_name.clone();
                runtime
                    .programs()
                    .keys()
                    .any(|program| program.eq_ignore_ascii_case(&type_name))
                    .then(|| {
                        let program = self
                            .configured
                            .iter()
                            .find(|(instance, _)| instance.eq_ignore_ascii_case(segments[0]))
                            .map(|(_, program)| program.clone())
                            .unwrap_or_else(|| type_name.to_string());
                        (id, program)
                    })
            });
        match found {
            Some((id, program)) => Some((id, program, segments[1].to_string())),
            None => {
                self.skip(
                    object,
                    format!(
                        "`{}` is not a program instance; variables are added to programs only",
                        segments[0]
                    ),
                );
                None
            }
        }
    }
}

/// The pairs of the project and its libraries.
pub(super) fn project_pairs_edited(
    root: &Path,
    open: &BTreeMap<std::path::PathBuf, String>,
) -> Vec<HmiPair> {
    super::hmi_authoring::bundle::read_edited(root, open)
        .unwrap_or_default()
        .files
        .into_iter()
        .filter_map(|(path, bytes)| {
            if !path.starts_with("hmi/pairs/") || !path.ends_with(".toml") {
                return None;
            }
            parse_pair(&path, &String::from_utf8_lossy(&bytes)).ok()
        })
        .collect()
}

fn pipe_label(from: &str) -> String {
    format!("pipe from {from}")
}

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// A member of a program instance, by name in any case.
fn member<'a>(
    storage: &'a VariableStorage,
    program: InstanceId,
    name: &str,
) -> Option<&'a StValue> {
    let mut instance = storage.get_instance(program);
    for _ in 0..32 {
        let current = instance?;
        if let Some((_, value)) = current
            .variables
            .iter()
            .find(|(member, _)| member.eq_ignore_ascii_case(name))
        {
            return Some(value);
        }
        instance = current
            .parent
            .and_then(|parent| storage.get_instance(parent));
    }
    None
}

/// The IEC type of a variable a property's binding needs.
fn type_for(kind: Option<ValueKind>) -> Option<&'static str> {
    match kind? {
        ValueKind::Boolean => Some("BOOL"),
        ValueKind::Number | ValueKind::Value => Some("LREAL"),
        ValueKind::Text => Some("STRING"),
        ValueKind::Time => Some("TIME"),
        ValueKind::Enumeration | ValueKind::Structure | ValueKind::Array => None,
    }
}

/// The text edits that add a plan to its program's source.
fn program_edits(
    project: &CompiledProject,
    program: &str,
    plan: &ProgramPlan,
) -> Option<(String, Vec<TextEdit>)> {
    let (path, text, node) = project.sources.iter().find_map(|source| {
        let root = trust_syntax::parser::parse(&source.text).syntax();
        let node = program_node(&root, program)?;
        Some((source.path.clone(), source.text.clone(), node))
    })?;
    let uri = Url::from_file_path(if path.is_absolute() {
        path
    } else {
        project.root.join(path)
    })
    .ok()?
    .to_string();
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut edits = Vec::new();
    if !plan.declarations.is_empty() {
        edits.push(declaration_edit(&text, &node, &plan.declarations, newline)?);
    }
    Some((uri, edits))
}

/// The `PROGRAM` declaration of a type.
fn program_node(root: &SyntaxNode, program: &str) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::Program)
        .find(|node| {
            node.children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .is_some_and(|name| name.text().to_string().trim().eq_ignore_ascii_case(program))
        })
}

/// The declarations, at the end of the program's first `VAR` section, or in a new one after its
/// header.
fn declaration_edit(
    text: &str,
    node: &SyntaxNode,
    declarations: &[(String, String)],
    newline: &str,
) -> Option<TextEdit> {
    let block = node.children().find(|child| {
        child.kind() == SyntaxKind::VarBlock
            && child
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwVar)
    });
    let header_indent = indent_at(text, usize::from(node.text_range().start()));
    match block {
        Some(block) => {
            let end_var = block
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| token.kind() == SyntaxKind::KwEndVar)?;
            let at = line_start(text, usize::from(end_var.text_range().start()));
            let indent = block
                .children()
                .find(|child| child.kind() == SyntaxKind::VarDecl)
                .map(|declaration| indent_at(text, text_start(&declaration)))
                .unwrap_or_else(|| format!("{}    ", indent_at(text, at)));
            let lines: String = declarations
                .iter()
                .map(|(name, declaration)| format!("{indent}{name} : {declaration};{newline}"))
                .collect();
            Some(insert_at(text, at, lines))
        }
        None => {
            let name = node
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)?;
            let at = line_end(text, text_end(&name));
            let lines: String = declarations
                .iter()
                .map(|(name, declaration)| {
                    format!("{header_indent}    {name} : {declaration};{newline}")
                })
                .collect();
            Some(insert_at(
                text,
                at,
                format!("{header_indent}VAR{newline}{lines}{header_indent}END_VAR{newline}"),
            ))
        }
    }
}

/// Where a node's own text ends, before the whitespace it carries.
fn text_end(node: &SyntaxNode) -> usize {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.text().trim().is_empty())
        .last()
        .map_or_else(
            || usize::from(node.text_range().end()),
            |token| usize::from(token.text_range().end()),
        )
}

/// Where a node's own text begins, after the whitespace it carries.
fn text_start(node: &SyntaxNode) -> usize {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.text().trim().is_empty())
        .map_or_else(
            || usize::from(node.text_range().start()),
            |token| usize::from(token.text_range().start()),
        )
}

fn insert_at(text: &str, offset: usize, new_text: String) -> TextEdit {
    let position = offset_to_position(text, offset as u32);
    TextEdit {
        range: Range {
            start: position,
            end: position,
        },
        new_text,
    }
}

fn line_start(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .rfind('\n')
        .map_or(0, |index| index + 1)
}

fn line_end(text: &str, offset: usize) -> usize {
    text[offset.min(text.len())..]
        .find('\n')
        .map_or(text.len(), |index| offset + index + 1)
}

fn indent_at(text: &str, offset: usize) -> String {
    let start = line_start(text, offset);
    text[start..]
        .chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .collect()
}

#[cfg(test)]
mod placement_tests {
    use super::{is_identifier, line_end, line_start};

    #[test]
    fn identifiers_and_lines() {
        assert!(
            is_identifier("P101")
                && is_identifier("_x")
                && !is_identifier("1a")
                && !is_identifier("a[1]")
        );
        assert_eq!(line_start("ab\ncd", 4), 3);
        assert_eq!(line_end("ab\ncd\n", 3), 6);
    }
}
