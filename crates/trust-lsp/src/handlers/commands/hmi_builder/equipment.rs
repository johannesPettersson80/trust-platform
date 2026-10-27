//! Equipment objects: the palette, bindings, instances and a loop's signal lines (specification
//! 39, §6 item 5, §8.5, §18 phase 4 item 1, §21.3; rows H9, H12). An equipment object shows one
//! instance of a function block through its component definition (`hmi/pairs/`); the editor's
//! catalogue carries the definitions as `equipment`.
//!
//! - `palette { catalogue, pid? }` → `{ palette }`: the Components panel's entries, each
//!   definition with its drawing first, then the components a definition does not map; in a
//!   P&ID only what it may hold (process objects, texts and lines).
//! - `definitions { equipment, blocks }` → `{ definitions }`: by block as an object records it,
//!   the definition of that block, names compared without regard to case.
//! - `instances { block, instance?, variables }` → `{ declared, current, programs, program,
//!   name }`: what the inspector offers for an object of the block.
//! - `place { object, definition, variables, page }` → `{ object }`: a new object of a
//!   definition, bound to the first free instance when the project has one program.
//! - `bind { object, definition, instance? | program, name?, variables, page }` → `{ object }`:
//!   the object bound to an instance, or to a name in a program: the name given, else the
//!   object's own name, else the program's first free name; `{ refused }` when it names no
//!   instance.
//! - `loopSignals { object, page, dependencies }` → `{ object }` when a loop controller's signal
//!   lines change to follow its connected equipment, else `{}`.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Deserialize;
use serde_json::{json, Map, Value};
use trust_runtime::hmi::catalogue::{ComponentCategory, ComponentDef};
use trust_runtime::hmi::page_files::{
    pair_member_path, HmiBinding, HmiBindingDirection, HmiClick, HmiObject,
};

use super::decode;
use super::variables::{every_variable, VariableNode};

/// How many numbers a new instance's name tries before it gives up.
const MOST_NUMBERS: u32 = 10_000;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Request {
    Palette {
        catalogue: CatalogueParts,
        #[serde(default)]
        pid: bool,
    },
    Definitions {
        #[serde(default)]
        equipment: Vec<Value>,
        #[serde(default)]
        blocks: Vec<String>,
    },
    Instances {
        block: String,
        #[serde(default)]
        instance: Option<String>,
        #[serde(default)]
        variables: Vec<VariableNode>,
    },
    Place {
        object: Box<HmiObject>,
        definition: Definition,
        #[serde(default)]
        variables: Vec<VariableNode>,
        #[serde(default)]
        page: PageObjects,
    },
    Bind {
        object: Box<HmiObject>,
        definition: Definition,
        #[serde(default)]
        instance: Option<String>,
        #[serde(default)]
        program: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        variables: Vec<VariableNode>,
        #[serde(default)]
        page: PageObjects,
    },
    LoopSignals {
        object: Box<HmiObject>,
        #[serde(default)]
        page: PageObjects,
        #[serde(default)]
        dependencies: Vec<Dependency>,
    },
}

/// The editor's catalogue: the components and the equipment definitions.
#[derive(Deserialize)]
struct CatalogueParts {
    #[serde(default)]
    components: Vec<ComponentDef>,
    #[serde(default)]
    equipment: Vec<Value>,
}

/// What the rules read of an equipment definition (§8.5).
#[derive(Deserialize)]
struct Definition {
    /// The function block type with its namespace, `Plant.FB_Pump`.
    block: String,
    /// The panel it opens.
    panel: String,
    /// Symbol properties and the members they show.
    #[serde(default)]
    bindings: BTreeMap<String, String>,
    /// The quality member of a bound property.
    #[serde(default)]
    quality: BTreeMap<String, String>,
}

/// The objects of a page, as far as the rules read them.
#[derive(Default, Deserialize)]
struct PageObjects {
    #[serde(default)]
    object: Vec<PageObject>,
}

#[derive(Deserialize)]
struct PageObject {
    id: String,
    #[serde(default)]
    instance: Option<String>,
}

/// A loop controller's connected equipment as its details give it (§21.3 item 2).
#[derive(Deserialize)]
struct Dependency {
    /// It drives an input of the equipment: an output, not a measurement.
    #[serde(default)]
    drives: Option<bool>,
    /// The instance it is connected to.
    #[serde(default)]
    value: Option<String>,
}

/// The answer of `trust-lsp.hmiEquipmentEdit`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Request>(argument)? {
        Request::Palette { catalogue, pid } => {
            let entries = palette(&catalogue, pid)?;
            Ok(json!({ "ok": true, "palette": entries }))
        }
        Request::Definitions { equipment, blocks } => {
            Ok(json!({ "ok": true, "definitions": definitions(&equipment, &blocks) }))
        }
        Request::Instances {
            block,
            instance,
            variables,
        } => Ok(instances(&block, instance.as_deref(), &variables)),
        Request::Place {
            object,
            definition,
            variables,
            page,
        } => {
            let object = match new_instance(&definition.block, &variables, &page) {
                Some(instance) => bind_equipment(*object, &definition, &instance),
                None => HmiObject {
                    block: Some(definition.block.clone()),
                    faceplate: Some(definition.panel.clone()),
                    ..*object
                },
            };
            Ok(json!({ "ok": true, "object": object }))
        }
        Request::Bind {
            object,
            definition,
            instance,
            program,
            name,
            variables,
            page,
        } => {
            let instance = match (instance, program) {
                (Some(instance), _) => instance,
                (None, Some(program)) => {
                    let current = object.instance.as_deref();
                    match named(
                        &definition.block,
                        &program,
                        name.as_deref(),
                        current,
                        &variables,
                        &page,
                    ) {
                        Ok(instance) => instance,
                        Err(reason) => return Ok(json!({ "ok": true, "refused": reason })),
                    }
                }
                (None, None) => return Err("bind needs an instance or a program.".into()),
            };
            if !is_instance_path(&instance) {
                return Ok(json!({
                    "ok": true,
                    "refused": format!("\"{instance}\" is not an instance of a program"),
                }));
            }
            Ok(json!({ "ok": true, "object": bind_equipment(*object, &definition, &instance) }))
        }
        Request::LoopSignals {
            object,
            page,
            dependencies,
        } => Ok(match with_loop_signals(*object, &page, &dependencies) {
            Some(object) => json!({ "ok": true, "object": object }),
            None => json!({ "ok": true }),
        }),
    }
}

/// Whether two names of the program are the same; IEC names do not depend on case.
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// `palette`: each definition with its drawing, as the definition's block; then every component
/// whose drawing no definition maps, and the components that are not equipment. Unmapped artwork
/// stays usable, but is never offered as a block of the PLC (§21.3 item 1).
fn palette(catalogue: &CatalogueParts, pid: bool) -> Result<Vec<Value>, String> {
    // A P&ID holds process objects, texts and lines (§18 phase 4 item 1).
    let components: Vec<&ComponentDef> = catalogue
        .components
        .iter()
        .filter(|component| {
            !pid || component.category == ComponentCategory::Equipment
                || component.kind == "text"
                || component.kind == "line"
        })
        .collect();
    let mapped: BTreeSet<&str> = catalogue
        .equipment
        .iter()
        .filter_map(|definition| definition["symbol"].as_str())
        .collect();
    let value = |component: &ComponentDef| {
        serde_json::to_value(component).map_err(|error| format!("Invalid catalogue: {error}."))
    };
    let mut entries = Vec::new();
    for definition in &catalogue.equipment {
        let symbol = definition["symbol"].as_str();
        let Some(drawing) = components
            .iter()
            .copied()
            .find(|component| Some(component.kind.as_str()) == symbol)
        else {
            continue;
        };
        let mut entry = value(drawing)?;
        entry["equipment"] = definition.clone();
        entry["category"] = json!("equipment");
        entries.push(entry);
    }
    for component in components {
        if !mapped.contains(component.kind.as_str())
            || component.category != ComponentCategory::Equipment
        {
            entries.push(value(component)?);
        }
    }
    Ok(entries)
}

/// `definitions`: for each block an object records, the definition of that block.
fn definitions(equipment: &[Value], blocks: &[String]) -> Map<String, Value> {
    blocks
        .iter()
        .filter_map(|block| {
            equipment
                .iter()
                .find(|definition| {
                    definition["block"]
                        .as_str()
                        .is_some_and(|defined| same_name(defined, block))
                })
                .map(|definition| (block.clone(), definition.clone()))
        })
        .collect()
}

/// The programs equipment can run in: every program but a screen service program, one that
/// holds an `Hmi.Display` (§18 phase 5).
fn equipment_programs(variables: &[VariableNode]) -> Vec<&VariableNode> {
    variables
        .iter()
        .filter(|node| {
            node.kind == "program"
                && !every_variable(&node.children).iter().any(|member| {
                    member.kind == "function-block" && same_name(&member.type_name, "Hmi.Display")
                })
        })
        .collect()
}

/// `instances`: the declared instances of the block, whether the object's instance is one of
/// them, the programs, and the program and name of the object's instance.
fn instances(block: &str, instance: Option<&str>, variables: &[VariableNode]) -> Value {
    let declared: Vec<&str> = every_variable(variables)
        .into_iter()
        .filter(|node| {
            node.kind == "function-block" && same_name(&node.type_name, block) && !node.pending
        })
        .map(|node| node.path.as_str())
        .collect();
    let current =
        instance.is_some_and(|instance| declared.iter().any(|path| same_name(path, instance)));
    let programs = equipment_programs(variables);
    let (prefix, name) = match instance {
        Some(instance) => match instance.rsplit_once('.') {
            Some((prefix, name)) => (prefix, name),
            None => ("", instance),
        },
        None => ("", ""),
    };
    let program = if !prefix.is_empty() {
        prefix
    } else {
        match programs.as_slice() {
            [only] => only.path.as_str(),
            [] => "Main",
            _ => "",
        }
    };
    json!({
        "ok": true,
        "declared": declared,
        "current": current,
        "programs": programs
            .iter()
            .map(|node| json!({ "path": node.path, "name": node.name }))
            .collect::<Vec<_>>(),
        "program": program,
        "name": name,
    })
}

/// The first free instance of a program for a block: the block's name without `FB_` and the
/// first number from 1 that no variable of the program and no object of the page holds, without
/// regard to case (`Main.Pump2` after `Main.Pump1`).
fn free_instance(
    block: &str,
    variables: &[VariableNode],
    page: &PageObjects,
    program: &str,
) -> Option<String> {
    let last = block.rsplit('.').next().unwrap_or(block);
    let prefix = last.strip_prefix("FB_").unwrap_or(last);
    let used: HashSet<String> = every_variable(variables)
        .into_iter()
        .map(|node| node.path.to_lowercase())
        .chain(
            page.object
                .iter()
                .filter_map(|object| object.instance.as_deref())
                .map(str::to_lowercase),
        )
        .collect();
    (1..=MOST_NUMBERS)
        .map(|number| format!("{program}.{prefix}{number}"))
        .find(|instance| !used.contains(&instance.to_lowercase()))
}

/// A new object's instance: the first free one of the project's program, or of `Main` while it
/// has none; none when it has several, for the user to choose.
fn new_instance(block: &str, variables: &[VariableNode], page: &PageObjects) -> Option<String> {
    match equipment_programs(variables).as_slice() {
        [] => free_instance(block, variables, page, "Main"),
        [only] => free_instance(block, variables, page, &only.path),
        _ => None,
    }
}

/// The instance an object is given in a program: the name typed; without one its own name, so
/// moving it to another program keeps it; else the program's first free name, so two new objects
/// never share an instance.
fn named(
    block: &str,
    program: &str,
    name: Option<&str>,
    current: Option<&str>,
    variables: &[VariableNode],
    page: &PageObjects,
) -> Result<String, String> {
    if let Some(name) = name {
        return if is_name(name) {
            Ok(format!("{program}.{name}"))
        } else {
            Err(format!("\"{name}\" is not a name"))
        };
    }
    let own = current
        .and_then(|instance| instance.rsplit('.').next())
        .filter(|own| !own.is_empty());
    if let Some(own) = own {
        return Ok(format!("{program}.{own}"));
    }
    let free = free_instance(block, variables, page, program)
        .unwrap_or_else(|| format!("{program}.Equipment1"));
    let last = free.rsplit('.').next().unwrap_or(&free);
    Ok(format!("{program}.{last}"))
}

/// An IEC identifier: a letter or `_`, then letters, digits and `_`.
fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// An instance of a program: two or more names joined by dots, `Main.P101`.
fn is_instance_path(text: &str) -> bool {
    text.split('.').count() >= 2 && text.split('.').all(is_name)
}

/// The object bound to an instance through its definition (§8.5): each symbol property reads its
/// member, `{instance}_…` naming a variable beside the instance; the object records the block,
/// the instance, its panel and, as its tag, the instance's name; a click that opens a panel opens
/// the definition's, for the instance. Bindings of the object that the definition does not name
/// are left out.
fn bind_equipment(mut object: HmiObject, definition: &Definition, instance: &str) -> HmiObject {
    object.bind = definition
        .bindings
        .iter()
        .map(|(property, member)| {
            let quality = definition
                .quality
                .get(property)
                .filter(|member| !member.is_empty())
                .map(|member| pair_member_path(instance, member));
            let binding = HmiBinding {
                quality,
                path: pair_member_path(instance, member),
                unit: None,
                range: None,
                direction: HmiBindingDirection::Read,
            };
            (property.clone(), binding)
        })
        .collect();
    if let Some(HmiClick::Faceplate {
        name,
        instance: opened,
    }) = &mut object.click
        && !name.is_empty()
    {
        *name = definition.panel.clone();
        *opened = Some(instance.to_string());
    }
    object.block = Some(definition.block.clone());
    object.instance = Some(instance.to_string());
    object.faceplate = Some(definition.panel.clone());
    object.tag = Some(instance.rsplit('.').next().unwrap_or(instance).to_string());
    object
}

/// The ids of the page objects a loop controller's lines go to (§6 item 5): `measures` to the
/// object that shows its first connected equipment that drives nothing, its measurement, and
/// `acts_on` to the one that shows its first that drives an input; none where the page shows no
/// such instance.
fn loop_signals(page: &PageObjects, dependencies: &[Dependency]) -> [Option<String>; 2] {
    let shown = |drives: bool| {
        let path = dependencies
            .iter()
            .find(|dependency| {
                dependency.drives.unwrap_or(false) == drives
                    && dependency
                        .value
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
            })
            .and_then(|dependency| dependency.value.as_deref())?;
        page.object
            .iter()
            .find(|object| {
                object
                    .instance
                    .as_deref()
                    .is_some_and(|instance| same_name(instance, path))
            })
            .map(|object| object.id.clone())
    };
    [shown(false), shown(true)]
}

/// The controller with its lines as its configuration has them, or none when they already are.
fn with_loop_signals(
    mut object: HmiObject,
    page: &PageObjects,
    dependencies: &[Dependency],
) -> Option<HmiObject> {
    let [measures, acts_on] = loop_signals(page, dependencies);
    if object.measures == measures && object.acts_on == acts_on {
        return None;
    }
    object.measures = measures;
    object.acts_on = acts_on;
    Some(object)
}

#[cfg(test)]
#[path = "equipment_tests.rs"]
mod tests;
