//! Member paths of HMI bindings (specification 39, §7.2–7.3).
//!
//! A path such as `FeedArea.P101.Running` or `FeedArea.Pumps[2].Running` starts at a program
//! instance or a global and walks function block instance members, structure fields and array
//! elements. The runtime resolves the paths that HMI pages bind in the running program through
//! its image's symbol and type tables (specification 44 §13.3); the editor's checks resolve them
//! in the project's declarations. Neither enumerates every nested member of every instance.

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime_core::native::image::NONE;

use crate::harness::Declarations;
use crate::memory::VariableStorage;
use crate::runtime::program_access::{Located, ProgramRead};
use crate::value::{array_offset_i64, Value};

#[cfg(test)]
mod tests;

/// A parsed member path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberPath {
    /// Program instance or global the path starts at.
    pub root: SmolStr,
    /// The steps after the root.
    pub segments: Vec<PathSegment>,
}

/// One step of a member path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSegment {
    /// A function block member or structure field.
    Field(SmolStr),
    /// An array element, one index per dimension.
    Index(Vec<i64>),
}

impl MemberPath {
    /// Parses a path written as identifiers separated by dots, each optionally followed by
    /// `[index]` or `[i, j]`.
    pub fn parse(path: &str) -> Result<Self, String> {
        if let Some((peer, local)) = path.split_once(':') {
            if peer.is_empty()
                || peer.len() > 128
                || !peer
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                || local.contains(':')
            {
                return Err(
                    "a remote binding needs one runtime identity and a local member path"
                        .to_string(),
                );
            }
            let mut parsed = Self::parse(local)?;
            parsed.root = format!("{peer}:{}", parsed.root).into();
            return Ok(parsed);
        }
        if path.trim().is_empty() {
            return Err("the path is empty".to_string());
        }
        let mut root = None;
        let mut segments = Vec::new();
        for (position, part) in path.split('.').enumerate() {
            let (name, indices) = split_part(part).map_err(|error| format!("`{path}`: {error}"))?;
            if position == 0 {
                root = Some(name);
            } else {
                segments.push(PathSegment::Field(name));
            }
            segments.extend(indices.into_iter().map(PathSegment::Index));
        }
        Ok(Self {
            root: root.expect("splitting a non-empty path yields a first part"),
            segments,
        })
    }
}

/// Splits `Name`, `Name[1]`, `Name[1, 2]` or `Name[1][2]` into the identifier and its indices.
fn split_part(part: &str) -> Result<(SmolStr, Vec<Vec<i64>>), String> {
    let part = part.trim();
    let name_end = part.find('[').unwrap_or(part.len());
    let name = part[..name_end].trim();
    if name.is_empty() {
        return Err("an identifier is missing".to_string());
    }
    if !is_identifier(name) {
        return Err(format!("`{name}` is not an identifier"));
    }
    let mut rest = &part[name_end..];
    let mut indices = Vec::new();
    while !rest.is_empty() {
        let Some(inside) = rest.strip_prefix('[') else {
            return Err(format!("unexpected `{rest}`"));
        };
        let Some(close) = inside.find(']') else {
            return Err("a `]` is missing".to_string());
        };
        let index = inside[..close]
            .split(',')
            .map(|value| {
                let value = value.trim();
                value
                    .parse::<i64>()
                    .map_err(|_| format!("`{value}` is not an array index"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        indices.push(index);
        rest = inside[close + 1..].trim_start();
    }
    Ok((SmolStr::new(name), indices))
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Where `path` lies in the running program: its first name a global or a program instance, then
/// function block members (inherited ones included), structure fields and array elements within
/// their declared bounds, matched without regard to case (IEC 61131-3 §6.1.2). `None` for a path
/// that names nothing, a remote path among them.
pub fn locate_member(path: &MemberPath, program: &dyn ProgramRead) -> Option<Located> {
    let image = program.program_image();
    // A path starts at a global or a program instance (§7.2), never at a program's variable by
    // its own name.
    let (_, root) = image.find_symbol(&path.root.to_ascii_uppercase())?;
    if root.parent != NONE || root.area == 0 {
        return None;
    }
    program.locate(&located_text(path)).ok()
}

/// The value at `path` in the running program; `None` for a path that names nothing, and for an
/// instance, which holds no single value.
pub fn member_value(path: &MemberPath, program: &dyn ProgramRead) -> Option<Value> {
    let at = locate_member(path, program)?;
    program.read_at(&at).ok()
}

/// A path as the program's tables resolve it: `Main.Pumps[2,1].Running`.
fn located_text(path: &MemberPath) -> String {
    let mut text = path.root.to_string();
    for segment in &path.segments {
        match segment {
            PathSegment::Field(name) => {
                text.push('.');
                text.push_str(name);
            }
            PathSegment::Index(indices) => {
                let list: Vec<String> = indices.iter().map(i64::to_string).collect();
                text.push('[');
                text.push_str(&list.join(","));
                text.push(']');
            }
        }
    }
    text
}

/// Resolves a path against the declarations' storage. Identifiers match without regard to case,
/// as in IEC 61131-3; array indices use the declared bounds.
pub fn resolve_member<'a>(path: &MemberPath, storage: &'a VariableStorage) -> Option<&'a Value> {
    resolve_member_with(path, storage, |_, _| {})
}

/// Resolves a path like [`resolve_member`] and reports each step that names a member of a
/// program or function block instance, with the instance's type name and the member's name.
pub fn resolve_member_with<'a>(
    path: &MemberPath,
    storage: &'a VariableStorage,
    mut on_instance_member: impl FnMut(&str, &str),
) -> Option<&'a Value> {
    let mut current = find_ignoring_case(storage.globals(), &path.root)?;
    for segment in &path.segments {
        current = match segment {
            PathSegment::Field(name) => match current {
                Value::Instance(id) => {
                    let (declaring, found) = instance_member(storage, *id, name)?;
                    on_instance_member(declaring.as_str(), name.as_str());
                    found
                }
                Value::Struct(structure) => find_ignoring_case(structure.fields(), name)?,
                _ => return None,
            },
            PathSegment::Index(indices) => element(current, indices)?,
        };
    }
    Some(current)
}

/// What a named part of a path names (specification 39, §7.8), so a rename can follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    /// The first part: a global, with the program or function block type when it is an instance.
    Root {
        /// The instance's type, for a program instance its program.
        instance_type: Option<SmolStr>,
    },
    /// A member of an instance, with the type that declares it, a base for an inherited member.
    Member {
        /// The declaring type, for example `Pump`.
        declaring_type: SmolStr,
    },
    /// A structure field, with the structure type.
    Field {
        /// The structure type.
        structure_type: SmolStr,
    },
}

/// One step per named part of a path (indices name no part) in the project's declarations, or
/// `None` where it does not resolve.
pub fn path_steps(path: &MemberPath, declarations: &Declarations) -> Option<Vec<PathStep>> {
    let storage = declarations.storage();
    let mut current = find_ignoring_case(storage.globals(), &path.root)?;
    // A program instance a configuration names is of its program's type, not of its own name.
    let program = declarations.metadata().program_type(&path.root);
    let root = match current {
        Value::Instance(id) => Some(*id),
        _ => None,
    };
    let instance_type = match (root, program) {
        (Some(_), Some(program)) => Some(program.clone()),
        (Some(id), None) => Some(storage.get_instance(id)?.type_name.clone()),
        (None, _) => None,
    };
    let mut steps = vec![PathStep::Root { instance_type }];
    for segment in &path.segments {
        current = match segment {
            PathSegment::Field(name) => match current {
                Value::Instance(id) => {
                    let (declaring, found) = instance_member(storage, *id, name)?;
                    let declaring_type = match program {
                        Some(program) if Some(*id) == root => program.clone(),
                        _ => declaring.clone(),
                    };
                    steps.push(PathStep::Member { declaring_type });
                    found
                }
                Value::Struct(structure) => {
                    steps.push(PathStep::Field {
                        structure_type: structure.type_name().clone(),
                    });
                    find_ignoring_case(structure.fields(), name)?
                }
                _ => return None,
            },
            PathSegment::Index(indices) => element(current, indices)?,
        };
    }
    Some(steps)
}

/// A member of an instance with the type that declares it. An inherited member lives in the
/// instance's base part (IEC 61131-3 Edition 3, 6.6.5.3), so the chain of parents is searched.
fn instance_member<'a>(
    storage: &'a VariableStorage,
    id: crate::memory::InstanceId,
    name: &str,
) -> Option<(&'a SmolStr, &'a Value)> {
    let mut instance = storage.get_instance(id)?;
    for _ in 0..32 {
        if let Some(found) = find_ignoring_case(&instance.variables, name) {
            return Some((&instance.type_name, found));
        }
        instance = storage.get_instance(instance.parent?)?;
    }
    None
}

fn element<'a>(value: &'a Value, indices: &[i64]) -> Option<&'a Value> {
    let Value::Array(array) = value else {
        return None;
    };
    array
        .elements()
        .get(array_offset_i64(array.dimensions(), indices)?)
}

/// Looks a name up the way IEC 61131-3 compares identifiers: without regard to case.
pub(crate) fn find_ignoring_case<'a, T>(
    map: &'a IndexMap<SmolStr, T>,
    name: &str,
) -> Option<&'a T> {
    map.get(name).or_else(|| {
        map.iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    })
}
