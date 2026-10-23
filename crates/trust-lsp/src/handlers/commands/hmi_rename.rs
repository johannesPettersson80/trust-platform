//! HMI bindings follow a rename (specification 39, §7.8): the rename's edits and Find All
//! References reach the bindings of the project's page and popup files whose paths name what is
//! renamed, and the equipment instances objects show. Paths resolve against the program as it is
//! before the rename; a file that does not load, or a path that does not resolve, is left for the
//! binding checks to list.

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;

use tower_lsp::lsp_types::{Location, Position, Range as LspRange, TextEdit, Url};
use trust_ide::rename_identity::RenameIdentity;
use trust_runtime::hmi::member_paths::{path_steps, MemberPath, PathStep};
use trust_runtime::hmi::page_files::{object_key_line, parse_page, parse_popup, problem_line};

use super::{compile_project, HmiBindingsCommandArgs};
use crate::handlers::context::ServerContext;

/// The edits a rename makes to the project's HMI files, by file: each named part of a binding's
/// path that names what is renamed becomes the new name, and every other byte stays.
pub(crate) fn rename_edits_with_context<C: ServerContext>(
    context: &C,
    document: &Url,
    identity: &RenameIdentity,
    new_name: &str,
) -> Result<HashMap<Url, Vec<TextEdit>>, String> {
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for found in found_parts(context, document, identity, true) {
        changes.entry(found.uri).or_default().push(TextEdit {
            range: found.range,
            new_text: new_name.to_string(),
        });
    }
    if let Some(root) = project_root(context, document)
        && let Some((uri, edit)) =
            super::hmi_plant_project::rename_edit(context, &root, identity, new_name)?
    {
        changes.entry(uri).or_default().push(edit);
    }
    Ok(changes)
}

/// The bindings of the project's HMI files that name what is renamed, each at its line.
pub(crate) fn references_with_context<C: ServerContext>(
    context: &C,
    document: &Url,
    identity: &RenameIdentity,
) -> Vec<Location> {
    found_parts(context, document, identity, false)
        .into_iter()
        .map(|found| Location {
            uri: found.uri,
            range: found.range,
        })
        .collect()
}

/// A named part of a binding's path that names what is renamed.
struct Found {
    uri: Url,
    range: LspRange,
}

/// The named parts that name what is renamed: in bindings and in the instances objects show,
/// and with `tags` in an object's displayed tag while it is its instance's own name.
fn found_parts<C: ServerContext>(
    context: &C,
    document: &Url,
    identity: &RenameIdentity,
    tags: bool,
) -> Vec<Found> {
    let Some(root) = project_root(context, document) else {
        return Vec::new();
    };
    let files = hmi_files(context, &root);
    // Compiling the program costs; a project whose HMI files never mention the name skips it.
    let name = identity_name(identity).to_ascii_lowercase();
    if !files
        .iter()
        .any(|file| file.text.to_ascii_lowercase().contains(&name))
    {
        return Vec::new();
    }
    let selection = HmiBindingsCommandArgs {
        open: Default::default(),
        root_uri: Url::from_directory_path(&root).ok(),
        text_document: None,
    };
    // Without a program that compiles there are no paths to follow; the program's own rename
    // still happens.
    let Ok(project) = compile_project(context, &selection, "rename") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for file in files {
        let page = if file.popup {
            parse_popup(&file.name, &file.text).map(|popup| popup.page)
        } else {
            parse_page(&file.name, &file.text)
        };
        let Ok(page) = page else {
            continue;
        };
        for object in &page.objects {
            for (property, binding) in &object.bind {
                let matched = matched_parts(identity, &project.runtime, &binding.path);
                if !matched.is_empty() {
                    let line =
                        problem_line(&file.text, &object.id, Some(property)).saturating_sub(1);
                    found.extend(parts_at(&file, line, &binding.path, &matched));
                }
            }
            // The equipment an object shows (§21.1) follows as its bindings do.
            let Some(instance) = &object.instance else {
                continue;
            };
            let matched = matched_parts(identity, &project.runtime, instance);
            if matched.is_empty() {
                continue;
            }
            let line = object_key_line(&file.text, &object.id, "instance").saturating_sub(1);
            found.extend(parts_at(&file, line, instance, &matched));
            let own = named_parts(instance).pop();
            if let (true, Some(own), Some(tag)) = (tags, own, object.tag.as_deref())
                && matched.contains(&own)
                && tag.eq_ignore_ascii_case(&instance[own])
            {
                let line = object_key_line(&file.text, &object.id, "tag").saturating_sub(1);
                let whole = 0..tag.len();
                found.extend(parts_at(&file, line, tag, std::slice::from_ref(&whole)));
            }
        }
    }
    found
}

/// The named parts of a path that name what is renamed, resolved against the program.
fn matched_parts(
    identity: &RenameIdentity,
    runtime: &trust_runtime::Runtime,
    path: &str,
) -> Vec<Range<usize>> {
    let Ok(parsed) = MemberPath::parse(path) else {
        return Vec::new();
    };
    let Some(steps) = path_steps(&parsed, runtime) else {
        return Vec::new();
    };
    let parts = named_parts(path);
    if parts.len() != steps.len() {
        return Vec::new();
    }
    parts
        .iter()
        .zip(&steps)
        .enumerate()
        .filter(|(index, (part, step))| names(identity, *index, &path[(*part).clone()], step))
        .map(|(_, (part, _))| part.clone())
        .collect()
}

/// Where `parts` of the quoted `text` are on a file's zero-based `line`.
fn parts_at(file: &HmiFile, line: usize, text: &str, parts: &[Range<usize>]) -> Vec<Found> {
    let Some(line_text) = file.text.lines().nth(line) else {
        return Vec::new();
    };
    let Some(column) = line_text.find(&format!("\"{text}\"")) else {
        return Vec::new();
    };
    let line = u32::try_from(line).unwrap_or(u32::MAX);
    parts
        .iter()
        .map(|part| {
            let (start, end) = (column + 1 + part.start, column + 1 + part.end);
            Found {
                uri: file.uri.clone(),
                range: LspRange {
                    start: Position {
                        line,
                        character: utf16_len(&line_text[..start]),
                    },
                    end: Position {
                        line,
                        character: utf16_len(&line_text[..end]),
                    },
                },
            }
        })
        .collect()
}

/// The root of the project that holds the document: the workspace project that holds it, or else
/// the nearest folder above it with HMI pages or popups, as for a project inside a workspace.
fn project_root<C: ServerContext>(context: &C, document: &Url) -> Option<PathBuf> {
    let path = document.to_file_path().ok()?;
    let configured = context
        .workspace_configs()
        .into_iter()
        .map(|(_, config)| config.root)
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.as_os_str().len());
    configured.or_else(|| {
        path.ancestors()
            .skip(1)
            .take(32)
            .find(|folder| {
                ["pages", "popups"]
                    .iter()
                    .any(|kind| folder.join("hmi").join(kind).is_dir())
            })
            .map(std::path::Path::to_path_buf)
    })
}

/// A page or popup file of the project with its text, as the editor holds it when it is open.
struct HmiFile {
    uri: Url,
    name: String,
    text: String,
    popup: bool,
}

fn hmi_files<C: ServerContext>(context: &C, root: &std::path::Path) -> Vec<HmiFile> {
    let mut files = Vec::new();
    for (folder, popup) in [("pages", false), ("popups", true)] {
        for path in toml_files(root.join("hmi").join(folder)) {
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            let text = match context.get_document(&uri) {
                Some(document) => document.content.clone(),
                None => match std::fs::read_to_string(&path) {
                    Ok(text) => text,
                    Err(_) => continue,
                },
            };
            let name = format!(
                "hmi/{folder}/{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            files.push(HmiFile {
                uri,
                name,
                text,
                popup,
            });
        }
    }
    files
}

fn identity_name(identity: &RenameIdentity) -> &str {
    match identity {
        RenameIdentity::Member { name, .. }
        | RenameIdentity::Field { name, .. }
        | RenameIdentity::Root { name }
        | RenameIdentity::Program { name } => name.as_str(),
    }
}

/// Whether the `index`-th named part of a path, `part`, with what it names, `step`, is what the
/// rename renames. Identifiers compare without regard to case, as in IEC 61131-3.
fn names(identity: &RenameIdentity, index: usize, part: &str, step: &PathStep) -> bool {
    match (identity, step) {
        (RenameIdentity::Member { owner, name }, PathStep::Member { declaring_type }) => {
            declaring_type.eq_ignore_ascii_case(owner) && part.eq_ignore_ascii_case(name)
        }
        (RenameIdentity::Field { owner, name }, PathStep::Field { structure_type }) => {
            structure_type.eq_ignore_ascii_case(owner) && part.eq_ignore_ascii_case(name)
        }
        (RenameIdentity::Root { name }, PathStep::Root { .. }) => {
            index == 0 && part.eq_ignore_ascii_case(name)
        }
        // A program's instance carries its name when no configuration names it.
        (
            RenameIdentity::Program { name },
            PathStep::Root {
                instance_type: Some(instance_type),
            },
        ) => {
            index == 0
                && instance_type.eq_ignore_ascii_case(name)
                && part.eq_ignore_ascii_case(name)
        }
        _ => false,
    }
}

/// The named parts of a path's text, each as its byte range: the identifiers between the dots,
/// without their indices.
fn named_parts(path: &str) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut start = 0;
    for piece in path.split('.') {
        let leading = piece.len() - piece.trim_start().len();
        let identifier = piece[..piece.find('[').unwrap_or(piece.len())].trim();
        parts.push(start + leading..start + leading + identifier.len());
        start += piece.len() + 1;
    }
    parts
}

/// The TOML files of a folder, in name order.
fn toml_files(folder: PathBuf) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    files.sort();
    files
}

fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}
