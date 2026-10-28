//! Binding checks of HMI pages against the compiled program (specification 39, §7.6).

use serde::Serialize;
use trust_hir::symbols::ParamDirection;

use super::catalogue::{Catalogue, ComponentDef};
use super::member_paths::{
    find_ignoring_case as find, resolve_member, resolve_member_with, MemberPath,
};
use super::page_files::{HmiBinding, HmiBindingDirection, HmiObject, HmiPage, MODE_NAMES};
use super::write_value::enumeration_values;
use crate::debug::dap::value_type_name;
use crate::memory::VariableStorage;
use crate::program_model::{FunctionBlockBase, VarDef};
use crate::runtime::RuntimeMetadata;
use crate::value::Value;

mod attachments;
mod faceplate_fields;
mod pairs;
mod placed;

pub use faceplate_fields::{check_faceplate, field_problems, without_fields};
pub use pairs::{block_named, check_pair, PairProblem, PairProgram};
pub use placed::check_placements;
mod pipes;

#[cfg(test)]
mod attachment_tests;
#[cfg(test)]
mod pair_tests;
#[cfg(test)]
mod pipe_tests;
#[cfg(test)]
mod tests;

/// One problem with an object of a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingProblem {
    /// Id of the object.
    pub object: String,
    /// The bound property, when the problem belongs to a binding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Plain description.
    pub message: String,
    /// The point a control writes that no rule of the write policy covers, which the editor's
    /// quick fix allows (specification 39, §12.2.3); none for any other problem.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point: Option<String>,
}

/// Checks every object and binding of a page.
pub fn check_page(
    page: &HmiPage,
    catalogue: &Catalogue,
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
) -> Vec<BindingProblem> {
    check_page_with_remote(
        page,
        catalogue,
        metadata,
        storage,
        &std::collections::BTreeMap::new(),
    )
}

/// Checks against explicitly supplied, fresh owner catalogue entries as well as local metadata.
pub fn check_page_with_remote(
    page: &HmiPage,
    catalogue: &Catalogue,
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
    remote: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        let Some(component) = catalogue.component(&object.kind) else {
            problems.push(problem(
                object,
                None,
                format!("the component type \"{}\" is not defined", object.kind),
            ));
            continue;
        };
        problems.extend(placed::check_placed(object, storage));
        if let Some(command) = &object.command {
            let checked = (|| -> Result<(), String> {
                if component.category != super::catalogue::ComponentCategory::Control {
                    return Err("only an operator control may declare a capability command".into());
                }
                let path = MemberPath::parse(&command.instance)?;
                let Some(Value::Instance(id)) = resolve_member(&path, storage) else {
                    return Err("command target is not a PLC instance".into());
                };
                let instance = storage
                    .get_instance(*id)
                    .ok_or("command instance is unavailable")?;
                super::commands::capability_checks::check(metadata, &instance.type_name, command)
            })();
            if let Err(message) = checked {
                problems.push(problem(object, Some("command"), message));
            }
        }
        for (property, binding) in &object.bind {
            if let Some(quality) = &binding.quality
                && let Err(message) = check_quality(quality, storage, remote)
            {
                problems.push(problem(object, Some(property), message));
            }
            if let Err(message) =
                check_direction(component, property, binding, object.command.is_some())
                    .and_then(|()| {
                        check_binding_with_remote(
                            component, property, binding, metadata, storage, remote,
                        )
                    })
                    .and_then(|()| check_choices(object, property, binding, metadata, storage))
            {
                problems.push(problem(object, Some(property), message));
            }
        }
    }
    problems.extend(pipes::check_pipes(
        page, catalogue, metadata, storage, remote,
    ));
    problems.extend(attachments::check_attachments(page, catalogue));
    problems
}

/// Drawing and binding checks with a declaration-only resolver, shared by the editor.
/// The callback owns type/access checks; no runtime, storage or PLC evaluation is required.
pub fn check_declared_page(
    page: &HmiPage,
    catalogue: &Catalogue,
    check: &dyn Fn(&ComponentDef, &str, &HmiBinding) -> Result<(), String>,
) -> Vec<BindingProblem> {
    let mut problems = Vec::new();
    for object in &page.objects {
        let Some(component) = catalogue.component(&object.kind) else {
            problems.push(problem(
                object,
                None,
                format!("the component type \"{}\" is not defined", object.kind),
            ));
            continue;
        };
        for (property, binding) in &object.bind {
            let checked = check_direction(component, property, binding, object.command.is_some())
                .and_then(|()| check(component, property, binding));
            if let Err(error) = checked {
                problems.push(problem(object, Some(property), error));
            }
        }
    }
    problems.extend(pipes::check_pipes_using(page, catalogue, check));
    problems.extend(attachments::check_attachments(page, catalogue));
    problems
}

fn problem(object: &HmiObject, property: Option<&str>, message: String) -> BindingProblem {
    BindingProblem {
        object: object.id.clone(),
        property: property.map(str::to_string),
        message,
        point: None,
    }
}

fn check_quality(
    quality: &str,
    storage: &VariableStorage,
    remote: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let path = MemberPath::parse(quality)?;
    let valid = if super::remote_values::remote_path(&path).is_some() {
        remote
            .get(&super::remote_values::qualified_key(quality))
            .and_then(|entry| entry["kinds"].as_array())
            .is_some_and(|kinds| {
                kinds
                    .iter()
                    .any(|kind| kind == "boolean" || kind == "enumeration")
            })
    } else {
        resolve_member(&path, storage)
            .is_some_and(|value| matches!(value, Value::Bool(_) | Value::Enum(_)))
    };
    if valid {
        Ok(())
    } else {
        Err(format!("quality {quality} must name a BOOL or quality enumeration with a fresh owner declaration"))
    }
}

/// Whether a binding's direction suits its property: one of the property's directions, or only
/// reading on a control that calls a method. Such a control writes nothing through its bindings;
/// its command calls the method, and its bindings show the program's values (§12.2).
pub(super) fn check_direction(
    component: &ComponentDef,
    property: &str,
    binding: &HmiBinding,
    calls_method: bool,
) -> Result<(), String> {
    let kind = &component.kind;
    let rule = component
        .property(property)
        .ok_or_else(|| format!("a {kind} has no property \"{property}\""))?;
    if rule.directions.contains(&binding.direction)
        || (calls_method && binding.direction == HmiBindingDirection::Read)
    {
        return Ok(());
    }
    let allowed = rule
        .directions
        .iter()
        .map(|direction| direction_words(*direction))
        .collect::<Vec<_>>()
        .join(" or ");
    Err(format!(
        "a {kind}'s {property} only {allowed}, but this binding {}",
        direction_words(binding.direction)
    ))
}

/// Whether the program has a binding's value with a kind its property takes; its direction is
/// [`check_direction`]'s.
pub(super) fn check_binding_with_remote(
    component: &ComponentDef,
    property: &str,
    binding: &HmiBinding,
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
    remote: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let kind = &component.kind;
    let rule = component
        .property(property)
        .ok_or_else(|| format!("a {kind} has no property \"{property}\""))?;
    let path = MemberPath::parse(&binding.path)
        .map_err(|error| format!("\"{}\" is not a valid path: {error}", binding.path))?;
    if super::remote_values::remote_path(&path).is_some() {
        let variable = remote
            .get(&super::remote_values::qualified_key(&binding.path))
            .ok_or_else(|| {
                format!(
                    "\"{}\" has no fresh owning-runtime publish declaration",
                    binding.path
                )
            })?;
        let kinds = variable["kinds"]
            .as_array()
            .ok_or("owner declaration has no value kinds")?;
        if !rule.accepts.iter().any(|kind| {
            serde_json::to_value(kind)
                .ok()
                .is_some_and(|kind| kinds.contains(&kind))
        }) {
            return Err(format!(
                "\"{}\" has incompatible published value kinds",
                binding.path
            ));
        }
        if binding.direction != HmiBindingDirection::Read && variable["writable"] != true {
            return Err(format!(
                "\"{}\" cannot be written on its owning runtime",
                binding.path
            ));
        }
        return Ok(());
    }
    let mut write_block = global_write_block(metadata, &path.root);
    let value = resolve_member_with(&path, storage, |type_name, member| {
        if write_block.is_none() {
            write_block = member_write_block(metadata, type_name, member);
        }
    })
    .ok_or_else(|| format!("\"{}\" is not a variable of the program", binding.path))?;
    if !rule.accepts.iter().any(|accepted| accepted.matches(value)) {
        let needed = rule
            .accepts
            .iter()
            .map(|accepted| accepted.words())
            .collect::<Vec<_>>()
            .join(" or ");
        let actual = value_type_name(value).unwrap_or_else(|| "a value".to_string());
        return Err(format!(
            "\"{}\" is {actual}; a {kind}'s {property} needs {needed}",
            binding.path
        ));
    }
    if binding.direction != HmiBindingDirection::Read
        && let Some(reason) = write_block
    {
        return Err(format!(
            "\"{}\" {reason} and cannot be written",
            binding.path
        ));
    }
    Ok(())
}

/// A selector's options are values of a bound enumeration, and a mode selector's enumeration
/// has the modes it offers (§5.8). A number target takes any options.
fn check_choices(
    object: &HmiObject,
    property: &str,
    binding: &HmiBinding,
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
) -> Result<(), String> {
    let own: Vec<&str> = object
        .options
        .iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let offered = match (object.kind.as_str(), property) {
        ("selector", "value") => own,
        ("mode-selector", "mode") if own.is_empty() => MODE_NAMES.to_vec(),
        ("mode-selector", "mode") => own,
        _ => return Ok(()),
    };
    let Ok(path) = MemberPath::parse(&binding.path) else {
        return Ok(());
    };
    let Some(Value::Enum(value)) = resolve_member(&path, storage) else {
        return Ok(());
    };
    let type_name = value.type_name();
    let registry = metadata.registry();
    let Some(values) = registry
        .lookup(type_name)
        .and_then(|id| enumeration_values(registry, id))
    else {
        return Ok(());
    };
    match offered.iter().find(|option| {
        !values
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(option))
    }) {
        Some(mode) if object.kind == "mode-selector" => Err(format!(
            "{type_name} has no value {mode}; list the modes it has in options"
        )),
        Some(option) => Err(format!("\"{option}\" is not a value of {type_name}")),
        None => Ok(()),
    }
}

fn direction_words(direction: HmiBindingDirection) -> &'static str {
    match direction {
        HmiBindingDirection::Read => "reads",
        HmiBindingDirection::Write => "writes",
        HmiBindingDirection::ReadWrite => "reads and writes",
    }
}

/// Why a global cannot be written, if it cannot: it is a constant. The variable picker uses the
/// same rule for the roots of its tree (§7.1).
pub(super) fn global_write_block(metadata: &RuntimeMetadata, name: &str) -> Option<String> {
    metadata
        .global_is_constant(name)
        .then(|| "is a constant".to_string())
}

/// Why a member of an instance cannot be written from outside it, if it cannot: it is a
/// constant, or an output of a function block (or of a block it extends). The variable picker
/// uses the same rule (§7.1).
pub(super) fn member_write_block(
    metadata: &RuntimeMetadata,
    type_name: &str,
    member: &str,
) -> Option<String> {
    let is_constant = |vars: &[VarDef]| {
        vars.iter()
            .any(|var| var.constant && var.name.eq_ignore_ascii_case(member))
    };
    if let Some(program) = find(metadata.program_templates(), type_name) {
        return is_constant(&program.vars).then(|| "is a constant".to_string());
    }
    let mut block = find(metadata.function_blocks(), type_name)?;
    // Inheritance chains are short; the bound only guards against a malformed cycle.
    for _ in 0..32 {
        if let Some(param) = block
            .params
            .iter()
            .find(|param| param.name.eq_ignore_ascii_case(member))
        {
            return (param.direction == ParamDirection::Out)
                .then(|| format!("is an output of {}", block.name));
        }
        if is_constant(&block.vars) {
            return Some("is a constant".to_string());
        }
        match &block.base {
            Some(FunctionBlockBase::FunctionBlock(base)) => {
                block = find(metadata.function_blocks(), base)?;
            }
            _ => return None,
        }
    }
    None
}
