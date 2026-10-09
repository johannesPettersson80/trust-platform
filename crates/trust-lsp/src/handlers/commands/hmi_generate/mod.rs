//! Create PLC code (specification 39, §10): for each equipment instance a page shows and the PLC
//! code does not declare yet, one declaration and one call in the program the instance names,
//! inside the regions the generator owns; a PID loop gets its declarations and its code as
//! ordinary ST around the real `Control.PID` (§10.5). Instances declared by hand are reused, never
//! redeclared. The result is one workspace edit for the editor to apply; nothing is written here.

mod equipment;
mod loops;
mod regions;

pub(in crate::handlers::commands) use loops::{beside_kind, is_loop, loop_variable_names};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Value};
use tower_lsp::lsp_types::{Range, TextEdit, Url};
use trust_runtime::hmi::page_files::{self as native, HmiPair};
use trust_syntax::SyntaxKind;

use super::hmi_authoring::declarations::Declarations;
use super::hmi_plant_project::{read_design, DesignEntry, PlantDesign};
use super::{HmiBindingsCommandArgs, ProjectInputs, HMI_ADD_MISSING_VARIABLES_COMMAND};
use crate::handlers::context::ServerContext;
use crate::handlers::lsp_utils::offset_to_position;
use equipment::Planned;

/// The arguments of Create PLC code.
pub(super) struct Request {
    pub selection: HmiBindingsCommandArgs,
    /// The PLC task the HMI screen service runs in, once the user chose it.
    pub screen_task: Option<String>,
    /// The control cycle for a project without a task, once the user chose it.
    pub cycle_ms: Option<u32>,
}

/// The project as Create PLC code reads it.
pub(super) struct Project<'a> {
    pub inputs: &'a ProjectInputs,
    pub index: &'a Declarations,
    pub design: &'a PlantDesign,
    pub definitions: &'a [HmiPair],
}

/// The item naming a block: the one naming it, else the one naming it without namespace.
pub(super) fn named<'a, T>(
    items: &'a [T],
    block: &str,
    name: impl Fn(&T) -> &str,
) -> Option<&'a T> {
    let found = trust_runtime::hmi::binding_checks::block_named(items.iter().map(&name), block)
        .or_else(|| {
            let short = block.rsplit('.').next().unwrap_or(block);
            items.iter().map(&name).find(|candidate| {
                candidate
                    .rsplit('.')
                    .next()
                    .is_some_and(|own| own.eq_ignore_ascii_case(short))
            })
        })?;
    items.iter().find(|item| name(*item) == found)
}

/// The definition of a block.
pub(super) fn definition_of<'a>(definitions: &'a [HmiPair], block: &str) -> Option<&'a HmiPair> {
    named(definitions, block, |definition| definition.block.as_str())
}

impl Project<'_> {
    /// The definition of a block.
    pub(super) fn definition(&self, block: &str) -> Option<&HmiPair> {
        definition_of(self.definitions, block)
    }

    /// Whether two type names name the same declared type.
    fn same_type(&self, left: &str, right: &str) -> bool {
        match (self.index.symbol(left), self.index.symbol(right)) {
            (Some(left), Some(right)) => left.id == right.id,
            _ => {
                let short =
                    |name: &str| name.rsplit('.').next().unwrap_or(name).to_ascii_uppercase();
                short(left) == short(right)
            }
        }
    }
}

/// Where an instance's program is declared.
#[derive(Debug, Clone)]
pub(super) struct Target {
    /// The program instance name the instance path starts with (`Main`).
    pub instance: String,
    /// The program type (`Main`, or the type a configuration runs as `Main`).
    pub program: String,
    /// The project-relative source file.
    pub file: String,
    /// Whether the file and program do not exist yet.
    pub new: bool,
}

/// The program an instance path's first part names.
pub(super) fn target(project: &Project<'_>, root: &str) -> Result<Target, String> {
    let index = project.index;
    let program = index
        .variables
        .iter()
        .find(|node| node.path.eq_ignore_ascii_case(root))
        .map_or_else(|| root.to_string(), |node| node.type_name.clone());
    let declared = index
        .symbol(&program)
        .filter(|symbol| matches!(symbol.kind, trust_hir::symbols::SymbolKind::Program));
    let Some(symbol) = declared else {
        if !is_identifier(&program) {
            return Err(format!("{root} is not a valid program name"));
        }
        let file = format!("src/{program}.st");
        if project.inputs.root.join(&file).exists() {
            return Err(format!(
                "{file} exists but does not declare PROGRAM {program}"
            ));
        }
        return Ok(Target {
            instance: root.to_string(),
            program,
            file,
            new: true,
        });
    };
    let path = symbol
        .origin
        .and_then(|origin| index.source_paths.get(&origin.file_id))
        .ok_or_else(|| format!("the source of PROGRAM {program} was not found"))?;
    let file = relative(&project.inputs.root, path)
        .filter(|file| !file.starts_with("libraries/") && !file.starts_with(".trust/"))
        .ok_or_else(|| {
            format!("PROGRAM {program} belongs to a library; choose a program of the project")
        })?;
    Ok(Target {
        instance: root.to_string(),
        program,
        file,
        new: false,
    })
}

fn relative(root: &Path, path: &Path) -> Option<String> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    path.strip_prefix(root)
        .ok()
        .map(|file| file.to_string_lossy().replace('\\', "/"))
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The equipment the project's pages show: instance path to block, from the edited pages.
fn shown_equipment(inputs: &ProjectInputs) -> Result<BTreeMap<String, String>, Value> {
    let bundle =
        super::hmi_authoring::bundle::read_edited(&inputs.root, &inputs.open).map_err(|error| {
            super::hmi_files::refused(format!("the HMI files cannot be read: {error}"))
        })?;
    let mut shown: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (file, bytes) in &bundle.files {
        if !file.starts_with("hmi/pages/") || !file.ends_with(".toml") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        let page = native::parse_page(file, &text)
            .map_err(|errors| super::hmi_files::file_errors(&errors))?;
        for object in &page.objects {
            let (Some(block), Some(instance)) = (&object.block, &object.instance) else {
                continue;
            };
            let key = instance.to_ascii_uppercase();
            match shown.get(&key) {
                Some((known, _)) if !known.eq_ignore_ascii_case(block) => {
                    return Err(super::hmi_files::refused(format!(
                        "{instance} is shown as {known} and as {block}; choose one type"
                    )));
                }
                Some(_) => {}
                None => {
                    shown.insert(key, (block.clone(), instance.clone()));
                }
            }
        }
    }
    Ok(shown
        .into_values()
        .map(|(block, instance)| (instance, block))
        .collect())
}

/// The design entries by upper-case instance path.
fn designs(design: &PlantDesign) -> BTreeMap<String, &DesignEntry> {
    design
        .equipment
        .iter()
        .map(|(path, entry)| (path.to_ascii_uppercase(), entry))
        .collect()
}

/// The instance paths of the project that have an addressed member (`AT %QX0.0` through
/// `VAR_CONFIG`), upper case: such a terminal is connected without a call parameter.
fn own_addresses(project: &Project<'_>) -> BTreeSet<String> {
    super::hmi_authoring::io::bindings(project.index, &project.inputs.sources)
        .into_iter()
        .map(|binding| binding.path.to_ascii_uppercase())
        .collect()
}

/// The regions of one program holding `planned`, rendered, with the write rules its loops need.
struct Rendered {
    declarations: String,
    retained: String,
    calls: String,
    notes: Vec<Value>,
    writes: Vec<Value>,
}

impl Rendered {
    /// The texts to write, with the namespaces a loop's code names without their prefix.
    fn generated(&self) -> regions::Generated<'_> {
        regions::Generated {
            declarations: &self.declarations,
            retained: &self.retained,
            calls: &self.calls,
            usings: if self.retained.is_empty() {
                &[]
            } else {
                &loops::LOOP_NAMESPACES
            },
        }
    }
}

/// The period of the task a program instance runs in, as written (`T#100ms`).
fn task_interval<'a>(index: &'a Declarations, instance: &str) -> Option<&'a str> {
    index
        .task_intervals
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(instance))
        .map(|(_, interval)| interval.as_str())
}

/// The regions of one program holding `planned`, rendered, with the notes. `cycle_ms` is the
/// control cycle the user chose for a project without a task.
fn render_program(
    project: &Project<'_>,
    target: &Target,
    planned: &[Planned],
    addressed: &BTreeSet<String>,
    cycle_ms: Option<u32>,
) -> Rendered {
    let designs = designs(project.design);
    let known_paths: BTreeSet<String> = planned
        .iter()
        .map(|item| item.path.to_ascii_uppercase())
        .collect();
    let chosen_cycle = cycle_ms.map(|ms| format!("T#{ms}ms"));
    let interval = task_interval(project.index, &target.instance).or(chosen_cycle.as_deref());
    let mut declarations = String::new();
    let mut retained = String::new();
    let mut calls = String::new();
    let mut notes = Vec::new();
    let mut writes = Vec::new();
    let mut values = BTreeMap::new();
    for item in planned {
        let design = designs.get(&item.path.to_ascii_uppercase()).copied();
        if loops::is_loop(&item.block) {
            let (plain, kept) = loops::declarations(item, design);
            declarations.push_str(&plain);
            retained.push_str(&kept);
            let range = loops::measurement_of(design)
                .and_then(|measured| measurement_range(project, planned, &designs, measured));
            writes.extend(loops::write_rules(&target.instance, item, design, range));
            continue;
        }
        let mut initial = equipment::initial_values(project.index, item, design);
        equipment::imply_conditions(project.index, item, design, &mut initial);
        declarations.push_str(&equipment::declaration(item, &initial));
        values.insert(item.path.to_ascii_uppercase(), initial);
    }
    for at in equipment::order(planned, &designs) {
        let item = &planned[at];
        let design = designs.get(&item.path.to_ascii_uppercase()).copied();
        let initial = values
            .get(&item.path.to_ascii_uppercase())
            .cloned()
            .unwrap_or_default();
        // A terminal needed only with feedback follows the block's default too (`HasFeedback`).
        let known: Vec<(String, String)> = match &item.definition {
            Some(definition) => super::hmi_authoring::io::with_block_conditions(
                project.index,
                &item.block,
                &definition.io,
                &initial.iter().cloned().collect(),
            )
            .into_iter()
            .collect(),
            None => initial,
        };
        let resolved = equipment::with_chosen_alternatives(project.index, planned, item, design);
        let (text, missing) = equipment::call(
            &resolved,
            &target.instance,
            design,
            &known,
            addressed,
            &known_paths,
            interval,
        );
        calls.push_str(&text);
        notes.extend(missing);
    }
    Rendered {
        declarations,
        retained,
        calls,
        notes,
        writes,
    }
}

/// The range a measurement covers, from its `RangeLow` and `RangeHigh` (an analog input's): the
/// design's choice, else the definition's initial value, else the block's default.
fn measurement_range(
    project: &Project<'_>,
    planned: &[Planned],
    designs: &BTreeMap<String, &DesignEntry>,
    measured: &str,
) -> Option<(f64, f64)> {
    let item = planned
        .iter()
        .find(|item| item.path.eq_ignore_ascii_case(measured))?;
    let design = designs.get(&item.path.to_ascii_uppercase()).copied();
    let initial = item
        .definition
        .as_ref()
        .and_then(|definition| definition.program.as_ref())
        .map(|program| program.initial.clone())
        .unwrap_or_default();
    let value = |member: &str| {
        super::hmi_authoring::equipment_settings::chosen_literal(
            project.index,
            &item.block,
            member,
            design,
            &BTreeMap::new(),
            &initial,
        )
        .map(|(literal, _)| literal)
        .or_else(|| {
            super::hmi_authoring::equipment_settings::block_default(
                project.index,
                &item.block,
                member,
            )
        })
        .and_then(|literal| loops::number(&literal))
    };
    Some((value("RangeLow")?, value("RangeHigh")?))
}

/// The regions of a program as found: the instances they declare, with their declared types,
/// and each region whose checksum no longer matches.
#[derive(Default)]
struct Found {
    instances: Vec<(String, String)>,
    stale: Vec<(regions::Kind, regions::Region)>,
}

fn found_regions(text: &str, file: &str, program: &str) -> Result<Found, String> {
    let root = trust_syntax::parser::parse(text).syntax();
    let Some(node) = regions::program_node(&root, program) else {
        return Ok(Found::default());
    };
    let range = regions::range_of(&node);
    let declarations = regions::find(text, file, range, regions::Kind::Declarations)?;
    let retained = regions::find(text, file, range, regions::Kind::Retained)?;
    let calls = regions::find(text, file, range, regions::Kind::Calls)?;
    let instances = declarations
        .as_ref()
        .map(|region| regions::declared(&region.content))
        .unwrap_or_default();
    let stale = [
        (regions::Kind::Declarations, declarations),
        (regions::Kind::Retained, retained),
        (regions::Kind::Calls, calls),
    ]
    .into_iter()
    .filter_map(|(kind, region)| {
        region
            .filter(|region| !region.intact)
            .map(|region| (kind, region))
    })
    .collect();
    Ok(Found { instances, stale })
}

/// The instances the regions of a program declare, as planned instances: a loop's own variables
/// (`TIC101_Setpoint`) belong to its PID, which is the instance.
fn existing(project: &Project<'_>, target: &Target, found: &Found) -> Vec<Planned> {
    let loops: Vec<&str> = found
        .instances
        .iter()
        .filter(|(_, ty)| loops::is_loop(ty))
        .map(|(name, _)| name.as_str())
        .collect();
    found
        .instances
        .iter()
        .filter(|(name, _)| !loops::belongs_to_loop(name, &loops))
        .map(|(name, ty)| Planned {
            name: name.clone(),
            path: format!("{}.{name}", target.instance),
            block: ty.clone(),
            definition: project.definition(ty).cloned(),
        })
        .collect()
}

/// A region whose checksum no longer matches is accepted only when it is exactly what the
/// generator writes for the instances it declares, as after renaming one of them; otherwise it
/// was changed by hand and is refused, never overwritten (§10.3 item 1). The calls of one kind
/// are in name order, so a rename may reorder them: the calls compare statement by statement.
fn check_stale(
    project: &Project<'_>,
    target: &Target,
    found: &Found,
    existing: &[Planned],
    addressed: &BTreeSet<String>,
) -> Result<(), String> {
    if found.stale.is_empty() {
        return Ok(());
    }
    let rendered = render_program(project, target, existing, addressed, None);
    for (kind, region) in &found.stale {
        let same = match kind {
            regions::Kind::Declarations => region.content == rendered.declarations,
            regions::Kind::Retained => region.content == rendered.retained,
            regions::Kind::Calls => statements(&region.content) == statements(&rendered.calls),
        };
        if !same {
            return Err(regions::changed_by_hand(&target.file, region.line, *kind));
        }
    }
    Ok(())
}

/// The statements of a calls region, each with its continuation lines, sorted.
fn statements(text: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    for line in text.split_inclusive('\n') {
        match statements.last_mut() {
            Some(statement) if line.starts_with("        ") => statement.push_str(line),
            _ => statements.push(line.to_string()),
        }
    }
    statements.sort();
    statements
}

/// The text of a project file as the editor or the disk has it.
fn source_text(inputs: &ProjectInputs, file: &str) -> Option<String> {
    let path = inputs.root.join(file);
    inputs
        .sources
        .iter()
        .find(|source| {
            let absolute = if source.path.is_absolute() {
                source.path.clone()
            } else {
                inputs.root.join(&source.path)
            };
            absolute == path
        })
        .map(|source| source.text.clone())
}

/// A whole-file edit.
fn replace_all(before: &str, after: String) -> TextEdit {
    TextEdit {
        range: Range {
            start: offset_to_position(before, 0),
            end: offset_to_position(before, before.len() as u32),
        },
        new_text: after,
    }
}

fn uri_of(root: &Path, file: &str) -> Result<String, String> {
    Url::from_file_path(root.join(file))
        .map(|uri| uri.to_string())
        .map_err(|_| format!("{file} is not a valid file path"))
}

/// The planned instances of each program: those its regions hold, and the shown instances it
/// does not declare yet.
fn plan(
    project: &Project<'_>,
    shown: &BTreeMap<String, String>,
    addressed: &BTreeSet<String>,
    notes: &mut Vec<Value>,
) -> Result<BTreeMap<String, (Target, Vec<Planned>)>, String> {
    let mut programs: BTreeMap<String, (Target, Vec<Planned>)> = BTreeMap::new();
    for (instance, block) in shown {
        let mut parts = instance.split('.');
        let (Some(root), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
            // A block inside another instance is shown, never declared on its own.
            continue;
        };
        if !is_identifier(name) {
            return Err(format!("{instance}: {name} is not a valid PLC name"));
        }
        let declared_type = project
            .index
            .resolve(instance)
            .map(|node| node.type_name.clone());
        if let Some(declared) = &declared_type {
            if !project.same_type(declared, block) {
                return Err(format!(
                    "{instance} is declared as {declared}; the page shows {block}"
                ));
            }
        }
        let target = target(project, root)?;
        let key = target.program.to_ascii_uppercase();
        if !programs.contains_key(&key) {
            let text = if target.new {
                String::new()
            } else {
                source_text(project.inputs, &target.file)
                    .ok_or_else(|| format!("{} cannot be read", target.file))?
            };
            let found = found_regions(&text, &target.file, &target.program)?;
            let held = existing(project, &target, &found);
            check_stale(project, &target, &found, &held, addressed)?;
            programs.insert(key.clone(), (target.clone(), held));
        }
        let (_, planned) = programs.get_mut(&key).expect("inserted above");
        if planned
            .iter()
            .any(|item| item.name.eq_ignore_ascii_case(name))
        {
            continue;
        }
        if declared_type.is_some() {
            // Declared by hand outside the regions: reused as it is (§10.3 item 1).
            continue;
        }
        let Some(definition) = project.definition(block).cloned() else {
            notes.push(equipment::note(
                instance,
                format!("install the component definition of {block}"),
            ));
            continue;
        };
        let is_block = project
            .index
            .symbol(&definition.block)
            .is_some_and(|symbol| {
                matches!(symbol.kind, trust_hir::symbols::SymbolKind::FunctionBlock)
            });
        if !is_block {
            return Err(format!(
                "{instance}: add the PLC library that declares {} to the project",
                definition.block
            ));
        }
        planned.push(Planned {
            name: name.to_string(),
            path: instance.clone(),
            block: definition.block.clone(),
            definition: Some(definition),
        });
    }
    Ok(programs)
}

/// The configuration a project without one gets: one task running each program.
fn configuration(
    project: &Project<'_>,
    programs: &[&Target],
    cycle_ms: u32,
) -> Result<(String, String), String> {
    let file = "src/plant-configuration.st";
    if project.inputs.root.join(file).exists() {
        return Err(format!(
            "{file} already exists; assign the programs to a task there"
        ));
    }
    if project.index.symbol("PlantConfiguration").is_some() {
        return Err(
            "PlantConfiguration is already declared; assign the programs to a task in ST".into(),
        );
    }
    let assignments: String = programs
        .iter()
        .map(|target| {
            format!(
                "        PROGRAM {} WITH ControlTask : {};\n",
                target.instance, target.program
            )
        })
        .collect();
    Ok((
        file.to_string(),
        format!(
            "CONFIGURATION PlantConfiguration\n    RESOURCE Cpu ON PLC\n        TASK ControlTask (INTERVAL := T#{cycle_ms}ms, PRIORITY := 1);\n{assignments}    END_RESOURCE\nEND_CONFIGURATION\n"
        ),
    ))
}

/// `trust-lsp.hmiAddMissingVariables` with `generate_project`: `{ ok, expected, edit, creates,
/// notes }`, or `{ ok: false, needsControlCycle }` / `{ needsScreenTask }` for a choice of the user.
pub(super) fn create<C: ServerContext>(context: &C, request: Request) -> Value {
    let inputs = match super::project_inputs(
        context,
        &request.selection,
        HMI_ADD_MISSING_VARIABLES_COMMAND,
    ) {
        Ok(inputs) => inputs,
        Err(message) => return super::hmi_files::refused(message),
    };
    let shown = match shown_equipment(&inputs) {
        Ok(shown) => shown,
        Err(answer) => return answer,
    };
    create_from(&inputs, &shown, &request).unwrap_or_else(super::hmi_files::refused)
}

fn create_from(
    inputs: &ProjectInputs,
    shown: &BTreeMap<String, String>,
    request: &Request,
) -> Result<Value, String> {
    let (design, _) = read_design(&inputs.root, &inputs.open)?;
    let definitions = super::hmi_program_edits::project_pairs_edited(&inputs.root, &inputs.open);
    let index = Declarations::read(&inputs.sources);
    let project = Project {
        inputs,
        index: index.as_ref(),
        design: &design,
        definitions: &definitions,
    };
    let mut notes = Vec::new();
    let mut writes = Vec::new();
    let addressed = own_addresses(&project);
    let programs = plan(&project, shown, &addressed, &mut notes)?;
    let mut output: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut creates: BTreeMap<String, String> = BTreeMap::new();
    for (target, planned) in programs.values() {
        if planned.is_empty() {
            continue;
        }
        let rendered = render_program(&project, target, planned, &addressed, request.cycle_ms);
        notes.extend(rendered.notes.iter().cloned());
        writes.extend(rendered.writes.iter().cloned());
        if target.new {
            creates.insert(
                target.file.clone(),
                regions::new_program(&target.program, &rendered.generated()),
            );
            continue;
        }
        let before = source_text(inputs, &target.file)
            .ok_or_else(|| format!("{} cannot be read", target.file))?;
        let current = output
            .get(&target.file)
            .map_or(before.clone(), |(_, after)| after.clone());
        let after = regions::write(
            &current,
            &target.file,
            &target.program,
            &rendered.generated(),
        )?;
        output.insert(target.file.clone(), (before, after));
    }
    // Every program holding equipment runs in a task (§10.3 item 3).
    let unassigned: Vec<&Target> = programs
        .values()
        .filter(|(_, planned)| !planned.is_empty())
        .map(|(target, _)| target)
        .filter(|target| {
            !index
                .task_intervals
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(&target.instance))
        })
        .collect();
    if !unassigned.is_empty() {
        let configured = inputs.sources.iter().any(|source| {
            trust_syntax::parser::parse(&source.text)
                .syntax()
                .descendants()
                .any(|node| node.kind() == SyntaxKind::Configuration)
        });
        if configured {
            let names: Vec<&str> = unassigned
                .iter()
                .map(|target| target.instance.as_str())
                .collect();
            return Err(format!(
                "assign {} to a task in the configuration before creating PLC code",
                names.join(", ")
            ));
        }
        let Some(cycle_ms) = request.cycle_ms else {
            return Ok(json!({"ok": false, "needsControlCycle": true}));
        };
        if !(1..=60_000).contains(&cycle_ms) {
            return Err("choose a control cycle from 1 to 60000 milliseconds".into());
        }
        let (file, text) = configuration(&project, &unassigned, cycle_ms)?;
        creates.insert(file, text);
    }
    if let Some(answer) = screen_service(inputs, &index, request, &mut output, &mut creates)? {
        return Ok(answer);
    }
    let mut expected = BTreeMap::new();
    let mut changes = BTreeMap::new();
    for (file, (before, after)) in output {
        if before == after {
            continue;
        }
        let uri = uri_of(&inputs.root, &file)?;
        changes.insert(uri.clone(), vec![replace_all(&before, after)]);
        expected.insert(uri, before);
    }
    let creates: Vec<Value> = creates
        .into_iter()
        .map(|(file, text)| Ok(json!({"uri": uri_of(&inputs.root, &file)?, "text": text})))
        .collect::<Result<_, String>>()?;
    Ok(json!({
        "ok": true,
        "expected": expected,
        "edit": {"changes": changes},
        "creates": creates,
        "notes": notes,
        "writes": writes,
    }))
}

/// The HMI screen service in a task, for a project using the HMI library's display.
fn screen_service(
    inputs: &ProjectInputs,
    index: &Declarations,
    request: &Request,
    output: &mut BTreeMap<String, (String, String)>,
    creates: &mut BTreeMap<String, String>,
) -> Result<Option<Value>, String> {
    if index.symbol("Hmi.Display").is_none() {
        return Ok(None);
    }
    let mut sources = BTreeMap::new();
    for source in &inputs.sources {
        let Some(file) = relative(&inputs.root, &source.path) else {
            continue;
        };
        if file.starts_with("libraries/") || file.starts_with(".trust/") {
            continue;
        }
        let text = output
            .get(&file)
            .map_or_else(|| source.text.clone(), |(_, after)| after.clone());
        sources.insert(file, text);
    }
    sources.extend(creates.clone());
    let sources: Vec<Value> = sources
        .iter()
        .map(|(file, text)| json!({"file": file, "text": text}))
        .collect();
    let screen = super::hmi_screens_program::prepare(vec![
        json!({"sources": sources, "existing_task": true, "screen_task": request.screen_task}),
    ]);
    if screen.get("needsScreenTask").is_some() {
        return Ok(Some(screen));
    }
    if screen["ok"] != true {
        return Err(screen["errors"][0]["message"]
            .as_str()
            .unwrap_or("the screen task is unresolved")
            .into());
    }
    for change in screen["changes"].as_array().into_iter().flatten() {
        let file = change["file"]
            .as_str()
            .ok_or("the screen change names no file")?;
        let text = change["text"]
            .as_str()
            .ok_or("the screen change has no text")?
            .to_string();
        if let Some(created) = creates.get_mut(file) {
            *created = text;
        } else {
            let before =
                source_text(inputs, file).ok_or_else(|| format!("{file} cannot be read"))?;
            output
                .entry(file.to_string())
                .or_insert((before, String::new()))
                .1 = text;
        }
    }
    for created in screen["creates"].as_array().into_iter().flatten() {
        let file = created["file"]
            .as_str()
            .ok_or("the screen file names no path")?;
        let text = created["text"]
            .as_str()
            .ok_or("the screen file has no text")?;
        // An empty file of that name, left by an earlier run whose text was never saved, is
        // written again; one with text is the user's own.
        let taken = std::fs::metadata(inputs.root.join(file)).is_ok_and(|meta| meta.len() > 0);
        if taken || creates.contains_key(file) {
            return Err(format!("{file} already exists"));
        }
        creates.insert(file.to_string(), text.to_string());
    }
    Ok(None)
}

/// After the inspector changed an instance's design: the program file with its regions written
/// again from the new design, when the regions hold the instance; no instance is added.
pub(super) fn refresh_regions(
    inputs: &ProjectInputs,
    index: &Declarations,
    design: &PlantDesign,
    definitions: &[HmiPair],
    instance: &str,
) -> Result<Option<(String, String, String)>, String> {
    let Some((root, name)) = instance.split_once('.') else {
        return Ok(None);
    };
    let project = Project {
        inputs,
        index,
        design,
        definitions,
    };
    let target = target(&project, root)?;
    if target.new {
        return Ok(None);
    }
    let before = source_text(inputs, &target.file)
        .ok_or_else(|| format!("{} cannot be read", target.file))?;
    let found = found_regions(&before, &target.file, &target.program)?;
    if !found
        .instances
        .iter()
        .any(|(held, _)| held.eq_ignore_ascii_case(name))
    {
        return Ok(None);
    }
    let planned = existing(&project, &target, &found);
    let addressed = own_addresses(&project);
    if !found.stale.is_empty() {
        // The stale regions are judged against the design they were written from.
        let (previous, _) = read_design(&inputs.root, &inputs.open)?;
        let earlier = Project {
            inputs,
            index,
            design: &previous,
            definitions,
        };
        check_stale(&earlier, &target, &found, &planned, &addressed)?;
    }
    let rendered = render_program(&project, &target, &planned, &addressed, None);
    let after = regions::write(
        &before,
        &target.file,
        &target.program,
        &rendered.generated(),
    )?;
    Ok(Some((uri_of(&inputs.root, &target.file)?, before, after)))
}

/// Whether the generator's regions declare an instance: the builder owns its configuration.
pub(super) fn generated(inputs: &ProjectInputs, index: &Declarations, instance: &str) -> bool {
    let Some((root, name)) = instance.split_once('.') else {
        return false;
    };
    let design = PlantDesign::default();
    let project = Project {
        inputs,
        index,
        design: &design,
        definitions: &[],
    };
    let Ok(target) = target(&project, root) else {
        return false;
    };
    source_text(inputs, &target.file)
        .and_then(|text| found_regions(&text, &target.file, &target.program).ok())
        .is_some_and(|found| {
            found
                .instances
                .iter()
                .any(|(held, _)| held.eq_ignore_ascii_case(name))
        })
}

/// Where an instance (`Main.P101`) is declared: its program's file and the byte offset of its name
/// there, for a rename that starts from the HMI Builder (§21.3 item 2).
pub(super) fn declaration_of(
    inputs: &ProjectInputs,
    instance: &str,
) -> Result<(String, usize), String> {
    let (root, name) = instance
        .split_once('.')
        .filter(|(_, name)| !name.contains('.'))
        .ok_or_else(|| format!("{instance} is not a program's instance"))?;
    let index = Declarations::read(&inputs.sources);
    let design = PlantDesign::default();
    let project = Project {
        inputs,
        index: &index,
        design: &design,
        definitions: &[],
    };
    let target = target(&project, root)?;
    let missing = || format!("{instance} is not declared in PROGRAM {}", target.program);
    let text = source_text(inputs, &target.file).ok_or_else(missing)?;
    let parsed = trust_syntax::parser::parse(&text).syntax();
    let program = regions::program_node(&parsed, &target.program).ok_or_else(missing)?;
    // A name node also holds the comments before it, such as a region's begin marker: the name is
    // its code, without them, and it starts at its first token that is code.
    let declared = program
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .flat_map(|declaration| {
            declaration
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
        })
        .find(|node| super::control_links::code_text(node).eq_ignore_ascii_case(name))
        .ok_or_else(missing)?;
    let start = declared
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| usize::from(token.text_range().start()))
        .ok_or_else(missing)?;
    Ok((target.file, start))
}

#[cfg(test)]
mod tests;
