//! Tests of the live sample of a loop (specification 35, A5, A6.4 and B3.4 step 1; runtime protocol annex E3), on the loop of
//! the `control-pid-commissioning` example: a setpoint, a measurement and a manipulated variable in °C and %, and a mode
//! signal `P1.AutoMode` that is TRUE in Auto.

use serde_json::json;

use super::*;
use crate::loop_definition::{AuxiliaryRole, AuxiliarySignal, Direction, Range};
use crate::signals::{loop_inspection_bindings, Inspection, InspectionRequest};

/// One binding's reading in an inspection: its value, quality and forced flag.
#[derive(Clone, Copy)]
struct Reading {
    value: Option<f64>,
    quality: Quality,
    forced: bool,
}

/// A good, unforced reading of `value`.
fn good(value: f64) -> Reading {
    Reading {
        value: Some(value),
        quality: Quality::Good,
        forced: false,
    }
}

/// The loop of the example project.
fn example_loop() -> LoopDefinition {
    let text = include_str!("../../../../examples/control-pid-commissioning/control-loop.json");
    LoopDefinition::decode(text).expect("the example loop decodes")
}

/// The steady loop in Auto: the setpoint at 45 °C, the measurement at 44.5 °C, the output at 30 % and every other binding,
/// the mode signal among them, TRUE.
fn steady(id: &str) -> Reading {
    match id {
        "setpoint" => good(45.0),
        "processVariable" => good(44.5),
        "manipulatedVariable" => good(30.0),
        _ => good(1.0),
    }
}

/// The inspection of the loop's watch at scan 42, with `reading` giving each binding's reading by its identifier.
fn inspection(
    loop_definition: &LoopDefinition,
    reading: &dyn Fn(&str) -> Reading,
) -> InspectionSample {
    let bindings = loop_inspection_bindings(loop_definition);
    let readings: Vec<Reading> = bindings
        .iter()
        .map(|binding| reading(&binding.id))
        .collect();
    InspectionSample {
        program_revision: "sha256:program".to_owned(),
        loop_revision: loop_definition.revision.clone(),
        bindings,
        scan: 42,
        runtime_time_ns: 4_200_000_000,
        captured_at_ms: 1_700_000_000_000,
        values: readings.iter().map(|each| each.value).collect(),
        forced: readings.iter().map(|each| each.forced).collect(),
        quality: readings.iter().map(|each| each.quality).collect(),
    }
}

/// A signal of the sample as the loop declares it, with its value and quality.
fn plan_signal(
    symbol: &str,
    address: &str,
    value: f64,
    unit: &str,
    quality: Quality,
) -> PlanSignal {
    PlanSignal {
        symbol: symbol.to_owned(),
        address: Some(address.to_owned()),
        value,
        unit: unit.to_owned(),
        quality,
    }
}

/// The mode of the loop's sample when every binding is steady but the mode signal reads `mode`.
fn mode_of(loop_definition: &LoopDefinition, mode: Reading) -> Option<ControllerMode> {
    let reading = |id: &str| {
        if id == "controllerMode" {
            mode
        } else {
            steady(id)
        }
    };
    experiment_sample(
        loop_definition,
        EvidenceSource::SimulatedPlant,
        &inspection(loop_definition, &reading),
    )
    .map(|sample| sample.controller_mode)
}

#[test]
fn a_usable_inspection_gives_the_sample_of_its_scan_with_the_signals_the_loop_declares() {
    let loop_definition = example_loop();
    let sample = experiment_sample(
        &loop_definition,
        EvidenceSource::PhysicalPlant,
        &inspection(&loop_definition, &steady),
    );
    assert_eq!(
        sample,
        Some(ExperimentSample {
            scan: 42,
            source: EvidenceSource::PhysicalPlant,
            controller_mode: ControllerMode::Auto,
            setpoint: Some(plan_signal(
                "P1.Setpoint",
                "%ML0",
                45.0,
                "°C",
                Quality::Good
            )),
            process_variable: plan_signal("P1.ProcessValue", "%IL0", 44.5, "°C", Quality::Good),
            manipulated_variable: plan_signal(
                "P1.ManipulatedValue",
                "%QL0",
                30.0,
                "%",
                Quality::Good
            ),
            additional: Vec::new(),
        })
    );
}

// Annex E3: a pending answer holds no scan, so it gives no sample; the available answer of the same watch converts.
#[test]
fn a_pending_inspection_has_no_sample_and_an_available_one_of_the_same_watch_converts() {
    let loop_definition = example_loop();
    let request = InspectionRequest::new(
        &loop_definition.revision,
        None,
        loop_inspection_bindings(&loop_definition),
    )
    .expect("the watch of the loop is a valid request");
    let bindings = serde_json::to_value(&request.bindings).expect("the bindings serialize");
    let pending = request
        .decode(&json!({
            "program_revision": "sha256:program", "loop_revision": loop_definition.revision,
            "bindings": bindings, "available": false
        }))
        .expect("the pending answer decodes");
    assert!(matches!(pending, Inspection::Pending { .. }), "{pending:?}");
    let available = request
        .decode(&json!({
            "program_revision": "sha256:program", "loop_revision": loop_definition.revision,
            "bindings": bindings, "available": true, "scan": 42, "runtime_time_ns": 4_200_000_000_u64,
            "captured_at_ms": 1_700_000_000_000_u64, "values": [45.0, 44.5, 30.0, 0],
            "forced": [false, false, false, false], "quality": ["good", "good", "good", "good"]
        }))
        .expect("the available answer decodes");
    let Inspection::Available(scan) = available else {
        panic!("the answer with a scan is available");
    };
    let sample = experiment_sample(&loop_definition, EvidenceSource::SimulatedPlant, &scan)
        .expect("the scan reads every signal");
    assert_eq!(sample.scan, 42);
    assert_eq!(
        sample.controller_mode,
        ControllerMode::Manual,
        "the mode signal is FALSE"
    );
}

#[test]
fn a_measurement_or_output_that_cannot_be_read_leaves_no_sample() {
    let loop_definition = example_loop();
    let unreadable = [
        Reading {
            value: None,
            quality: Quality::Bad,
            forced: false,
        },
        Reading {
            value: None,
            quality: Quality::Missing,
            forced: false,
        },
        Reading {
            value: Some(44.5),
            quality: Quality::Bad,
            forced: false,
        },
    ];
    for id in ["processVariable", "manipulatedVariable", "setpoint"] {
        for broken in unreadable {
            let reading = |binding: &str| {
                if binding == id {
                    broken
                } else {
                    steady(binding)
                }
            };
            assert_eq!(
                experiment_sample(
                    &loop_definition,
                    EvidenceSource::SimulatedPlant,
                    &inspection(&loop_definition, &reading),
                ),
                None,
                "{id} with {:?}",
                broken.quality
            );
        }
    }
    // A binding that is not in the inspection cannot be read either.
    let mut without_measurement = inspection(&loop_definition, &steady);
    if let Some(binding) = without_measurement
        .bindings
        .iter_mut()
        .find(|binding| binding.id == "processVariable")
    {
        binding.id = "elsewhere".into();
    }
    assert_eq!(
        experiment_sample(
            &loop_definition,
            EvidenceSource::SimulatedPlant,
            &without_measurement
        ),
        None
    );
}

// Specification 35, B3.9: a forced signal is named and refused by the review, so the sample keeps its force.
#[test]
fn a_forced_value_is_read_with_its_force() {
    let loop_definition = example_loop();
    let forced_flag = |id: &str| {
        if id == "processVariable" {
            Reading {
                value: Some(44.5),
                quality: Quality::Good,
                forced: true,
            }
        } else {
            steady(id)
        }
    };
    let forced_quality = |id: &str| {
        if id == "manipulatedVariable" {
            Reading {
                value: Some(30.0),
                quality: Quality::Forced,
                forced: false,
            }
        } else {
            steady(id)
        }
    };
    let flagged = experiment_sample(
        &loop_definition,
        EvidenceSource::SimulatedPlant,
        &inspection(&loop_definition, &forced_flag),
    )
    .expect("a forced value is read");
    assert_eq!(flagged.process_variable.quality, Quality::Forced);
    assert_eq!(flagged.manipulated_variable.quality, Quality::Good);
    let qualified = experiment_sample(
        &loop_definition,
        EvidenceSource::SimulatedPlant,
        &inspection(&loop_definition, &forced_quality),
    )
    .expect("a forced value is read");
    assert_eq!(qualified.manipulated_variable.quality, Quality::Forced);
}

// Specification 35, A6.4: the mode comes from the mode signal's usable value, as the loop's mode-when-true says.
#[test]
fn the_mode_is_read_from_the_mode_signal_or_is_unavailable() {
    let loop_definition = example_loop();
    assert_eq!(
        mode_of(&loop_definition, good(1.0)),
        Some(ControllerMode::Auto)
    );
    assert_eq!(
        mode_of(&loop_definition, good(0.0)),
        Some(ControllerMode::Manual)
    );
    let mut true_is_manual = example_loop();
    true_is_manual.controller.mode_when_true = Some(ModeWhenTrue::Manual);
    assert_eq!(
        mode_of(&true_is_manual, good(1.0)),
        Some(ControllerMode::Manual)
    );
    assert_eq!(
        mode_of(&true_is_manual, good(0.0)),
        Some(ControllerMode::Auto)
    );
    for unusable in [
        Reading {
            value: Some(1.0),
            quality: Quality::Good,
            forced: true,
        },
        Reading {
            value: None,
            quality: Quality::Missing,
            forced: false,
        },
        good(0.5),
    ] {
        assert_eq!(
            mode_of(&loop_definition, unusable),
            Some(ControllerMode::Unavailable),
            "a mode that is forced, missing or not a BOOL is not read"
        );
    }
    let mut without_mode = example_loop();
    without_mode.controller.mode_signal = None;
    assert_eq!(
        mode_of(&without_mode, good(1.0)),
        Some(ControllerMode::Unavailable),
        "a loop without a mode signal has no readable mode"
    );
}

#[test]
fn the_further_signals_of_the_loop_are_part_of_the_sample() {
    let mut loop_definition = example_loop();
    loop_definition.signals.additional = Some(vec![AuxiliarySignal {
        id: "inlet-temperature".to_owned(),
        channel: "primary".to_owned(),
        role: AuxiliaryRole::Disturbance,
        symbol: "P1.InletTemperature".to_owned(),
        address: Some("%IL8".to_owned()),
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "°C".to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    }]);
    let with_inlet = |id: &str| {
        if id == "inlet-temperature" {
            good(12.5)
        } else {
            steady(id)
        }
    };
    let sample = experiment_sample(
        &loop_definition,
        EvidenceSource::SimulatedPlant,
        &inspection(&loop_definition, &with_inlet),
    )
    .expect("every signal is read");
    assert_eq!(
        sample.additional,
        [PlanAuxiliarySignal {
            id: "inlet-temperature".to_owned(),
            signal: plan_signal("P1.InletTemperature", "%IL8", 12.5, "°C", Quality::Good),
        }]
    );
    let lost_inlet = |id: &str| {
        if id == "inlet-temperature" {
            Reading {
                value: None,
                quality: Quality::Bad,
                forced: false,
            }
        } else {
            steady(id)
        }
    };
    assert_eq!(
        experiment_sample(
            &loop_definition,
            EvidenceSource::SimulatedPlant,
            &inspection(&loop_definition, &lost_inlet),
        ),
        None,
        "a further signal that cannot be read leaves no sample"
    );
}
