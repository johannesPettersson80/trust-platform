//! The PID group of a loop (specification 35, C7): its declaration against the loop's channels, and the two group seeds that its
//! program law may name (`decentralized-pid` and `decoupled-pid`). Ported from the declaration rules of `pidControllerGroup.ts`
//! (`validPidControllerGroup`) and the seed rules of `projectController.ts`.

use serde_json::{json, Value};

use super::super::tests::{complete_loop, decode};
use crate::loop_definition::{
    ControllerProject, LoopChannels, PidControllerGroup, ProjectSeedBlock,
};

/// The loop's two input channels and two output channels, as a two-member group's setup writes them.
fn two_channels() -> LoopChannels {
    serde_json::from_value(json!({
        "inputs": [
            { "modelChannelId": "u1", "signalId": "manipulatedVariable",
              "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
            { "modelChannelId": "u2", "signalId": "mv-2", "manualOutputSignal": "P1.Zone2ManualPct", "manualOutputType": "LREAL",
              "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } }
        ],
        "outputs": [
            { "modelChannelId": "y1", "signalId": "processVariable", "setpointSignalId": "setpoint" },
            { "modelChannelId": "y2", "signalId": "pv-2", "setpointSignalId": "sp-2" }
        ]
    }))
    .expect("two channels")
}

/// One member of a group on the given channels, with its three gain variables of its own parameter record.
fn member(instance: &str, input: &str, output: &str) -> Value {
    json!({
        "instance": instance, "type": "Control.PID", "inputChannelId": input, "outputChannelId": output,
        "modeSymbol": "P1.ZonesAuto",
        "parameterSymbols": [
            format!("{instance}Parameters.ProportionalGain"),
            format!("{instance}Parameters.IntegralGain"),
            format!("{instance}Parameters.DerivativeGain"),
        ],
    })
}

/// The two-member group of the loop's channels, as the setup writes it.
fn two_member_group() -> Value {
    json!({ "members": [member("P1.Zone1", "u1", "y1"), member("P1.Zone2", "u2", "y2")] })
}

fn group(value: Value) -> PidControllerGroup {
    serde_json::from_value(value).expect("a typed group")
}

/// A program's own law that is designed by the project, with the seed given.
fn project_with_seed(seed: &Value) -> Value {
    json!({
        "family": "paired-PI",
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": seed,
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 1,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000,
    })
}

#[test]
fn a_declared_group_covers_each_input_and_output_channel_once() {
    assert_eq!(
        group(two_member_group()).check(Some(&two_channels())),
        Ok(())
    );
}

#[test]
fn a_group_without_channels_or_with_one_member_is_refused() {
    let declared = group(two_member_group());
    assert!(declared.check(None).is_err(), "no channels");
    let one = group(json!({ "members": [member("P1.Zone1", "u1", "y1")] }));
    assert!(one.check(Some(&two_channels())).is_err(), "one member");
    let none = group(json!({ "members": [] }));
    assert!(none.check(Some(&two_channels())).is_err(), "no member");
}

#[test]
fn an_instance_is_declared_once_whatever_its_case() {
    let twice = group(
        json!({ "members": [member("P1.Zone1", "u1", "y1"), member("p1.zone1", "u2", "y2")] }),
    );
    assert!(twice.check(Some(&two_channels())).is_err());
}

#[test]
fn a_channel_is_driven_or_measured_by_exactly_one_member() {
    let shared_input = group(
        json!({ "members": [member("P1.Zone1", "u1", "y1"), member("P1.Zone2", "u1", "y2")] }),
    );
    assert!(
        shared_input.check(Some(&two_channels())).is_err(),
        "u2 has no member"
    );
    let missing_output = group(
        json!({ "members": [member("P1.Zone1", "u1", "y1"), member("P1.Zone2", "u2", "y1")] }),
    );
    assert!(
        missing_output.check(Some(&two_channels())).is_err(),
        "y2 has no member"
    );
}

#[test]
fn a_member_is_a_control_pid_with_distinct_gain_variables() {
    let mut other_type = two_member_group();
    other_type["members"][1]["type"] = json!("Control.TON");
    assert!(group(other_type).check(Some(&two_channels())).is_err());
    let mut repeated = two_member_group();
    repeated["members"][0]["parameterSymbols"] = json!([
        "P1.Zone1Parameters.ProportionalGain",
        "p1.zone1parameters.proportionalgain"
    ]);
    assert!(group(repeated).check(Some(&two_channels())).is_err());
    let mut no_gain = two_member_group();
    no_gain["members"][0]["parameterSymbols"] = json!([]);
    assert!(group(no_gain).check(Some(&two_channels())).is_err());
}

#[test]
fn the_declaration_takes_only_its_own_fields() {
    let mut extra = two_member_group();
    extra["members"][0]["gains"] = json!(1);
    assert!(serde_json::from_value::<PidControllerGroup>(extra).is_err());
    let mut extra_group = two_member_group();
    extra_group["shared"] = json!(true);
    assert!(serde_json::from_value::<PidControllerGroup>(extra_group).is_err());
}

#[test]
fn the_decentralized_seed_is_served_and_names_only_the_family_pi() {
    let project = ControllerProject::from_value(&project_with_seed(
        &json!({ "kind": "decentralized-pid", "family": "PI" }),
    ))
    .expect("the block decodes");
    assert_eq!(project.seed, Some(ProjectSeedBlock::Decentralized));
    assert_eq!(
        project.parked_feature(),
        None,
        "the engine serves the group seed"
    );
    assert!(project.feedforward().is_none() && project.trajectory().is_none());
    let mut with_axis = project_with_seed(&json!({ "kind": "decentralized-pid", "family": "PI" }));
    with_axis["axes"] =
        json!([{ "symbol": "P1.Zone1Parameters.ProportionalGain", "values": [1.0] }]);
    let searched = ControllerProject::from_value(&with_axis).expect("the block decodes");
    assert!(
        searched.parked_feature().is_some(),
        "a group seed with tuning axes stays parked"
    );
    assert!(ControllerProject::from_value(&project_with_seed(
        &json!({ "kind": "decentralized-pid", "family": "P" })
    ))
    .is_err());
    assert!(ControllerProject::from_value(&project_with_seed(
        &json!({ "kind": "decentralized-pid", "family": "PI", "extra": 1 })
    ))
    .is_err());
}

#[test]
fn the_decoupled_seed_is_a_square_matrix_of_distinct_variables() {
    let seed = json!({ "kind": "decoupled-pid", "family": "PI",
        "matrixSymbols": [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]] });
    let project =
        ControllerProject::from_value(&project_with_seed(&seed)).expect("the decoupler decodes");
    let Some(ProjectSeedBlock::Decoupled(decoupler)) = &project.seed else {
        panic!("a decoupled seed, got {:?}", project.seed);
    };
    assert_eq!(
        decoupler.variables(),
        vec!["P1.D11", "P1.D12", "P1.D21", "P1.D22"]
    );
    assert_eq!(project.parked_feature(), None);
    let ragged = json!({ "kind": "decoupled-pid", "family": "PI", "matrixSymbols": [["P1.D11", "P1.D12"], ["P1.D21"]] });
    assert!(
        ControllerProject::from_value(&project_with_seed(&ragged)).is_err(),
        "not square"
    );
    let repeated = json!({ "kind": "decoupled-pid", "family": "PI", "matrixSymbols": [["P1.D11", "P1.D11"], ["P1.D21", "P1.D22"]] });
    assert!(
        ControllerProject::from_value(&project_with_seed(&repeated)).is_err(),
        "one variable twice"
    );
    let nine: Vec<Vec<String>> = (0..9)
        .map(|row| (0..9).map(|column| format!("P1.D{row}{column}")).collect())
        .collect();
    let too_large = json!({ "kind": "decoupled-pid", "family": "PI", "matrixSymbols": nine });
    assert!(
        ControllerProject::from_value(&project_with_seed(&too_large)).is_err(),
        "more than eight by eight"
    );
}

#[test]
fn a_group_seed_is_written_back_as_the_block_it_reads() {
    let seed = json!({ "kind": "decoupled-pid", "family": "PI", "matrixSymbols": [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]] });
    let block = project_with_seed(&seed);
    let project = ControllerProject::from_value(&block).expect("decodes");
    assert_eq!(project.to_value()["seed"], seed);
    let decentralized = json!({ "kind": "decentralized-pid", "family": "PI" });
    let served =
        ControllerProject::from_value(&project_with_seed(&decentralized)).expect("decodes");
    assert_eq!(served.to_value()["seed"], decentralized);
}

#[test]
fn a_group_seed_without_the_group_members_is_refused_when_the_loop_is_read() {
    // The decentralized and decoupled seeds design the members of `controller.group`, so a loop that names a seed and no group is refused.
    let mut value = complete_loop();
    let controller = value["controller"].as_object_mut().expect("the controller");
    controller.remove("law");
    controller.insert("design".to_owned(), json!("project"));
    controller.insert(
        "project".to_owned(),
        project_with_seed(&json!({ "kind": "decentralized-pid", "family": "PI" })),
    );
    let refusal =
        decode(&value).expect_err("the seed names members that the loop does not declare");
    assert!(
        refusal.to_string().contains("controller.group"),
        "{refusal}"
    );
}
