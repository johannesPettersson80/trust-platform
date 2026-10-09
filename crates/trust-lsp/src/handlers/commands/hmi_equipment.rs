//! The builder's inspector of an equipment instance (specification 39, §21.3 items 2 and 3): its
//! definition's settings with their real values, its hardware terminals and status, and the
//! instances it depends on; and the change of these choices. A change is one workspace edit to
//! `plant.toml` and, for an instance in the generator's regions, to those regions, so its
//! declaration stays the truth. Nothing is compiled or run.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{TextDocumentIdentifier, Url};
use trust_runtime::hmi::page_files::{self as native, HmiDependency, HmiPair, HmiScalar};
use trust_runtime::hmi::variable_tree::VariableNode;

use super::hmi_authoring::{declarations::Declarations, equipment_settings, io};
use super::hmi_files::{refused, single_argument};
use super::hmi_plant_project::{read_design, PlantDesign, PLANT_FILE};
use super::{HmiBindingsCommandArgs, ProjectInputs, HMI_EQUIPMENT_COMMAND};
use crate::handlers::context::ServerContext;
use crate::handlers::lsp_utils::offset_to_position;

/// The most entries one change may hold.
const MAX_CHANGES: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    open: BTreeMap<String, String>,
    instance: String,
    #[serde(default)]
    changes: Option<Changes>,
    /// The page being edited, unsaved.
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    root_uri: Option<Url>,
    #[serde(default)]
    text_document: Option<TextDocumentIdentifier>,
    /// `plant.toml` as the editor has it.
    #[serde(default)]
    manifest: Option<String>,
    /// The block of an instance just placed, before its page is saved (§21.3 item 3).
    #[serde(default)]
    block: Option<String>,
}

/// Changed choices; an empty text or `null` removes a choice.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Changes {
    /// Settings in their units, by member.
    #[serde(default)]
    settings: BTreeMap<String, Value>,
    /// Chosen instances, by dependency member.
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    /// Chosen PLC signals, by terminal member.
    #[serde(default)]
    signals: BTreeMap<String, String>,
}

/// An instance as the inspector sees it.
struct Selected<'a> {
    instance: &'a str,
    block: String,
    definition: Option<HmiPair>,
    /// Declared in PLC code.
    declared: bool,
    /// Its choices are the builder's: not declared yet, or declared in the generator's regions.
    owned: bool,
}

pub(super) fn inspect<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    inspect_result(context, args).unwrap_or_else(refused)
}

fn inspect_result<C: ServerContext>(context: &C, args: Vec<Value>) -> Result<Value, String> {
    let args: Args = single_argument(args, HMI_EQUIPMENT_COMMAND)?;
    let mut open = args.open;
    if let (Some(file), Some(text)) = (args.file, args.text) {
        open.insert(file, text);
    }
    if let Some(manifest) = args.manifest {
        open.insert(PLANT_FILE.into(), manifest);
    }
    let selection = HmiBindingsCommandArgs {
        open,
        root_uri: args.root_uri,
        text_document: args.text_document,
    };
    let inputs = super::project_inputs(context, &selection, HMI_EQUIPMENT_COMMAND)?;
    let index = Declarations::read(&inputs.sources);
    let (design, design_text) = read_design(&inputs.root, &inputs.open)?;
    let definitions = super::hmi_program_edits::project_pairs_edited(&inputs.root, &inputs.open);
    let selected = select(
        &inputs,
        index.as_ref(),
        &design,
        &definitions,
        &args.instance,
        args.block,
    )?;
    match args.changes {
        Some(changes) => apply(
            &inputs,
            index.as_ref(),
            design,
            design_text,
            &definitions,
            &selected,
            changes,
        ),
        None => Ok(describe(&inputs, index.as_ref(), &design, &selected)),
    }
}

/// The instance, its block and definition, and who owns its choices.
fn select<'a>(
    inputs: &ProjectInputs,
    index: &Declarations,
    design: &PlantDesign,
    definitions: &[HmiPair],
    instance: &'a str,
    placed: Option<String>,
) -> Result<Selected<'a>, String> {
    let declared = index.resolve(instance).map(|node| node.type_name.clone());
    let block = declared
        .clone()
        .or_else(|| design.entry(instance).map(|entry| entry.block.clone()))
        .or_else(|| shown_block(inputs, instance))
        .or(placed.filter(|block| super::hmi_plant_project::is_type_name(block)))
        .ok_or("Select a component or a declared PLC equipment instance")?;
    let definition = super::hmi_generate::definition_of(definitions, &block).cloned();
    let owned = declared.is_none() || super::hmi_generate::generated(inputs, index, instance);
    Ok(Selected {
        instance,
        block,
        definition,
        declared: declared.is_some(),
        owned,
    })
}

/// The block a page records for an instance not declared yet (§21.1 item 2).
fn shown_block(inputs: &ProjectInputs, instance: &str) -> Option<String> {
    let bundle = super::hmi_authoring::bundle::read_edited(&inputs.root, &inputs.open).ok()?;
    bundle
        .files
        .iter()
        .filter(|(file, _)| file.starts_with("hmi/pages/") && file.ends_with(".toml"))
        .filter_map(|(file, bytes)| native::parse_page(file, &String::from_utf8_lossy(bytes)).ok())
        .flat_map(|page| page.objects)
        .find(|object| {
            object
                .instance
                .as_deref()
                .is_some_and(|own| own.eq_ignore_ascii_case(instance))
        })
        .and_then(|object| object.block)
}

fn flatten<'a>(nodes: &'a [VariableNode], result: &mut Vec<&'a VariableNode>) {
    for node in nodes {
        result.push(node);
        flatten(&node.children, result);
    }
}

/// The definition's initial values, when the builder still writes them.
fn initial_of(selected: &Selected<'_>) -> BTreeMap<String, HmiScalar> {
    match (&selected.definition, selected.declared) {
        (Some(definition), false) => definition
            .program
            .as_ref()
            .map(|program| program.initial.clone())
            .unwrap_or_default(),
        _ => BTreeMap::new(),
    }
}

/// A definition's dependencies by member, in order: a member may have several entries, one for
/// each interface it can drive (a controller's output drives a position, a power or a speed).
pub(super) fn alternatives(dependencies: &[HmiDependency]) -> Vec<Vec<&HmiDependency>> {
    let mut grouped: Vec<Vec<&HmiDependency>> = Vec::new();
    for dependency in dependencies {
        match grouped
            .iter_mut()
            .find(|entries| entries[0].member.eq_ignore_ascii_case(&dependency.member))
        {
            Some(entries) => entries.push(dependency),
            None => grouped.push(vec![dependency]),
        }
    }
    grouped
}

/// The instances that may be chosen for a dependency: of the same program, implementing its
/// interface and, when it drives an input, having that input.
fn candidates(
    inputs: &ProjectInputs,
    index: &Declarations,
    selected: &Selected<'_>,
    interface: &str,
    drives: Option<&str>,
) -> Vec<Value> {
    let Ok(variables) = super::hmi_authoring::design_variables::variables(inputs, index) else {
        return Vec::new();
    };
    let mut nodes = Vec::new();
    flatten(&variables, &mut nodes);
    let program = selected.instance.split('.').next().unwrap_or_default();
    let mut seen = BTreeSet::new();
    nodes
        .into_iter()
        .filter(|node| {
            let mut parts = node.path.split('.');
            matches!((parts.next(), parts.next(), parts.next()), (Some(root), Some(_), None) if root.eq_ignore_ascii_case(program))
        })
        .filter(|node| !node.path.eq_ignore_ascii_case(selected.instance))
        .filter(|node| index.implements_capability(&node.type_name, interface))
        .filter(|node| {
            drives.is_none_or(|input| {
                index.block_members(&node.type_name).is_some_and(|members| {
                    members.iter().any(|(member, _)| member.eq_ignore_ascii_case(input))
                })
            })
        })
        .filter(|node| seen.insert(node.path.to_ascii_uppercase()))
        .map(|node| json!({"path": node.path, "type": node.type_name}))
        .collect()
}

/// The values known for an instance, as ST literals: what decides whether a terminal is needed.
fn known_values(
    index: &Declarations,
    selected: &Selected<'_>,
    design: &PlantDesign,
) -> BTreeMap<String, String> {
    let mut known = equipment_settings::instance_initializers(index, selected.instance);
    if !selected.declared {
        for (member, value) in initial_of(selected) {
            let kind = equipment_settings::kind_of(index, &selected.block, &member);
            known.insert(member, equipment_settings::scalar_literal(kind, &value));
        }
        if let Some(entry) = design.entry(selected.instance) {
            known.extend(entry.settings.clone());
        }
    }
    known
}

fn describe(
    inputs: &ProjectInputs,
    index: &Declarations,
    design: &PlantDesign,
    selected: &Selected<'_>,
) -> Value {
    let bindings = io::bindings(index, &inputs.sources);
    let empty = PlantDesign::default();
    // A declared instance shows its declaration (§21.3 item 2); a new one the design's choices.
    let entry = (!selected.declared)
        .then(|| design.entry(selected.instance))
        .flatten();
    let choices = if selected.owned { design } else { &empty };
    let initial = initial_of(selected);
    let title = selected
        .definition
        .as_ref()
        .and_then(|definition| definition.title.clone())
        .unwrap_or_else(|| {
            selected
                .block
                .rsplit('.')
                .next()
                .unwrap_or(&selected.block)
                .trim_start_matches("FB_")
                .to_string()
        });
    let Some(definition) = &selected.definition else {
        return json!({"ok": true, "instance": selected.instance, "block": selected.block, "title": title,
            "declared": selected.declared, "owned": selected.owned, "settings": [], "dependencies": [],
            "terminals": [], "hardware": "none", "bindings": []});
    };
    let settings = equipment_settings::describe(
        index,
        selected.instance,
        &selected.block,
        &definition.settings,
        entry,
        &initial,
    );
    let terminals = io::describe(
        index,
        &bindings,
        choices,
        selected.instance,
        &selected.block,
        &definition.io,
        &known_values(index, selected, design),
    );
    let chosen = choices.entry(selected.instance);
    let dependencies: Vec<Value> = if selected.owned {
        alternatives(&definition.dependencies)
            .into_iter()
            .map(|entries| {
                let dependency = entries[0];
                let value = chosen.and_then(|entry| {
                    entry
                        .dependencies
                        .iter()
                        .find(|(member, _)| member.eq_ignore_ascii_case(&dependency.member))
                        .map(|(_, target)| target.clone())
                });
                let mut seen = BTreeSet::new();
                let choices: Vec<Value> = entries
                    .iter()
                    .flat_map(|entry| {
                        candidates(
                            inputs,
                            index,
                            selected,
                            &entry.interface,
                            entry.drives.as_deref(),
                        )
                    })
                    .filter(|choice| {
                        seen.insert(
                            choice["path"]
                                .as_str()
                                .unwrap_or_default()
                                .to_ascii_uppercase(),
                        )
                    })
                    .collect();
                json!({
                    "member": dependency.member,
                    "title": dependency.title,
                    "interface": dependency.interface,
                    "required": entries.iter().any(|entry| entry.required),
                    // An output drives an input of the equipment it is connected to; a measurement
                    // drives none (a controller's two signal lines, §6 item 5).
                    "drives": entries.iter().any(|entry| entry.drives.is_some()),
                    "value": value,
                    "choices": choices,
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    json!({
        "ok": true,
        "instance": selected.instance,
        "block": selected.block,
        "title": title,
        "declared": selected.declared,
        "owned": selected.owned,
        "settings": settings,
        "dependencies": dependencies,
        "hardware": io::status(&terminals),
        "terminals": terminals,
        "bindings": bindings.iter().map(io::binding_json).collect::<Vec<_>>(),
        "picture": picture(inputs, index, &selected.block),
    })
}

/// The picture of the instance's definition, when the project owns that definition (§21.7).
fn picture(inputs: &ProjectInputs, index: &Declarations, block: &str) -> Value {
    let files = super::hmi_authoring::bundle::read_edited(&inputs.root, &inputs.open)
        .map(|bundle| bundle.files)
        .unwrap_or_default();
    let (symbols, _) = trust_runtime::hmi::my_symbols::symbols_in(&files);
    let catalogue = trust_runtime::hmi::catalogue::Catalogue::shipped().with_symbols(&symbols);
    super::hmi_own_equipment::picture::describe(
        &inputs.root,
        &inputs.open,
        index,
        &catalogue,
        block,
    )
}

/// The changed design, written to `plant.toml`, and the regions written again for an instance
/// they hold.
fn apply(
    inputs: &ProjectInputs,
    index: &Declarations,
    mut design: PlantDesign,
    design_text: Option<String>,
    definitions: &[HmiPair],
    selected: &Selected<'_>,
    changes: Changes,
) -> Result<Value, String> {
    if !selected.owned {
        return Err(format!(
            "{} is declared in your PLC code; change its values and connections there",
            selected.instance
        ));
    }
    let definition = selected
        .definition
        .as_ref()
        .ok_or_else(|| format!("{} has no component definition", selected.block))?;
    if changes.settings.len() + changes.dependencies.len() + changes.signals.len() > MAX_CHANGES {
        return Err("Change at most 64 values at once".into());
    }
    let bindings = io::bindings(index, &inputs.sources);
    let mut literals = BTreeMap::new();
    for (member, value) in &changes.settings {
        let setting = definition
            .settings
            .iter()
            .find(|setting| setting.member.eq_ignore_ascii_case(member))
            .ok_or_else(|| format!("{} has no setting {member}", selected.block))?;
        let literal = match value {
            Value::Null => None,
            value => Some(equipment_settings::literal_for(
                index,
                &selected.block,
                setting,
                value,
            )?),
        };
        literals.insert(setting.member.clone(), literal);
    }
    for (member, target) in &changes.dependencies {
        let entries: Vec<_> = definition
            .dependencies
            .iter()
            .filter(|dependency| dependency.member.eq_ignore_ascii_case(member))
            .collect();
        let Some(dependency) = entries.first() else {
            return Err(format!("{} has no dependency {member}", selected.block));
        };
        let offered = entries.iter().any(|entry| {
            candidates(
                inputs,
                index,
                selected,
                &entry.interface,
                entry.drives.as_deref(),
            )
            .iter()
            .any(|choice| {
                choice["path"]
                    .as_str()
                    .is_some_and(|path| path.eq_ignore_ascii_case(target))
            })
        });
        if !target.is_empty() && !offered {
            let interfaces: Vec<&str> = entries
                .iter()
                .map(|entry| entry.interface.as_str())
                .collect();
            return Err(format!(
                "{}: choose equipment of the same program implementing {}",
                dependency.title,
                interfaces.join(" or ")
            ));
        }
    }
    {
        let entry = design.entry_mut(selected.instance, &selected.block);
        for (member, literal) in literals {
            match literal {
                Some(literal) => entry.settings.insert(member, literal),
                None => entry.settings.remove(&member),
            };
        }
        for (member, target) in &changes.dependencies {
            if target.is_empty() {
                entry.dependencies.remove(member);
            } else {
                entry.dependencies.insert(member.clone(), target.clone());
            }
        }
        for (member, target) in &changes.signals {
            if target.is_empty() {
                entry.signals.remove(member);
            } else {
                entry.signals.insert(member.clone(), target.clone());
            }
        }
    }
    for (member, target) in changes
        .signals
        .iter()
        .filter(|(_, target)| !target.is_empty())
    {
        let terminal = definition
            .io
            .iter()
            .find(|terminal| terminal.member.eq_ignore_ascii_case(member))
            .ok_or_else(|| format!("{} has no hardware terminal {member}", selected.block))?;
        io::validate(
            index,
            &bindings,
            &design,
            selected.instance,
            &selected.block,
            terminal,
            target,
        )?;
    }
    let text = design.write();
    PlantDesign::parse(&text)?;
    let design_uri = Url::from_file_path(inputs.root.join(PLANT_FILE))
        .map_err(|_| "plant.toml is not a valid file path")?
        .to_string();
    let mut expected = BTreeMap::new();
    let mut changes_out = BTreeMap::new();
    let mut creates = Vec::new();
    match design_text {
        Some(before) => {
            changes_out.insert(design_uri.clone(), vec![whole_file(&before, text)]);
            expected.insert(design_uri, before);
        }
        None => creates.push(json!({"uri": design_uri, "text": text})),
    }
    if selected.declared {
        if let Some((uri, before, after)) = super::hmi_generate::refresh_regions(
            inputs,
            index,
            &design,
            definitions,
            selected.instance,
        )? {
            if before != after {
                changes_out.insert(uri.clone(), vec![whole_file(&before, after)]);
                expected.insert(uri, before);
            }
        }
    }
    Ok(
        json!({"ok": true, "expected": expected, "edit": {"changes": changes_out}, "creates": creates}),
    )
}

fn whole_file(before: &str, after: String) -> Value {
    json!({
        "range": {"start": offset_to_position(before, 0), "end": offset_to_position(before, before.len() as u32)},
        "newText": after,
    })
}
