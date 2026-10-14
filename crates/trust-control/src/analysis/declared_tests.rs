use std::collections::BTreeMap;

use super::*;
use crate::model::{ModelSource, ModelValidation};

/// The shaft of control-studio-declared-analysis.test.ts: 1/(s² + s) with the controller Kd·s + Kp.
fn shaft() -> DeclaredControlAnalysis {
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "Kp".to_string(),
        DeclaredParameter {
            value: 12.0,
            minimum: 0.0,
            maximum: 100.0,
            unit: "N·m/rad".to_string(),
        },
    );
    parameters.insert(
        "Kd".to_string(),
        DeclaredParameter {
            value: 7.0,
            minimum: 0.0,
            maximum: 20.0,
            unit: "N·m·s/rad".to_string(),
        },
    );
    DeclaredControlAnalysis {
        plant: DeclaredPlant {
            kind: PlantKind::TransferFunction,
            numerator: vec![1.0],
            denominator: vec![1.0, 1.0, 0.0],
            delay_seconds: 0.0,
            input_unit: "N·m".to_string(),
            output_unit: "rad".to_string(),
            revision: "shaft-1".to_string(),
            source: ModelSource::Provided,
            validation: ModelValidation::Unvalidated,
        },
        controller: DeclaredController {
            numerator: vec![parameter("Kd"), parameter("Kp")],
            denominator: vec![DeclaredCoefficient::Constant(1.0)],
            parameters,
        },
        root_locus: DeclaredSweep {
            parameter: "Kp".to_string(),
            values: vec![12.0, 16.0, 20.0],
            selected_value: 16.0,
        },
    }
}

fn parameter(name: &str) -> DeclaredCoefficient {
    DeclaredCoefficient::Parameter(ParameterCoefficient {
        parameter: name.to_string(),
        scale: None,
    })
}

fn set_parameter(
    analysis: &mut DeclaredControlAnalysis,
    name: &str,
    minimum: f64,
    maximum: f64,
    value: f64,
) {
    if let Some(entry) = analysis.controller.parameters.get_mut(name) {
        entry.minimum = minimum;
        entry.maximum = maximum;
        entry.value = value;
    }
}

#[test]
fn a_fixed_parameter_may_be_exact_while_another_gain_is_swept() {
    // Kd fixed at 7 with minimum = maximum = 7 is valid; the calculation reports it as fixed.
    let mut analysis = shaft();
    set_parameter(&mut analysis, "Kd", 7.0, 7.0, 7.0);
    assert!(check_declared(&analysis).is_ok());
    let result = calculate_declared_root_locus(&analysis).expect("the pole paths");
    assert_eq!(result.fixed_parameters["Kd"].value, 7.0);
}

#[test]
fn the_shaft_poles_at_each_gain_and_the_unvalidated_provenance() {
    // The characteristic polynomial is s² + (Kd·…)s + … : at Kp = 12, s² + 8s + 12 has poles −6 and −2.
    let result = calculate_declared_root_locus(&shaft()).expect("the pole paths");
    let gains: Vec<f64> = result.branches.iter().map(|branch| branch.gain).collect();
    assert_eq!(gains, vec![12.0, 16.0, 20.0]);
    let expected = [
        [(-6.0, 0.0), (-2.0, 0.0)],
        [(-4.0, 0.0), (-4.0, 0.0)],
        [(-4.0, -2.0), (-4.0, 2.0)],
    ];
    for (branch, poles) in result.branches.iter().zip(expected) {
        for (pole, (real, imaginary)) in branch.poles.iter().zip(poles) {
            assert!((pole.real - real).abs() < 1e-6, "{pole:?}");
            assert!((pole.imaginary - imaginary).abs() < 1e-6, "{pole:?}");
        }
    }
    assert!(result.provenance.contains("provided") && result.provenance.contains("unvalidated"));
}

#[test]
fn the_pole_paths_cover_the_declared_range_in_201_evenly_spaced_gains() {
    // Ported: Kd fixed at 7, Kp swept over 0 to 20 on 1/(s² + 8s); the path samples Kp = 0, 16 and 20 at 0, 160, 200.
    let mut analysis = shaft();
    analysis.plant.denominator = vec![1.0, 8.0, 0.0];
    analysis.controller.numerator = vec![parameter("Kp")];
    set_parameter(&mut analysis, "Kp", 0.0, 20.0, 16.0);
    set_parameter(&mut analysis, "Kd", 7.0, 7.0, 7.0);
    analysis.root_locus.values = vec![12.0, 16.0, 20.0];
    let result = calculate_declared_root_locus(&analysis).expect("the pole paths");
    assert_eq!(result.path.len(), 201);
    assert_eq!(
        [
            result.path[0].gain,
            result.path[160].gain,
            result.path[200].gain
        ],
        [0.0, 16.0, 20.0]
    );
    let at_zero: Vec<f64> = result.path[0].poles.iter().map(|pole| pole.real).collect();
    assert!(
        (at_zero[0] + 8.0).abs() < 1e-6 && at_zero[1].abs() < 1e-6,
        "{at_zero:?}"
    );
    assert!(result.path[200]
        .poles
        .iter()
        .all(|pole| (pole.real + 4.0).abs() < 1e-6));
}

#[test]
fn an_unsupported_coefficient_operation_is_not_silently_ignored() {
    // The coefficient `{ parameter, offset }` is not a declared form: the record refuses the unknown key.
    let refused = serde_json::from_value::<ParameterCoefficient>(
        serde_json::json!({ "parameter": "Kp", "offset": 10 }),
    );
    assert!(refused.is_err());
}

#[test]
fn a_sweep_denominator_must_be_defined_at_every_gain() {
    let mut analysis = shaft();
    analysis.controller.denominator = vec![parameter("Kp")];
    analysis.root_locus.values = vec![0.0, 12.0];
    analysis.root_locus.selected_value = 12.0;
    assert!(check_declared(&analysis).is_err());
}

#[test]
fn unknown_parameters_out_of_range_candidates_and_unbounded_sweeps_are_refused() {
    let base = shaft();
    let mut refusals = Vec::new();
    let mut unknown = base.clone();
    unknown.root_locus.parameter = "missing".to_string();
    refusals.push(unknown);
    let mut outside = base.clone();
    outside.root_locus.values = vec![-1.0, 16.0];
    refusals.push(outside);
    let mut empty = base.clone();
    empty.root_locus.values = Vec::new();
    refusals.push(empty);
    let mut not_a_number = base.clone();
    not_a_number.root_locus.values = vec![12.0, f64::NAN, 16.0];
    refusals.push(not_a_number);
    let mut too_many = base.clone();
    too_many.root_locus.values = vec![16.0; 1_002];
    refusals.push(too_many);
    let mut unselected = base.clone();
    unselected.root_locus.selected_value = 18.0;
    refusals.push(unselected);
    for refused in &refusals {
        assert!(check_declared(refused).is_err(), "{refused:?}");
    }
    let mut zero_denominator = base;
    zero_denominator.plant.denominator = vec![0.0];
    assert!(check_declared(&zero_denominator).is_err());
}

#[test]
fn a_saved_pole_path_must_match_its_definition() {
    let definition = shaft();
    let result = calculate_declared_root_locus(&definition).expect("the pole paths");
    let saved = SavedDeclaredAnalysis {
        definition: definition.clone(),
        result: result.clone(),
    };
    assert!(check_saved_declared(&saved).is_ok());
    let mut changed = saved.clone();
    changed.result.selected_value = 12.0;
    assert!(check_saved_declared(&changed).is_err());
}
