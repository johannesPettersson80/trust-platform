//! Tests of the Smith predictor's generator (specification 35, C9; `smithPredictorGenerator.ts`) on a compiled heater program: the seed, the
//! parameters and the correction state the generator writes, the library block's variables when the program names them, the refusal of a
//! repeated variable, and the round trip through the loop file, which the Rust engine serves. The numbers are those of the `TypeScript`
//! generator's test (`control-studio-smith-predictor.test.ts`).

#![allow(clippy::float_cmp)] // The generated bounds and gains are compared with their exact values.

use serde_json::{json, Value};

use super::*;
use crate::failure::Failure;
use crate::loop_definition::{InitialValue, LoopDefinition, LoopScope, PostScan, ProjectSeedBlock};
use crate::setup::catalog::{decode_catalog_answer, CompiledCatalog};
use crate::setup::save::{parse_setup_draft, plan_setup_save, SetupDraft};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:setup".to_string(),
        runtime_id: "runtime-1".to_string(),
    }
}

/// The sentence a refusal shows: its whole text, which is the detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literal its caller writes in place.
fn decode(control: Value) -> CompiledCatalog {
    decode_catalog_answer(&json!({ "ok": true, "processImage": {}, "controlSymbols": control }))
        .expect("the compiled answer decodes")
        .control
}

/// The heater of the predictor's tests: the water through the heater command, the program's model and correction, and the library block's
/// variables when `library` is set (the result, the maximum correction and the operating point the block takes).
fn heater(library: bool) -> CompiledCatalog {
    let mut symbols = vec![
        json!({ "symbol": "P1.WaterC", "type": "LREAL", "address": "%IL0" }),
        json!({ "symbol": "P1.SetpointC", "type": "LREAL", "address": "%ML0" }),
        json!({ "symbol": "P1.HeaterPct", "type": "LREAL", "address": "%QL0" }),
        json!({ "symbol": "P1.ManualPct", "type": "LREAL", "address": "%ML16" }),
        json!({ "symbol": "P1.Auto", "type": "BOOL" }),
        json!({ "symbol": "P1.Initialized", "type": "BOOL" }),
        json!({ "symbol": "P1.ModelGain", "type": "LREAL" }),
        json!({ "symbol": "P1.ModelTimeConstantS", "type": "LREAL" }),
        json!({ "symbol": "P1.ModelDelayS", "type": "LREAL" }),
        json!({ "symbol": "P1.CorrectionC", "type": "LREAL" }),
        json!({ "symbol": "P1.PredictedC", "type": "LREAL" }),
    ];
    if library {
        symbols.extend([
            json!({ "symbol": "P1.SmithOut", "type": "SmithPredictorOutput" }),
            json!({ "symbol": "P1.MaxCorrectionC", "type": "LREAL" }),
            json!({ "symbol": "P1.OperatingPct", "type": "LREAL" }),
            json!({ "symbol": "P1.OperatingC", "type": "LREAL" }),
        ]);
    }
    decode(json!({
        "symbols": symbols,
        "controllers": [
            { "instance": "P1.Loop", "type": "Control.PID", "tasks": [{ "name": "MainTask", "intervalMs": 100 }] }
        ],
        "links": [],
        "writes": ["P1.HeaterPct", "P1.CorrectionC", "P1.PredictedC"]
    }))
}

/// The draft of the predictor as the setup form posts it, without its structure (C9).
fn body() -> Value {
    json!({
        "name": "Water temperature",
        "instance": "P1.Loop",
        "action": "direct",
        "parameterRecord": "P1.Parameters",
        "processVariable": { "symbol": "P1.WaterC", "unit": "degC", "minimum": 0, "maximum": 80 },
        "manipulatedVariable": { "symbol": "P1.HeaterPct", "unit": "%", "minimum": 0, "maximum": 100 },
        "setpoint": { "symbol": "P1.SetpointC" },
        "mode": { "symbol": "P1.Auto", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualPct",
        "model": { "gain": "P1.ModelGain", "timeConstant": "P1.ModelTimeConstantS", "delay": "P1.ModelDelayS", "correction": "P1.CorrectionC" }
    })
}

fn draft() -> SmithLoopDraft {
    serde_json::from_value(body()).expect("the draft decodes")
}

/// The draft with its library block: the result, the maximum correction and the operating point of `Control.SmithPredictorFOPDT`.
fn library_draft() -> SmithLoopDraft {
    let mut value = body();
    value["libraryBlock"] = json!({
        "output": "P1.SmithOut",
        "maximumCorrection": "P1.MaxCorrectionC",
        "operatingInput": "P1.OperatingPct",
        "operatingOutput": "P1.OperatingC"
    });
    serde_json::from_value(value).expect("the draft with its library block decodes")
}

#[test]
fn the_draft_of_the_form_plans_one_predictor_loop_that_decodes_from_its_file() {
    let mut value = body();
    value["structure"] = json!("smith");
    let SetupDraft::Smith(parsed) = parse_setup_draft(&value).expect("the draft decodes") else {
        panic!("a smith draft is read as the predictor");
    };
    let planned = plan_setup_save(&heater(false), &scope(), &SetupDraft::Smith(parsed))
        .expect("the predictor is planned");
    assert_eq!(planned.len(), 1);
    let loop_definition = &planned[0].loop_definition;
    let text = serde_json::to_string(loop_definition).expect("the loop serializes");
    let decoded = LoopDefinition::decode(&text).expect("the loop file decodes");
    decoded
        .rust_engine_support()
        .expect("the Rust engine serves the predictor");
    decoded.validate(&scope()).expect("the loop file validates");
    assert_eq!(
        decoded
            .controller
            .project
            .as_ref()
            .map(|project| project.family.as_str()),
        Some("Smith")
    );
}

#[test]
fn the_seed_names_the_pi_and_the_model_and_the_loop_keeps_the_process_measurement() {
    let loop_definition = smith_loop_definition(&heater(false), &scope(), &draft())
        .expect("the predictor is generated");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a predictor declares its project block");
    assert_eq!(project.family, "Smith");
    assert_eq!(
        project.seed,
        Some(ProjectSeedBlock::Smith(
            crate::tuning::project_seed::SmithPidSeed {
                pid: crate::tuning::project_seed::ParallelPidSeed {
                    family: crate::tuning::structure::Structure::PI,
                    proportional_symbol: "P1.Parameters.ProportionalGain".to_owned(),
                    integral_symbol: Some("P1.Parameters.IntegralGain".to_owned()),
                    derivative_symbol: Some("P1.Parameters.DerivativeGain".to_owned()),
                },
                gain_symbol: "P1.ModelGain".to_owned(),
                time_constant_symbol: "P1.ModelTimeConstantS".to_owned(),
                delay_symbol: "P1.ModelDelayS".to_owned(),
                library_block: None,
            }
        ))
    );
    assert_eq!(loop_definition.signals.process_variable.symbol, "P1.WaterC");
    assert_eq!(project.objective.requirement_id, "target-error");
}

#[test]
fn the_model_values_are_bound_with_their_units_and_bounds_and_the_correction_starts_at_zero() {
    let loop_definition = smith_loop_definition(&heater(false), &scope(), &draft())
        .expect("the predictor is generated");
    let parameters = &loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter binding")
        .parameters;
    let model: Vec<(&str, &str, f64, f64)> = parameters
        .iter()
        .skip(3)
        .map(|parameter| {
            (
                parameter.symbol.as_str(),
                parameter.unit.as_str(),
                parameter.minimum,
                parameter.maximum,
            )
        })
        .collect();
    assert_eq!(
        model,
        vec![
            ("P1.ModelGain", "degC/%", -1e6, 1e6),
            ("P1.ModelTimeConstantS", "s", 0.001, 1e5),
            ("P1.ModelDelayS", "s", 0.0, 1e4),
        ]
    );
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the project");
    let correction = project
        .initialization
        .states()
        .iter()
        .find(|state| state.symbol == "P1.CorrectionC")
        .expect("the correction is a declared state");
    assert_eq!(correction.initial, InitialValue::Constant(0.0));
    assert_eq!(correction.unit, "degC");
    assert_eq!(correction.minimum, -80.0);
    assert_eq!(correction.maximum, 80.0);
    assert_eq!(correction.post_scan, PostScan::Bounded);
}

#[test]
fn the_library_block_adds_its_result_maximum_and_operating_point_to_the_seed_and_the_record() {
    let loop_definition = smith_loop_definition(&heater(true), &scope(), &library_draft())
        .expect("the predictor with its library block is generated");
    let project = loop_definition
        .controller
        .project
        .as_ref()
        .expect("the project");
    let Some(ProjectSeedBlock::Smith(seed)) = project.seed.as_ref() else {
        panic!("a Smith seed");
    };
    let block = seed.library_block.as_ref().expect("the library block");
    assert_eq!(block.output_symbol, "P1.SmithOut");
    assert_eq!(block.maximum_correction_symbol, "P1.MaxCorrectionC");
    assert_eq!(block.operating_input_symbol, "P1.OperatingPct");
    assert_eq!(block.operating_output_symbol, "P1.OperatingC");
    let parameters = &loop_definition
        .parameter_binding
        .as_ref()
        .expect("a parameter binding")
        .parameters;
    let operating: Vec<(&str, &str)> = parameters
        .iter()
        .filter(|parameter| parameter.symbol.starts_with("P1.Operating"))
        .map(|parameter| (parameter.symbol.as_str(), parameter.unit.as_str()))
        .collect();
    assert_eq!(
        operating,
        vec![("P1.OperatingPct", "%"), ("P1.OperatingC", "degC")]
    );
}

#[test]
fn a_program_that_writes_a_seeded_setpoint_needs_the_startup_flag() {
    // C1 Setup: validation seeds the setpoint, so a program that writes it needs the startup-done flag, as a PID loop does.
    let mut catalog = heater(false);
    catalog.writes.push("P1.SetpointC".to_string());
    let failure = smith_loop_definition(&catalog, &scope(), &draft())
        .expect_err("the program writes a seeded value");
    assert_eq!(
        failure.message,
        "The program writes P1.SetpointC in its own code."
    );
    let flagged = SmithLoopDraft {
        startup_flag: Some("P1.Initialized".to_string()),
        ..draft()
    };
    smith_loop_definition(&catalog, &scope(), &flagged)
        .expect("with the flag the predictor is generated");
}

#[test]
fn a_predictor_whose_variables_repeat_or_whose_block_names_an_uncompiled_variable_is_refused() {
    let mut repeated = body();
    repeated["model"]["delay"] = json!("P1.ModelGain");
    let repeated: SmithLoopDraft = serde_json::from_value(repeated).expect("the draft decodes");
    let refusal = smith_loop_definition(&heater(false), &scope(), &repeated)
        .expect_err("a variable is one term only");
    assert_eq!(
        text_of(&refusal),
        "The predictor's model variables and correction must differ from each other and from the loop's signals."
    );

    let mut uncompiled = library_draft();
    uncompiled.library_block = Some(SmithLibraryVariables {
        output: "P1.Missing".to_owned(),
        maximum_correction: "P1.MaxCorrectionC".to_owned(),
        operating_input: "P1.OperatingPct".to_owned(),
        operating_output: "P1.OperatingC".to_owned(),
    });
    assert!(smith_loop_definition(&heater(true), &scope(), &uncompiled).is_err());
}
