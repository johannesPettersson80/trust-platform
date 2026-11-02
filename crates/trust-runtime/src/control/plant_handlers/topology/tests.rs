use serde_json::{json, Value};

use super::*;

const PROJECT: &str = "project:sha256:topology";

/// The `plant.read` answer of a project whose `simulation.toml` is `simulation`.
fn read_of(name: &str, simulation: &str) -> Value {
    let root = std::env::temp_dir().join(format!(
        "trust-plant-topology-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create the project folder");
    std::fs::write(root.join("simulation.toml"), simulation).expect("write simulation.toml");
    let Ok(read) = saved_plants(1, &root, PROJECT) else {
        panic!("the saved plants of {name} are readable");
    };
    std::fs::remove_dir_all(&root).expect("remove the project folder");
    read
}

fn plant<'a>(plants: &'a [Value], id: &str) -> &'a Value {
    plants
        .iter()
        .find(|plant| plant["id"] == id)
        .unwrap_or_else(|| panic!("plant {id} is shown"))
}

const TANK_AND_PUMP: &str = r#"
[[plants]]
id = "tank"
name = "Tank"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = 1.0
time_constant_s = 5.0

[[plants]]
id = "pump"
name = "Pump"
model = "static_gain"
command_sources = ["%QL8"]
feedback_targets = ["%IL8"]

[plants.parameters]
gain = 2.0
"#;

/// A current `plant.status` answer of [`PROJECT`] with the given active revision and plant entries.
fn current_status(active_revision: &str, plants: Value) -> Value {
    json!({
        "contract": "trust.plant",
        "schema_version": 1,
        "operation": "plant.status",
        "project_id": PROJECT,
        "available": true,
        "active_configuration_revision": active_revision,
        "plants": plants,
    })
}

// Catches a status entry that draws a node with no saved plant, and live values joined to the
// wrong plant: specification 47 §20.6 joins status to a saved plant only by its project and ID.
#[test]
fn a_current_status_joins_only_the_saved_plant_it_names() {
    let read = read_of("join", TANK_AND_PUMP);
    let revision = read["configuration_revision"]
        .as_str()
        .expect("saved revision");
    let status = current_status(
        revision,
        json!([
            { "id": "ghost", "operating_state": "running", "simulation_time_ns": "2000000", "command": [9.0], "published_feedback": [9.0] },
            { "id": "tank", "operating_state": "running", "simulation_time_ns": "2000000", "command": [1.5], "published_feedback": [2.25] },
        ]),
    );

    let plants = project(&read, Some(&status));

    let ids = plants
        .iter()
        .map(|plant| plant["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![json!("tank"), json!("pump")],
        "saved order, no ghost"
    );
    let tank = plant(&plants, "tank");
    assert_eq!(tank["projectId"], PROJECT);
    assert_eq!(tank["displayName"], "Tank");
    assert_eq!(tank["model"], "first_order");
    assert_eq!(tank["enabled"], true);
    assert_eq!(tank["commandSources"], json!(["%QL0"]));
    assert_eq!(tank["feedbackTargets"], json!(["%IL0"]));
    assert_eq!(tank["statusAvailability"], "available");
    assert_eq!(tank["savedConfigurationRevision"], revision);
    assert_eq!(tank["activeConfigurationRevision"], revision);
    assert_eq!(tank["activeOperatingState"], "running");
    assert_eq!(tank["hasCompletedSample"], true);
    assert_eq!(
        tank["liveSample"],
        json!({ "command": [1.5], "feedback": [2.25] })
    );
    assert_eq!(tank["states"], json!(["Running"]));

    // The current status answers for the pump too: not running, so neither unknown nor live.
    let pump = plant(&plants, "pump");
    assert_eq!(pump["statusAvailability"], "available");
    assert_eq!(pump["states"], json!(["Configured"]));
    for live in ["activeOperatingState", "hasCompletedSample", "liveSample"] {
        assert!(pump.get(live).is_none(), "pump has no {live}");
    }
}

// Catches live values or an active revision taken from a status that is not current. The
// TypeScript projection took `active_configuration_revision` from a wrong-project status, which
// showed a false `Restart required` (specification 47 §20.6).
#[test]
fn a_status_that_is_not_current_marks_every_plant_unavailable() {
    let read = read_of("stale", TANK_AND_PUMP);
    let tank_running = json!([
        { "id": "tank", "operating_state": "running", "command": [1.5], "published_feedback": [2.25] },
    ]);
    let current = current_status("sha256:other", tank_running);
    let mut other_project = current.clone();
    other_project["project_id"] = json!("project:sha256:another");
    let mut not_available = current.clone();
    not_available["available"] = json!(false);
    let mut other_operation = current.clone();
    other_operation["operation"] = json!("plant.trace.read");
    let mut other_schema = current.clone();
    other_schema["schema_version"] = json!(2);
    let mut other_contract = current.clone();
    other_contract["contract"] = json!("trust.other");
    let statuses = [
        ("another project", Some(other_project)),
        ("not available", Some(not_available)),
        ("another operation", Some(other_operation)),
        ("another schema", Some(other_schema)),
        ("another contract", Some(other_contract)),
        ("not an answer", Some(json!("timeout"))),
        ("none", None),
    ];

    for (case, status) in &statuses {
        let plants = project(&read, status.as_ref());
        let tank = plant(&plants, "tank");
        assert_eq!(tank["statusAvailability"], "unavailable", "{case}");
        assert_eq!(tank["states"], json!(["Status unavailable"]), "{case}");
        for live in [
            "activeConfigurationRevision",
            "activeOperatingState",
            "hasCompletedSample",
            "liveSample",
        ] {
            assert!(tank.get(live).is_none(), "{case}: no {live}");
        }
    }
}

// Catches a plant's starting values shown as live values, and `Waiting for samples` never shown:
// a plant whose simulation has not advanced holds no completed sample (specification 47 §5.2,
// §5.3).
#[test]
fn a_plant_before_its_first_step_is_waiting_for_samples() {
    let read = read_of("first-step", TANK_AND_PUMP);
    let revision = read["configuration_revision"]
        .as_str()
        .expect("saved revision");
    let at = |nanoseconds: &str| {
        let status = current_status(
            revision,
            json!([
                { "id": "tank", "operating_state": "running", "simulation_time_ns": nanoseconds, "command": [0.0], "published_feedback": [0.0] },
            ]),
        );
        project(&read, Some(&status))
    };

    let before = at("0");
    let tank = plant(&before, "tank");
    assert_eq!(tank["states"], json!(["Waiting for samples"]));
    assert_eq!(tank["activeOperatingState"], "running");
    for live in ["hasCompletedSample", "liveSample"] {
        assert!(tank.get(live).is_none(), "no {live} before the first step");
    }

    let after = at("1");
    let tank = plant(&after, "tank");
    assert_eq!(tank["states"], json!(["Running"]));
    assert_eq!(
        tank["liveSample"],
        json!({ "command": [0.0], "feedback": [0.0] })
    );
}

// Catches a plant that the runtime does not run reported as `Status unavailable` although the
// runtime answered: a disabled plant is `Disabled`, and one saved after the runtime started waits
// for a restart (specification 47 §5.2).
#[test]
fn a_plant_the_current_status_does_not_run_is_not_unknown() {
    let simulation = format!("{TANK_AND_PUMP}\n[[plants]]\nid = \"valve\"\nmodel = \"static_gain\"\nenabled = false\ncommand_sources = [\"%QL16\"]\nfeedback_targets = [\"%IL16\"]\n\n[plants.parameters]\ngain = 1.0\n");
    let read = read_of("not-running", &simulation);
    let revision = read["configuration_revision"]
        .as_str()
        .expect("saved revision");

    let same = current_status(revision, json!([]));
    let plants = project(&read, Some(&same));
    assert_eq!(plant(&plants, "valve")["states"], json!(["Disabled"]));
    assert_eq!(plant(&plants, "pump")["states"], json!(["Configured"]));

    let older = current_status("sha256:older", json!([]));
    let plants = project(&read, Some(&older));
    assert_eq!(
        plant(&plants, "valve")["states"],
        json!(["Restart required", "Disabled"])
    );
    assert_eq!(
        plant(&plants, "pump")["states"],
        json!(["Restart required"])
    );
}

fn evidence() -> PlantEvidence<'static> {
    PlantEvidence {
        enabled: true,
        available: true,
        operating_state: Some("running"),
        completed_sample: true,
        saved_revision: Some("sha256:saved"),
        active_revision: Some("sha256:saved"),
    }
}

// Catches a primary state chosen in another order: specification 47 §5.2 orders Faulted, Restart
// required, Disabled, Status unavailable, Waiting for samples, Running, then Configured, and keeps
// every other true state as a detail.
#[test]
fn a_plants_states_follow_the_specified_order() {
    let cases = [
        (
            "a fault is not hidden by a revision mismatch",
            PlantEvidence {
                enabled: false,
                operating_state: Some("faulted"),
                active_revision: Some("sha256:active"),
                ..evidence()
            },
            vec!["Faulted", "Restart required", "Disabled"],
        ),
        (
            "disabling a plant that still runs needs a restart first",
            PlantEvidence {
                enabled: false,
                active_revision: Some("sha256:active"),
                ..evidence()
            },
            vec!["Restart required", "Disabled", "Running"],
        ),
        (
            "running without a completed sample is waiting",
            PlantEvidence {
                completed_sample: false,
                ..evidence()
            },
            vec!["Waiting for samples"],
        ),
        (
            "running with a completed sample",
            evidence(),
            vec!["Running"],
        ),
        (
            "no status",
            PlantEvidence {
                available: false,
                operating_state: None,
                completed_sample: false,
                active_revision: None,
                ..evidence()
            },
            vec!["Status unavailable"],
        ),
        (
            "a fault needs current status",
            PlantEvidence {
                available: false,
                operating_state: Some("faulted"),
                ..evidence()
            },
            vec!["Status unavailable"],
        ),
        (
            "an empty active revision names none",
            PlantEvidence {
                active_revision: Some(""),
                ..evidence()
            },
            vec!["Running"],
        ),
        (
            "a disabled plant that the current status does not run",
            PlantEvidence {
                enabled: false,
                operating_state: None,
                completed_sample: false,
                ..evidence()
            },
            vec!["Disabled"],
        ),
        (
            "nothing else holds",
            PlantEvidence {
                operating_state: None,
                ..evidence()
            },
            vec!["Configured"],
        ),
    ];

    for (case, evidence, expected) in cases {
        assert_eq!(states(&evidence), expected, "{case}");
    }
}

// Catches the editor losing what it does not manage when it reopens a saved plant (the `validity`
// table, specification 47 §13.2), another plant's scenario event in this plant's list, and a saved
// uncertainty reopened as excluded (§11, §12).
#[test]
fn a_saved_plant_opens_with_its_whole_saved_table() {
    let read = read_of(
        "reopen",
        r#"
[[plants]]
id = "custom"
name = "Custom model"
description = "Existing model"
model = "state_space"
step_ms = 5
initial_state = [1.0, 2.0]
initial_input = [0.0, 0.0]
input_scale = [1.0, 2.0]
input_offset = [0.0, 3.0]
output_scale = [4.0, 5.0]
output_offset = [6.0, 7.0]
command_sources = ["%QL0", "%QL8"]
feedback_targets = ["%IL0", "%IL8"]

[plants.parameters]
a = [[-1.0, 0.0], [0.0, -2.0]]
b = [[1.0, 0.0], [0.0, 1.0]]
c = [[1.0, 0.0], [0.0, 1.0]]
d = [[0.0, 0.0], [0.0, 0.0]]

[plants.validity]
state_minimum = [-10.0, -20.0]
state_maximum = [10.0, 20.0]

[plants.actuator]
rate_limit_per_s = 2.5

[[plants.uncertainties]]
kind = "measurement_noise"
channel = 0
amplitude = 0.1

[[plant_events]]
plant = "custom"
at_ms = 1500
kind = "set_input_offset"
channel = 1
value = 0.5

[[plant_events]]
plant = "other"
at_ms = 100
kind = "fault"
"#,
    );

    let plants = project(&read, None);

    let configuration = &plant(&plants, "custom")["configuration"];
    let saved = &read["saved_configuration"]["plants"][0];
    assert_eq!(
        saved["validity"]["state_minimum"],
        json!([-10.0, -20.0]),
        "the fixture's saved table holds a field the editor does not manage"
    );
    assert_eq!(configuration["preservedConfiguration"], *saved);
    assert_eq!(configuration["description"], "Existing model");
    assert_eq!(
        configuration["parameters"],
        json!({
            "a": [[-1.0, 0.0], [0.0, -2.0]],
            "b": [[1.0, 0.0], [0.0, 1.0]],
            "c": [[1.0, 0.0], [0.0, 1.0]],
            "d": [[0.0, 0.0], [0.0, 0.0]],
        })
    );
    assert_eq!(configuration["initialState"], json!([1.0, 2.0]));
    assert_eq!(configuration["stepMs"], 5);
    assert_eq!(configuration["initialInput"], json!([0.0, 0.0]));
    assert_eq!(configuration["inputScale"], json!([1.0, 2.0]));
    assert_eq!(configuration["inputOffset"], json!([0.0, 3.0]));
    assert_eq!(configuration["outputScale"], json!([4.0, 5.0]));
    assert_eq!(configuration["outputOffset"], json!([6.0, 7.0]));
    assert_eq!(
        configuration["actuator"],
        json!({ "rate_limit_per_s": 2.5 })
    );
    assert_eq!(
        configuration["uncertainties"],
        json!([{ "kind": "measurement_noise", "channel": 0, "amplitude": 0.1, "enabled": true }])
    );
    assert_eq!(
        configuration["scenarioEvents"],
        json!([{ "enabled": true, "atMs": 1500, "kind": "set_input_offset", "channel": 1, "value": 0.5 }])
    );
    assert_eq!(
        configuration["enabledDefaulted"], true,
        "`enabled` is not saved"
    );
    assert!(configuration.get("initialStateDefaulted").is_none());
}

// Catches an explicit initial state replaced by the default, and a malformed one taken for an
// omitted one: Rust's `open` edit sizes only an omitted state (specification 47 §9).
#[test]
fn only_an_omitted_initial_state_reaches_the_editor_as_omitted() {
    let models = [
        ("static_gain", "gain = 2.0"),
        ("first_order", "time_constant_s = 2.0"),
        (
            "first_order_dead_time",
            "time_constant_s = 2.0\ndead_time_s = 0.5",
        ),
        ("integrator", "gain = 2.0"),
        (
            "gravity_tank",
            "area_m2 = 2.0\noutflow_coefficient_m2_sqrt_m_per_s = 0.5",
        ),
        (
            "piecewise_linear",
            "input_points = [0.0, 1.0]\noutput_points = [0.0, 1.0]",
        ),
        (
            "robot_joint",
            "inertia_kg_m2 = 2.0\nfriction_smoothing_rad_s = 0.01",
        ),
        (
            "second_order",
            "natural_frequency_rad_s = 2.0\ndamping_ratio = 0.7",
        ),
        (
            "state_space",
            "a = [[0.0, 1.0], [-1.0, -1.0]]\nb = [[0.0], [1.0]]\nc = [[1.0, 0.0]]\nd = [[0.0]]",
        ),
    ];
    let mut simulation = String::new();
    for (model, parameters) in models {
        simulation.push_str(&format!(
            "[[plants]]\nid = \"omitted-{model}\"\nmodel = \"{model}\"\nenabled = true\n\
             command_sources = [\"%QL0\"]\nfeedback_targets = [\"%IL0\"]\n\
             [plants.parameters]\n{parameters}\n\n"
        ));
    }
    for (id, state) in [("zero", "[0.0]"), ("empty", "[]"), ("word", "\"zero\"")] {
        simulation.push_str(&format!(
            "[[plants]]\nid = \"{id}\"\nmodel = \"static_gain\"\ninitial_state = {state}\n\
             command_sources = [\"%QL0\"]\nfeedback_targets = [\"%IL0\"]\n\
             [plants.parameters]\ngain = 2.0\n\n"
        ));
    }
    let read = read_of("initial-state", &simulation);

    let plants = project(&read, None);

    for (model, _) in models {
        let configuration = &plant(&plants, &format!("omitted-{model}"))["configuration"];
        assert_eq!(configuration["initialState"], json!([]), "{model}");
        assert_eq!(configuration["initialStateDefaulted"], true, "{model}");
        assert!(
            configuration.get("enabledDefaulted").is_none(),
            "{model}: `enabled` is saved"
        );
    }
    assert_eq!(
        plant(&plants, "omitted-state_space")["configuration"]["parameters"]["a"],
        json!([[0.0, 1.0], [-1.0, -1.0]])
    );
    for (id, state) in [("zero", json!([0.0])), ("empty", json!([]))] {
        let configuration = &plant(&plants, id)["configuration"];
        assert_eq!(configuration["initialState"], state, "{id}");
        assert!(configuration.get("initialStateDefaulted").is_none(), "{id}");
    }
    assert!(
        plant(&plants, "word").get("configuration").is_none(),
        "a word is not an initial state"
    );
}

// Catches a node for a saved plant the canvas cannot draw (a missing model, an empty ID, name or
// model: specification 47 §20.1, an empty identifier is invalid), and an editor opened on model
// values that are not numbers.
#[test]
fn an_incomplete_saved_plant_is_left_out_and_wordy_values_open_no_editor() {
    let read = read_of(
        "incomplete",
        r#"
[[plants]]
id = "nomodel"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = 1.0

[[plants]]
id = "wordy"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[plants.parameters]
gain = "two"

[[plants]]
id = ""
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[[plants]]
id = "unnamed"
name = ""
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]

[[plants]]
id = "modelless"
model = ""
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
"#,
    );

    let plants = project(&read, None);

    let ids = plants
        .iter()
        .map(|plant| plant["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![json!("wordy")]);
    assert!(plant(&plants, "wordy").get("configuration").is_none());
}
