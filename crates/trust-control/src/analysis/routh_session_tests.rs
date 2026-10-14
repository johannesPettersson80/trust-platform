use super::*;
use crate::analysis::declared::{
    DeclaredCoefficient, DeclaredController, DeclaredParameter, DeclaredPlant, DeclaredSweep,
    PlantKind,
};
use crate::analysis::routh_interval::calculate_declared_routh;
use crate::model::{ModelSource, ModelValidation};
use std::collections::BTreeMap;

/// The definition of control-studio-routh-session.test.ts: 1/(s² + 2s) with the controller gain Kp.
fn definition(revision: &str) -> DeclaredControlAnalysis {
    let mut parameters = BTreeMap::new();
    parameters.insert(
        "Kp".to_string(),
        DeclaredParameter {
            value: 1.0,
            minimum: 0.0,
            maximum: 10.0,
            unit: "%/°C".to_string(),
        },
    );
    DeclaredControlAnalysis {
        plant: DeclaredPlant {
            kind: PlantKind::TransferFunction,
            numerator: vec![1.0],
            denominator: vec![1.0, 2.0, 0.0],
            delay_seconds: 0.0,
            input_unit: "%".to_string(),
            output_unit: "°C".to_string(),
            revision: revision.to_string(),
            source: ModelSource::Provided,
            validation: ModelValidation::Validated,
        },
        controller: DeclaredController {
            numerator: vec![DeclaredCoefficient::Parameter(
                crate::analysis::declared::ParameterCoefficient {
                    parameter: "Kp".to_string(),
                    scale: None,
                },
            )],
            denominator: vec![DeclaredCoefficient::Constant(1.0)],
            parameters,
        },
        root_locus: DeclaredSweep {
            parameter: "Kp".to_string(),
            values: vec![0.0, 1.0, 2.0],
            selected_value: 1.0,
        },
    }
}

fn mapping() -> BTreeMap<String, String> {
    BTreeMap::from([("Kp".to_string(), "P1.Kp".to_string())])
}

#[test]
fn a_saved_result_with_a_mismatched_selection_is_refused() {
    let definition = definition("routh-session");
    let mut saved = SavedRouthAnalysis {
        selected_value: 2.0,
        result: calculate_declared_routh(&definition, Some(2.0)).expect("reading"),
        definition: definition.clone(),
    };
    assert!(check_saved_routh(&saved).is_ok());
    saved.result.selected_value = 3.0;
    assert!(
        check_saved_routh(&saved).is_err(),
        "the outer selected value must match the result"
    );
}

#[test]
fn a_union_result_needs_its_regions_and_boundaries() {
    let mut union_definition = definition("routh-union");
    union_definition.plant.denominator = vec![1.0, 3.0, 3.0, 10.0];
    union_definition.controller.numerator = vec![
        DeclaredCoefficient::Parameter(crate::analysis::declared::ParameterCoefficient {
            parameter: "Kp".to_string(),
            scale: None,
        }),
        DeclaredCoefficient::Parameter(crate::analysis::declared::ParameterCoefficient {
            parameter: "Kp".to_string(),
            scale: None,
        }),
        DeclaredCoefficient::Parameter(crate::analysis::declared::ParameterCoefficient {
            parameter: "Kp".to_string(),
            scale: Some(4.0),
        }),
    ];
    union_definition
        .controller
        .parameters
        .get_mut("Kp")
        .expect("Kp")
        .minimum = -20.0;
    union_definition
        .controller
        .parameters
        .get_mut("Kp")
        .expect("Kp")
        .maximum = 20.0;
    union_definition
        .controller
        .parameters
        .get_mut("Kp")
        .expect("Kp")
        .value = 2.0;
    union_definition.root_locus.values = vec![2.0];
    union_definition.root_locus.selected_value = 2.0;
    let saved = SavedRouthAnalysis {
        selected_value: 2.0,
        result: calculate_declared_routh(&union_definition, Some(2.0)).expect("reading"),
        definition: union_definition.clone(),
    };
    assert!(check_saved_routh(&saved).is_ok());
    let mut malformed = saved;
    malformed.result.interval.intervals = None;
    assert!(
        check_saved_routh(&malformed).is_err(),
        "a union without its regions is refused"
    );
}

#[test]
fn a_snapshot_is_read_only_and_restores_only_for_its_own_definition() {
    let definition = definition("routh-session");
    let mut session = RouthSession::new();
    session.run(&definition, 2.0).expect("the reading");
    let saved = session.snapshot().expect("a saved result");
    assert_eq!(
        session.selected_values(&mapping(), &definition),
        Some(BTreeMap::from([("P1.Kp".to_string(), 2.0)]))
    );
    session.invalidate();
    assert!(session.selected_values(&mapping(), &definition).is_none());
    assert!(session.restore(&saved, &definition));
    assert!(
        session.selected_values(&mapping(), &definition).is_none(),
        "restoring evidence grants no runtime selection"
    );
    let other = definition_with_revision("other");
    let mut other_session = RouthSession::new();
    assert!(
        !other_session.restore(&saved, &other),
        "a result from another definition is not restored"
    );
    assert!(other_session.snapshot().is_none());
}

fn definition_with_revision(revision: &str) -> DeclaredControlAnalysis {
    definition(revision)
}

#[test]
fn invalidate_clears_the_saved_result_and_the_selection() {
    let definition = definition("routh-session");
    let mut session = RouthSession::new();
    session.run(&definition, 1.0).expect("the reading");
    session.invalidate();
    assert!(session.snapshot().is_none());
    assert!(session.selected_values(&mapping(), &definition).is_none());
}

#[test]
fn a_failed_reading_keeps_the_previous_result_and_grants_no_selection() {
    let definition = definition("routh-session");
    let mut session = RouthSession::new();
    session.run(&definition, 1.0).expect("the reading");
    assert!(session.run(&definition, f64::NAN).is_err());
    assert!(session.selected_values(&mapping(), &definition).is_none());
    assert_eq!(session.snapshot().expect("kept").selected_value, 1.0);
}
