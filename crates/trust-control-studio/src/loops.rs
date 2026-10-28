//! The loops a project declares (specification 35, §17.1): `control-loop.json` and the JSON files
//! of `control-loops/`. Signal names are never guessed from Structured Text.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::{
    effective_loop, parked_failure, parked_feature_of, ChannelSelection, LoopDefinition, LoopScope,
};
use trust_control::messages::LoopEntry;

use crate::storage;

/// One loop file and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectLoop {
    /// The file.
    pub file: PathBuf,
    /// The checked loop, or why the file cannot be opened.
    pub loop_definition: Result<LoopDefinition, String>,
}

/// The project's loop files: `control-loop.json` when present, then `control-loops/*.json` in name
/// order.
///
/// # Errors
/// `control-loops/` exists but cannot be read.
pub fn loop_files(project_root: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut files = Vec::new();
    let root_file = project_root.join("control-loop.json");
    if root_file.is_file() {
        files.push(root_file);
    }
    let directory = project_root.join("control-loops");
    match fs::read_dir(&directory) {
        Ok(entries) => {
            let mut more: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_file()
                        && path
                            .extension()
                            .is_some_and(|extension| extension == "json")
                })
                .collect();
            more.sort();
            files.extend(more);
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Failure::new(
                &format!("The control-loops directory could not be read: {error}."),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ))
        }
    }
    Ok(files)
}

/// Every loop file of the project, read and checked for the runtime `runtime_id`. A loop
/// identifier declared twice makes both files unusable.
///
/// # Errors
/// As [`loop_files`].
pub fn project_loops(project_root: &Path, runtime_id: &str) -> Result<Vec<ProjectLoop>, Failure> {
    let mut loops: Vec<ProjectLoop> = loop_files(project_root)?
        .into_iter()
        .map(|file| {
            let loop_definition = read_loop(&file, runtime_id);
            ProjectLoop {
                file,
                loop_definition,
            }
        })
        .collect();
    let ids: Vec<Option<String>> = loops
        .iter()
        .map(|entry| {
            entry
                .loop_definition
                .as_ref()
                .ok()
                .map(|loop_definition| loop_definition.id.clone())
        })
        .collect();
    for (entry, id) in loops.iter_mut().zip(&ids) {
        if let Some(id) = id
            && ids.iter().flatten().filter(|other| *other == id).count() > 1
        {
            entry.loop_definition = Err(format!(
                "Duplicate control loop ID {id} in this project. Give each loop a unique ID."
            ));
        }
    }
    Ok(loops)
}

/// The project's loops as the panel lists them.
///
/// # Errors
/// As [`loop_files`].
pub fn list(project_root: &Path, runtime_id: &str) -> Result<Vec<LoopEntry>, Failure> {
    Ok(project_loops(project_root, runtime_id)?
        .into_iter()
        .map(|entry| {
            let file = entry.file.display().to_string();
            match entry.loop_definition {
                Ok(loop_definition) => LoopEntry {
                    file,
                    loop_id: Some(loop_definition.id),
                    name: Some(loop_definition.name),
                    problem: None,
                },
                // A parked loop is listed by its identity, as a served loop is; opening it refuses it by name (D12).
                Err(problem) => match parked_identity_and_name(&entry.file) {
                    Some((loop_id, name)) => LoopEntry {
                        file,
                        loop_id: Some(loop_id),
                        name: Some(name),
                        problem: None,
                    },
                    None => LoopEntry {
                        file,
                        loop_id: None,
                        name: None,
                        problem: Some(problem),
                    },
                },
            }
        })
        .collect())
}

/// The identifier and name of a loop file that does not decode because it uses a parked feature (`parked_feature_of`), when the file
/// names both. Such a loop is listed as the loop it is; its open refuses it by name.
fn parked_identity_and_name(file: &Path) -> Option<(String, String)> {
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    parked_feature_of(&raw)?;
    Some((
        raw["id"].as_str()?.to_owned(),
        raw["name"].as_str()?.to_owned(),
    ))
}

/// The checked loop `loop_id` of the project, as the stages read it: the declared loop with the study's channel selection applied
/// (A8.1, the Tune page's **Channels…**), so that Identify, Tune, Validate and Apply use one mapping. The selection belongs to the
/// study of the loop's revision; without that study, or without a selection, the declared loop is returned.
///
/// # Errors
/// The project declares no usable loop of that identifier, the study cannot be read, or the selection no longer fits the loop.
pub fn open(
    project_root: &Path,
    runtime_id: &str,
    loop_id: &str,
) -> Result<LoopDefinition, Failure> {
    let declared = open_declared(project_root, runtime_id, loop_id)?;
    let selection = match storage::current_study(project_root, &declared.id, &declared.revision)? {
        Some(study) => study.section::<ChannelSelection>()?,
        None => None,
    };
    effective_loop(&declared, selection.as_ref())
}

/// The checked loop `loop_id` of the project as its file declares it, without the study's channel selection.
///
/// # Errors
/// The project declares no usable loop of that identifier.
pub fn open_declared(
    project_root: &Path,
    runtime_id: &str,
    loop_id: &str,
) -> Result<LoopDefinition, Failure> {
    let loops = project_loops(project_root, runtime_id)?;
    let mut problems = Vec::new();
    for entry in loops {
        match entry.loop_definition {
            Ok(loop_definition) if loop_definition.id == loop_id => return Ok(loop_definition),
            Ok(_) => {}
            Err(problem) => {
                // A loop that uses a parked feature is refused as such, even when its file does not decode (D12).
                if let Some(feature) = parked_feature_in(&entry.file, loop_id) {
                    return Err(parked_failure(&feature));
                }
                problems.push(problem);
            }
        }
    }
    let detail = if problems.is_empty() {
        String::new()
    } else {
        format!(" {}", problems.join(" "))
    };
    Err(Failure::new(
        &format!("This project declares no usable loop {loop_id}. Configure the loop to bind its signals.{detail}"),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    ))
}

/// The parked feature of the loop `loop_id`, when `file` holds that loop and it uses one (`parked_feature_of`).
fn parked_feature_in(file: &Path, loop_id: &str) -> Option<String> {
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    if raw["id"].as_str() == Some(loop_id) {
        parked_feature_of(&raw)
    } else {
        None
    }
}

/// The identity that the loop file of `loop_id` names, `(projectId, id)`, when the project has a file of that loop which uses a parked
/// structure (`parked_feature_in`). Such a file does not decode, so an operation of the loop claims it by these names, as a served loop
/// is claimed by its decoded identity (A8.5). `None` when no file of the project is such a loop.
#[must_use]
pub fn parked_identity(project_root: &Path, loop_id: &str) -> Option<(String, String)> {
    loop_files(project_root)
        .ok()?
        .iter()
        .find_map(|file| parked_identity_in(file, loop_id))
}

/// The `projectId` and `id` of `file`, read as raw JSON, when `file` is the parked loop `loop_id` (`parked_feature_of`).
fn parked_identity_in(file: &Path, loop_id: &str) -> Option<(String, String)> {
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    let id = raw["id"].as_str().filter(|id| *id == loop_id)?;
    parked_feature_of(&raw)?;
    Some((raw["projectId"].as_str()?.to_owned(), id.to_owned()))
}

fn read_loop(file: &Path, runtime_id: &str) -> Result<LoopDefinition, String> {
    let name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let text =
        fs::read_to_string(file).map_err(|error| format!("{name} could not be read: {error}."))?;
    let loop_definition =
        LoopDefinition::decode(&text).map_err(|rejection| format!("{name}: {rejection}."))?;
    if loop_definition.runtime_id != runtime_id {
        return Err(format!(
            "{name}: the loop uses {}, but {runtime_id} is the selected target.",
            loop_definition.runtime_id
        ));
    }
    let scope = LoopScope {
        project_id: loop_definition.project_id.clone(),
        runtime_id: runtime_id.to_string(),
    };
    loop_definition
        .validate(&scope)
        .map_err(|rejection| format!("{name}: {rejection}."))?;
    Ok(loop_definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str =
        include_str!("../../../examples/control-pid-commissioning/control-loop.json");

    fn project(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("trust-control-loops-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("control-loops")).expect("project folder");
        root
    }

    fn example_runtime() -> String {
        LoopDefinition::decode(EXAMPLE).expect("example").runtime_id
    }

    #[test]
    fn lists_the_root_loop_then_the_folder_in_name_order_and_opens_one_by_id() {
        let root = project("list");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("root loop");
        fs::write(root.join("control-loops/b-broken.json"), "{ not json").expect("broken loop");
        let loops = list(&root, &example_runtime()).expect("listed");
        assert_eq!(loops.len(), 2);
        assert!(loops[0].problem.is_none(), "{:?}", loops[0].problem);
        assert!(
            loops[1]
                .problem
                .as_deref()
                .is_some_and(|problem| problem.starts_with("b-broken.json:")),
            "{:?}",
            loops[1].problem
        );
        let id = loops[0].loop_id.clone().expect("the example's id");
        assert_eq!(open(&root, &example_runtime(), &id).expect("opened").id, id);
        fs::remove_dir_all(&root).expect("clean up");
    }

    /// The loop file of a parked structure, as its `TypeScript` generator writes it: the example's loop with the program's own law of
    /// `family`, and the parts that only that structure declares (`extra`), which the Rust loop does not decode.
    fn parked_loop(id: &str, family: &str, extra: serde_json::Value) -> String {
        let mut file: serde_json::Value = serde_json::from_str(EXAMPLE).expect("example");
        file["id"] = serde_json::json!(id);
        file["controller"]["project"] = serde_json::json!({
            "family": family, "convention": "project-owned-native-controller",
            "initialization": { "kind": "declared-state-v1", "states": [] }, "axes": [],
        });
        if let (Some(object), serde_json::Value::Object(parts)) = (file.as_object_mut(), extra) {
            object.extend(parts);
        }
        file.to_string()
    }

    /// The tank of `control-studio-gain-schedule.test.ts`: a level held by one library PID whose gains are scheduled over two points.
    fn tank_catalog() -> trust_control::setup::catalog::CompiledCatalog {
        let answer = serde_json::json!({
            "ok": true,
            "processImage": {},
            "controlSymbols": {
                "symbols": [
                    { "symbol": "P1.LevelM", "type": "LREAL", "address": "%IL0" },
                    { "symbol": "P1.SetpointM", "type": "LREAL", "address": "%ML0" },
                    { "symbol": "P1.ValvePct", "type": "LREAL", "address": "%QL0" },
                    { "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML8" },
                    { "symbol": "P1.Auto", "type": "BOOL" },
                    { "symbol": "P1.Started", "type": "BOOL" },
                    { "symbol": "P1.KpLow", "type": "LREAL" },
                    { "symbol": "P1.KiLow", "type": "LREAL" },
                    { "symbol": "P1.KpHigh", "type": "LREAL" },
                    { "symbol": "P1.KiHigh", "type": "LREAL" }
                ],
                "controllers": [
                    { "instance": "P1", "type": "Main", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] },
                    { "instance": "P1.Loop", "type": "Control.PID", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
                ],
                "links": [],
                "writes": ["P1.ValvePct"]
            }
        });
        trust_control::setup::catalog::decode_catalog_answer(&answer)
            .expect("the compiled answer decodes")
            .control
    }

    /// The setup's draft of the tank: Low at 0.09 m and High at 0.49 m, each with a tolerance of 0.01 m.
    fn tank_draft() -> trust_control::setup::schedule_loop::ScheduleLoopDraft {
        use trust_control::loop_definition::{Action, ModeWhenTrue};
        use trust_control::setup::loop_bindings::FormSignal;
        use trust_control::setup::pid_loop::{ModeChoice, SignalChoice};
        use trust_control::setup::schedule_loop::{
            ScheduleLoopDraft, SchedulePointDraft, SchedulingVariableDraft,
        };
        let signal = |symbol: &str, unit: &str, maximum: f64| FormSignal {
            symbol: symbol.to_owned(),
            unit: unit.to_owned(),
            minimum: Some(0.0),
            maximum: Some(maximum),
        };
        let point =
            |name: &str, value: f64, proportional: &str, integral: &str| SchedulePointDraft {
                name: name.to_owned(),
                value: Some(value),
                tolerance: Some(0.01),
                proportional_gain: proportional.to_owned(),
                integral_gain: integral.to_owned(),
            };
        ScheduleLoopDraft {
            name: "Tank level".to_owned(),
            action: Action::Direct,
            instance: "P1.Loop".to_owned(),
            parameter_record: "P1.Parameters".to_owned(),
            scheduling: SchedulingVariableDraft {
                symbol: "P1.LevelM".to_owned(),
                unit: "m".to_owned(),
            },
            process_variable: signal("P1.LevelM", "m", 2.0),
            manipulated_variable: signal("P1.ValvePct", "%", 100.0),
            setpoint: SignalChoice {
                symbol: "P1.SetpointM".to_owned(),
                unit: None,
                minimum: None,
                maximum: None,
            },
            mode: ModeChoice {
                symbol: "P1.Auto".to_owned(),
                when_true: ModeWhenTrue::Auto,
            },
            manual_output: "P1.ManualPct".to_owned(),
            points: vec![
                point("Low", 0.09, "P1.KpLow", "P1.KiLow"),
                point("High", 0.49, "P1.KpHigh", "P1.KiHigh"),
            ],
            experiment_step: None,
            experiment_duration_seconds: None,
            startup_flag: Some("P1.Started".to_owned()),
        }
    }

    #[test]
    fn feedforward_and_smith_loops_are_refused_as_parked_until_v7_and_a_schedule_point_is_served() {
        // V6 decision D12: these structures' loops carry the program's own law and parts the Rust loop does not decode; the engine
        // refuses them as a capability it does not have yet, so the panel keeps them on the TypeScript engine.
        let root = project("parked");
        let cases = [
            (
                "loop:feedforward",
                "feedforward",
                serde_json::json!({ "channels": { "inputs": [{ "modelChannelId": "MV", "unknownToRust": true }] } }),
            ),
            (
                "loop:smith",
                "Smith",
                serde_json::json!({ "smithModel": { "gain": 1.0 } }),
            ),
        ];
        for (id, family, extra) in cases {
            fs::write(
                root.join(format!("control-loops/{family}.json")),
                parked_loop(id, family, extra),
            )
            .expect("parked loop");
            // A file that decodes is refused by the engine's gate; one that does not, by `open` itself.
            let refusal = match open(&root, &example_runtime(), id) {
                Ok(loop_definition) => loop_definition
                    .rust_engine_support()
                    .expect_err("a parked loop that decodes is refused by the gate"),
                Err(failure) => failure,
            };
            assert_eq!(
                refusal.cause,
                Cause::ControllerCapability,
                "{family}: {refusal:?}"
            );
            assert!(
                refusal.message.contains("the program's own law"),
                "{family}: {}",
                refusal.message
            );
        }
        // A gain schedule's point that the setup wrote is served (C10): the gate takes its project law and the engine opens it.
        let example = LoopDefinition::decode(EXAMPLE).expect("example");
        let scope = trust_control::loop_definition::LoopScope {
            project_id: example.project_id.clone(),
            runtime_id: example.runtime_id.clone(),
        };
        let planned = trust_control::setup::schedule_loop::schedule_loop_definitions(
            &tank_catalog(),
            &scope,
            &tank_draft(),
        )
        .expect("the tank's schedule is generated");
        let low = &planned[0];
        fs::write(
            root.join("control-loops/tank-level-low.json"),
            serde_json::to_string(low).expect("the loop serializes"),
        )
        .expect("the schedule point");
        let opened = open(&root, &example_runtime(), &low.id).expect("the written point opens");
        assert!(
            opened.rust_engine_support().is_ok(),
            "a written schedule point is served by the gate"
        );
        // A parked loop that does not decode is still listed as the loop it is (D12); opening it refuses it by name.
        let listed = list(&root, &example_runtime()).expect("listed");
        for id in ["loop:feedforward", "loop:smith", "loop:tank-level-low"] {
            assert!(
                listed
                    .iter()
                    .any(|entry| entry.loop_id.as_deref() == Some(id) && entry.problem.is_none()),
                "{id} is listed: {listed:?}"
            );
        }
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_loop_of_another_runtime_or_a_duplicate_id_cannot_be_opened() {
        let root = project("refused");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("root loop");
        let other = list(&root, "runtime:other").expect("listed");
        assert!(
            other[0]
                .problem
                .as_deref()
                .is_some_and(|problem| problem.contains("is the selected target")),
            "{:?}",
            other[0].problem
        );
        fs::write(root.join("control-loops/copy.json"), EXAMPLE).expect("duplicate");
        let loops = list(&root, &example_runtime()).expect("listed");
        assert!(loops.iter().all(|entry| entry
            .problem
            .as_deref()
            .is_some_and(|problem| problem.starts_with("Duplicate control loop ID"))));
        let id = LoopDefinition::decode(EXAMPLE).expect("example").id;
        let failure = open(&root, &example_runtime(), &id).expect_err("refused");
        assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_parked_loop_is_named_by_the_identity_its_file_declares() {
        // A8.5: an operation of a loop the engine cannot open claims it by the `projectId` and `id` that its file names.
        let root = project("identity");
        let project_id = LoopDefinition::decode(EXAMPLE).expect("example").project_id;
        fs::write(
            root.join("control-loops/feedforward.json"),
            parked_loop(
                "loop:feedforward",
                "feedforward",
                serde_json::json!({ "channels": {} }),
            ),
        )
        .expect("parked loop");
        assert_eq!(
            parked_identity(&root, "loop:feedforward"),
            Some((project_id, "loop:feedforward".to_string()))
        );
        assert_eq!(parked_identity(&root, "loop:other"), None);
        fs::remove_dir_all(&root).expect("clean up");
    }
}
