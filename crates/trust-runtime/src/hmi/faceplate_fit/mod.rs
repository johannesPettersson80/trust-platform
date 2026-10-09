//! Whether a faceplate fits an instance (specification 39, §8.9 items 1 and 2): the instance's
//! block has every field the faceplate needs, each as the member the faceplate's pair for the
//! block names, else as the member of the field's own name, holding a kind of value the field
//! takes. A placed faceplate may be bound to any instance it fits.

use crate::hmi::catalogue::ValueKind;
use crate::hmi::instance_walk::instances_where;
use crate::hmi::member_paths::{resolve_member, MemberPath};
use std::collections::BTreeMap;

use crate::hmi::page_files::{
    pair_member_path, HmiFaceplate, HmiField, HmiObject, HmiPair, PLACED_FACEPLATE,
};
use crate::memory::VariableStorage;
use crate::value::Value;

/// Why the instance at `instance` cannot show the faceplate, one reason per field it lacks or
/// holds as another kind of value; empty when it fits. `pairs` are the faceplate's pairs.
pub fn misfits(
    faceplate: &HmiFaceplate,
    pairs: &[HmiPair],
    instance: &str,
    storage: &VariableStorage,
) -> Vec<String> {
    let Some(Value::Instance(id)) = member(instance, storage) else {
        return vec![format!("\"{instance}\" is not an instance of the program")];
    };
    let type_name = storage
        .get_instance(*id)
        .map(|data| data.type_name.to_string());
    let members = choose_pair(pairs, type_name.as_deref()).map(|pair| &pair.fields);
    faceplate
        .fields
        .iter()
        .filter_map(|(field, rule)| {
            let name = members
                .and_then(|fields| fields.get(field))
                .map_or(field.as_str(), String::as_str);
            let path = pair_member_path(instance, name);
            match member(&path, storage) {
                None => Some(format!(
                    "{instance} has no member {name} for the field {field}"
                )),
                Some(value) if !takes(rule, value) => Some(format!(
                    "{path} is not {} for the field {field}",
                    kinds(rule)
                )),
                Some(_) => None,
            }
        })
        .collect()
}

/// The function block instances of the program the faceplate fits, in declaration order, at
/// most `limit`; `is_program` tells program instances, which are not offered, from blocks.
pub fn fitting_instances(
    faceplate: &HmiFaceplate,
    pairs: &[HmiPair],
    runtime: &crate::Runtime,
    is_program: &dyn Fn(&str) -> bool,
    limit: usize,
) -> Vec<String> {
    let storage = runtime.storage();
    let holdings = crate::hmi::instance_holdings::Holdings::new(
        runtime.function_blocks(),
        runtime.classes(),
        runtime.programs(),
        runtime.registry(),
    );
    let takes = |path: &str, id| {
        storage
            .get_instance(id)
            .is_some_and(|data| !is_program(data.type_name.as_str()))
            && misfits(faceplate, pairs, path, storage).is_empty()
    };
    instances_where(storage, &holdings, limit, is_program, &takes)
        .into_iter()
        .map(|found| found.path)
        .collect()
}

/// A placed faceplate bound to its instance as its popup is (§8.9 item 3): each `$Field` binding
/// the instance's member by the pair for the instance's type, and the header naming the instance
/// by the object's tag, else by the instance's own name, with the faceplate's description, or
/// else its title, below. `None` for an object that is no placed faceplate.
pub fn placed_faceplate(
    faceplate: HmiFaceplate,
    pairs: &[HmiPair],
    object: &HmiObject,
    instance_type: Option<&str>,
) -> Option<HmiFaceplate> {
    if object.kind != PLACED_FACEPLATE {
        return None;
    }
    let Some(instance) = object.instance.as_ref() else {
        return (!requires_instance(&faceplate)).then_some(faceplate);
    };
    let members = choose_pair(pairs, instance_type)
        .map(|pair| pair.fields.clone())
        .unwrap_or_default();
    let mut faceplate = bind_fields(faceplate, instance, &members);
    let tag = object
        .tag
        .clone()
        .filter(|tag| !tag.trim().is_empty())
        .unwrap_or_else(|| own_name(instance).to_string());
    name_header(&mut faceplate, tag);
    Some(faceplate)
}

/// Whether a reusable panel requires a selected equipment instance.
pub fn requires_instance(faceplate: &HmiFaceplate) -> bool {
    faceplate.settings
        || !faceplate.fields.is_empty()
        || faceplate.popup.page.objects.iter().any(|object| {
            object
                .command
                .as_ref()
                .is_some_and(|command| command.names_panel_instance())
        })
}

/// The header names the equipment by `tag` (§8.4), with the faceplate's description, or else its
/// title, below.
pub fn name_header(faceplate: &mut HmiFaceplate, tag: String) {
    let title = std::mem::replace(&mut faceplate.popup.page.title, tag);
    faceplate.description.get_or_insert(title);
}

/// The faceplate with each `$Field` binding made the instance's member: the member `members`
/// names for it, else the member of the field's own name; a member `{instance}_…` names the
/// variable beside the instance (§8.5).
pub fn bind_fields(
    mut faceplate: HmiFaceplate,
    instance: &str,
    members: &BTreeMap<String, String>,
) -> HmiFaceplate {
    faceplate.settings_instance = faceplate.settings.then(|| instance.to_owned());
    let resolve = |path: &mut String| {
        if let Some(field) = path.strip_prefix('$') {
            let member = members.get(field).map(String::as_str).unwrap_or(field);
            *path = pair_member_path(instance, member);
        }
    };
    for object in &mut faceplate.popup.page.objects {
        if let Some(command) = &mut object.command {
            // `$instance`, or `$instance.<member>` for a block inside it (§21.2 item 1).
            if command.instance == "$instance" {
                command.instance = instance.to_owned();
            } else if let Some(member) = command.instance.strip_prefix("$instance.") {
                command.instance = format!("{instance}.{member}");
            }
        }
        object.bind.values_mut().for_each(|binding| {
            resolve(&mut binding.path);
            if let Some(quality) = &mut binding.quality {
                resolve(quality);
            }
        });
    }
    for pipe in &mut faceplate.popup.page.pipes {
        pipe.bind.values_mut().for_each(|binding| {
            resolve(&mut binding.path);
            if let Some(quality) = &mut binding.quality {
                resolve(quality);
            }
        });
    }
    faceplate
}

/// The last part of an instance path, `P101` of `Main.P101`.
pub fn own_name(instance: &str) -> &str {
    instance.rsplit('.').next().unwrap_or(instance)
}

/// The pair for an instance's type, else the only pair. A qualified block names its own type; one
/// without a namespace may name a type in one, as the link check reads it (§8.6).
pub fn choose_pair<'a>(pairs: &'a [HmiPair], instance_type: Option<&str>) -> Option<&'a HmiPair> {
    let short = |name: &str| name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    if let Some(found) = instance_type.and_then(|kind| {
        pairs
            .iter()
            .find(|pair| pair.block.eq_ignore_ascii_case(kind))
            .or_else(|| {
                pairs
                    .iter()
                    .find(|pair| !pair.block.contains('.') && short(&pair.block) == short(kind))
            })
    }) {
        return Some(found);
    }
    (pairs.len() == 1).then(|| &pairs[0])
}

fn member<'a>(path: &str, storage: &'a VariableStorage) -> Option<&'a Value> {
    resolve_member(&MemberPath::parse(path).ok()?, storage)
}

/// The kinds a field takes; any when it names none, or a kind truST does not know.
fn kinds_of(rule: &HmiField) -> Vec<ValueKind> {
    rule.accepts
        .iter()
        .filter_map(|kind| serde_json::from_value(serde_json::Value::String(kind.clone())).ok())
        .collect()
}

/// Check a faceplate field against declared value kinds without creating runtime values.
/// An empty accepted-kind list has the same wildcard meaning as a live faceplate field.
pub fn accepts_declared_kinds(rule: &HmiField, actual: &[ValueKind]) -> bool {
    let expected = kinds_of(rule);
    expected.is_empty() || expected.iter().any(|kind| actual.contains(kind))
}

fn takes(rule: &HmiField, value: &Value) -> bool {
    let kinds = kinds_of(rule);
    kinds.is_empty() || kinds.iter().any(|kind| kind.matches(value))
}

fn kinds(rule: &HmiField) -> String {
    kinds_of(rule)
        .iter()
        .map(|kind| kind.words())
        .collect::<Vec<_>>()
        .join(" or ")
}

#[cfg(test)]
mod tests;

/// The instance whose members an object's bindings name: the common part of their paths, ending
/// before the last member.
pub fn common_instance(object: &HmiObject) -> Option<String> {
    let mut parents = object.bind.values().map(|binding| {
        let mut segments: Vec<&str> = binding.path.split('.').collect();
        segments.pop();
        segments
    });
    let first = parents.next()?;
    let common = parents.fold(first, |common, parent| {
        common
            .iter()
            .zip(&parent)
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .map(|(a, _)| *a)
            .collect()
    });
    (!common.is_empty()).then(|| common.join("."))
}
