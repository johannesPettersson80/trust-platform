//! Tests of the controller adapter of the step-by-step setup (`configureControllerAdapter.ts`; C0 "Missing adapters").
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::adapters::controller_adapter;
use crate::loop_definition::{
    Direction, Execution, ExperimentKind, LoopDefinition, PidInitialization, RealType,
};
use crate::setup::catalog::{CompiledCatalog, CompiledSymbol};

/// The example of the connected PID commissioning, `examples/control-pid-commissioning/control-loop.json`.
const EXAMPLE: &str =
    include_str!("../../../../examples/control-pid-commissioning/control-loop.json");

fn example() -> LoopDefinition {
    LoopDefinition::decode(EXAMPLE).expect("the example is a loop definition")
}

fn real_word(value: RealType) -> &'static str {
    match value {
        RealType::Real => "REAL",
        RealType::Lreal => "LREAL",
    }
}

fn manual_word(value: ManualOutputType) -> &'static str {
    type_word(manual_type(value))
}

fn symbol(name: &str, iec_type: &str) -> CompiledSymbol {
    CompiledSymbol {
        symbol: name.into(),
        iec_type: iec_type.into(),
        address: None,
    }
}

/// The compiled catalog of the mapping, as `control-studio-library-pid.test.ts` builds it: the parameters, the mode, the
/// manual output and the transfer states of the loop's own adapter.
fn catalog_for(loop_definition: &LoopDefinition) -> CompiledCatalog {
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the example declares a parameter binding");
    let mut symbols: Vec<CompiledSymbol> = binding
        .parameters
        .iter()
        .map(|parameter| symbol(&parameter.symbol, real_word(parameter.iec_type)))
        .collect();
    symbols.push(symbol(&binding.mode.symbol, "BOOL"));
    if let (Some(name), Some(output_type)) = (
        &loop_definition.controller.manual_output_signal,
        loop_definition.controller.manual_output_type,
    ) {
        symbols.push(symbol(name, manual_word(output_type)));
    }
    let transfer = controller_adapter(loop_definition)
        .expect("the example has an adapter")
        .transfer_binding()
        .expect("the example has a transfer binding");
    symbols.extend(
        transfer
            .states
            .iter()
            .map(|state| symbol(&state.symbol, type_word(state.iec_type))),
    );
    CompiledCatalog {
        symbols,
        controllers: Vec::new(),
        links: Vec::new(),
        writes: Vec::new(),
    }
}

/// The mapping file the engineer picks: the loop's `controller` and `parameterBinding`, as the TypeScript test writes it.
fn mapping_of(loop_definition: &LoopDefinition) -> Value {
    serde_json::json!({
        "controller": serde_json::to_value(&loop_definition.controller).expect("a controller"),
        "parameterBinding": serde_json::to_value(&loop_definition.parameter_binding).expect("a binding"),
    })
}

#[test]
fn a_mapping_of_a_compiled_pid_is_imported_and_its_states_are_the_controllers() {
    // control-studio-library-pid.test.ts: "imports compiled nested PID fields through the public adapter setup": the
    // mapping imports, the loop's initialization is the library's, and the adapter has its ten transfer states.
    let source = example();
    let mapping = mapping_of(&source);
    let catalog = catalog_for(&source);
    let mut loop_definition = source.clone();
    apply_mapping(&mut loop_definition, &mapping, &catalog, Some("MainTask"))
        .expect("the mapping is imported");
    assert!(matches!(
        loop_definition.controller.pid_initialization,
        Some(PidInitialization::ControlPid { .. })
    ));
    let states = controller_adapter(&loop_definition)
        .expect("the adapter")
        .transfer_binding()
        .expect("the transfer binding")
        .states;
    assert_eq!(states.len(), 10);
}

#[test]
fn a_required_state_missing_from_the_compiled_catalog_refuses_the_mapping() {
    // control-studio-library-pid.test.ts: "every required nested state must actually exist in the compiled catalog".
    let source = example();
    let mapping = mapping_of(&source);
    let mut catalog = catalog_for(&source);
    catalog
        .symbols
        .retain(|candidate| !candidate.symbol.ends_with(".StateValue.NominalOutput"));
    let failure = apply_mapping(&mut source.clone(), &mapping, &catalog, Some("MainTask"))
        .expect_err("the missing state refuses the mapping");
    assert!(
        failure.message.contains("compiled scalar"),
        "{}",
        failure.message
    );
    assert_eq!(failure.cause, Cause::Implementation);
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn an_imported_mapping_writes_a_control_studio_observation_and_keeps_the_duration_it_has() {
    // configureControllerAdapter.ts: the manipulated variable is written, the experiment is the Control Studio observation
    // with the duration and the seed it had (the example's 16 s and seed).
    let source = example();
    let mapping = mapping_of(&source);
    let catalog = catalog_for(&source);
    let mut loop_definition = source.clone();
    apply_mapping(&mut loop_definition, &mapping, &catalog, Some("MainTask")).expect("imported");
    assert_eq!(
        loop_definition.signals.manipulated_variable.direction,
        Direction::Write
    );
    let experiment = loop_definition.experiment.expect("an experiment");
    assert_eq!(experiment.execution, Execution::ControlStudio);
    assert_eq!(experiment.kind, ExperimentKind::Observation);
    assert_eq!(experiment.amplitude, 0.0);
    assert_eq!(experiment.duration_seconds, Some(16.0));
    assert_eq!(experiment.seed, 20_260_824);
}

#[test]
fn an_absent_duration_stays_absent_because_identify_and_limits_choose_it() {
    // configureControllerAdapter.ts used `?? 10` for an absent duration; the spec says a maximum duration is never assumed
    // silently (B3.2, C1 Setup), so the port keeps it absent. The seed defaults to 1 as the TypeScript's does.
    let source = example();
    let mapping = mapping_of(&source);
    let catalog = catalog_for(&source);
    let mut loop_definition = source.clone();
    loop_definition.experiment = None;
    apply_mapping(&mut loop_definition, &mapping, &catalog, Some("MainTask")).expect("imported");
    let experiment = loop_definition.experiment.expect("an experiment");
    assert_eq!(experiment.duration_seconds, None);
    assert_eq!(experiment.seed, 1);
}

#[test]
fn a_mapping_symbol_missing_from_the_catalog_names_the_field_and_its_type() {
    // configureControllerAdapter.ts `check`: "Adapter field … must be a compiled scalar member of type …".
    let source = example();
    let mapping = mapping_of(&source);
    let first = source
        .parameter_binding
        .as_ref()
        .expect("a binding")
        .parameters[0]
        .clone();
    let mut catalog = catalog_for(&source);
    catalog
        .symbols
        .retain(|candidate| candidate.symbol != first.symbol);
    let failure = apply_mapping(&mut source.clone(), &mapping, &catalog, Some("MainTask"))
        .expect_err("the parameter is not compiled");
    assert_eq!(
        failure.message,
        texts::configure::adapter_symbol(&first.symbol, real_word(first.iec_type))
    );
}

#[test]
fn a_manual_output_missing_from_the_catalog_refuses_the_mapping() {
    // configureControllerAdapter.ts: the manual output is checked against the compiled catalog with its type.
    let source = example();
    let mapping = mapping_of(&source);
    let manual = source
        .controller
        .manual_output_signal
        .clone()
        .expect("the example has a manual output");
    let output_type = source.controller.manual_output_type.expect("and its type");
    let mut catalog = catalog_for(&source);
    catalog
        .symbols
        .retain(|candidate| candidate.symbol != manual);
    let failure = apply_mapping(&mut source.clone(), &mapping, &catalog, Some("MainTask"))
        .expect_err("the manual output is not compiled");
    assert_eq!(
        failure.message,
        texts::configure::adapter_symbol(&manual, manual_word(output_type))
    );
}

#[test]
fn a_mapping_without_both_objects_or_for_another_task_is_refused() {
    // configureControllerAdapter.ts: the `controller` and `parameterBinding` objects, and the selected task.
    let source = example();
    let catalog = catalog_for(&source);
    let only_controller = serde_json::json!({ "controller": {} });
    let failure = apply_mapping(
        &mut source.clone(),
        &only_controller,
        &catalog,
        Some("MainTask"),
    )
    .expect_err("the binding object is missing");
    assert_eq!(failure.message, texts::configure::adapter_objects());

    let mapping = mapping_of(&source);
    let failure = apply_mapping(&mut source.clone(), &mapping, &catalog, Some("OtherTask"))
        .expect_err("the task differs");
    assert_eq!(failure.message, texts::configure::adapter_task_mismatch());
}

#[test]
fn the_program_s_own_law_in_a_mapping_is_refused_by_name_as_parked() {
    // configureControllerAdapter.ts copies `project`; the Rust adapter refuses the program's own law by name (V7).
    let source = example();
    let mut mapping = mapping_of(&source);
    mapping["controller"]["project"] = serde_json::json!({});
    let catalog = catalog_for(&source);
    let failure = apply_mapping(&mut source.clone(), &mapping, &catalog, Some("MainTask"))
        .expect_err("the program's own law is parked");
    assert_eq!(failure.cause, Cause::ControllerCapability);
    assert_eq!(failure.next_action, NextAction::None);
}

#[test]
fn a_mapping_file_is_read_up_to_256_kb_and_must_be_json() {
    // configureControllerAdapter.ts: `bytesRead > 256000` is refused; the text is JSON.
    let mut at_limit = b"{}".to_vec();
    at_limit.resize(MAXIMUM_MAPPING_BYTES, b' ');
    assert!(
        mapping_from_bytes(&at_limit).is_ok(),
        "256 000 bytes are accepted"
    );

    let mut over = b"{}".to_vec();
    over.resize(MAXIMUM_MAPPING_BYTES + 1, b' ');
    let failure = mapping_from_bytes(&over).expect_err("256 001 bytes are refused");
    assert_eq!(
        failure.message,
        "Controller adapter mappings are limited to 256 kB."
    );

    let failure = mapping_from_bytes(b"{").expect_err("a truncated file is not JSON");
    assert!(failure
        .message
        .starts_with("The controller adapter mapping is not valid JSON:"));
}

#[test]
fn a_member_path_is_two_or_more_identifiers_separated_by_dots() {
    // configureControllerAdapter.ts: `^[\p{L}_][\p{L}\p{N}_]*(?:\.[\p{L}_][\p{L}\p{N}_]*)+$`.
    assert!(is_member_path("P1.AutoMode"));
    assert!(is_member_path("A.B_c1.d2"));
    assert!(!is_member_path("P1"), "a single identifier is not a member");
    assert!(!is_member_path("P1..X"));
    assert!(!is_member_path("1P.X"));
    assert!(!is_member_path("P1.X-Y"));
    assert!(!is_member_path(".X"));
}
