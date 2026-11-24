//! A plant's feedback in a cycle trace (specification 48 RU-59; specification 46 §13.2): a plant
//! publishes before the next scan, so each of its writes is the next cycle's change from a plant
//! (source 7), in the order it wrote them.

use super::*;
use crate::harness::TestHarness;
use crate::runtime::{ended_trace, record_cycles};
use crate::simulation::SimulationConfig;
use trust_runtime_core::trace::{Change, ChangeKind, ChangeSource, IoArea};

const CONTROL_LIBRARY: &str = include_str!("../../../../../../libraries/control/src/control.st");

/// A static gain of 0.5 from `Main.Command` to each of `targets`, one plant each, in their order.
fn plants(targets: &[&str]) -> SimulationConfig {
    let mut config = "[simulation]\nenabled = true\nplant_step_ms = 100\n".to_string();
    for (index, target) in targets.iter().enumerate() {
        config.push_str(&format!(
            "\n[[plants]]\nid = \"plant-{index}\"\nmodel = \"static_gain\"\ncommand_sources = [\"Main.Command\"]\nfeedback_targets = [\"{target}\"]\n[plants.parameters]\ngain = 0.5\n"
        ));
    }
    SimulationConfig::from_toml_text(&config, "plant trace test").expect("the plants are valid")
}

/// A plant's write of the variable `path`, of type `type_name`, as the bytes `value`.
fn plant_wrote(path: &str, type_name: &str, value: &[u8]) -> Change {
    Change {
        source: ChangeSource::Plant,
        kind: ChangeKind::VariableWrite {
            path: path.into(),
            type_name: type_name.into(),
            value: value.to_vec(),
        },
    }
}

/// Specification 48 RU-59: a plant's feedback at an address and at a program variable, published
/// between the two cycles of a recording, is the second cycle's changes from a plant, in the order
/// the plants wrote them. Worked by hand: the static gain 0.5 of `Main.Command`, 38.0, is 19.0; the
/// first plant writes it at `%IW4`, an INT there, the bytes 13 00 at offset 4 of `%I`; the second
/// writes `Main.Measurement`, an LREAL, 0x4033000000000000, little-endian
/// 00 00 00 00 00 00 33 40. Catches a plant's write not recorded, an address write between two
/// cycles dropped, and a write recorded as another source's or in the cycle before.
#[test]
fn a_plants_feedback_is_the_next_cycles_changes_from_a_plant() {
    let program = "PROGRAM Main VAR Command : LREAL := 38.0; Measurement : LREAL; \
                   Level AT %IW4 : INT; END_VAR END_PROGRAM";
    let config = plants(&["%IW4", "Main.Measurement"]);
    let mut harness = TestHarness::from_source(program).unwrap();
    let mut system = PlantSystem::new(&config.plant_system);
    harness.cycle();
    let traces = record_cycles(harness.runtime_mut(), 2);
    harness.cycle();
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    harness.cycle();

    let trace = ended_trace(&traces);
    assert_eq!(trace.cycles[0].changes, vec![]);
    assert_eq!(
        trace.cycles[1].changes,
        vec![
            Change {
                source: ChangeSource::Plant,
                kind: ChangeKind::AddressWrite {
                    area: IoArea::Input,
                    bit: None,
                    offset: 4,
                    bytes: vec![19, 0],
                },
            },
            plant_wrote("Main.Measurement", "LREAL", &[0, 0, 0, 0, 0, 0, 0x33, 0x40]),
        ]
    );
}

/// Specification 48 RU-59 and specification 46 §13.2: a plant gives a Control.PID instance its
/// measurement by writing the instance's held measurement and its flag, and the first publish of a
/// program generation releases the flag first. Worked by hand: published between the two cycles of
/// a recording, the release writes `MeasurementHeld` FALSE (00), then the hold writes
/// `HeldMeasurement`, 0.5 × 84.0 = 42.0, an LREAL, 0x4045000000000000, little-endian
/// 00 00 00 00 00 00 45 40, and `MeasurementHeld` TRUE (01): the second cycle's three changes from
/// a plant. Catches a held measurement or its flag not recorded, and a release not recorded.
#[test]
fn a_plants_measurement_for_a_controller_is_the_next_cycles_changes_from_a_plant() {
    let program = r#"
PROGRAM Main
VAR
    Command : LREAL := 84.0;
    Controller : Control.PID;
    Settings : Control.PIDParameters := (ProportionalGain := 1.0, OutputMaximum := 100.0,
        RateMinimum := -1.0E9, RateMaximum := 1.0E9, TrackingTime := 1.0);
    Input : Control.PIDInput;
    Output : LREAL;
END_VAR
    Controller.Configure(Parameters := Settings);
    Input.Setpoint := 50.0;
    Input.ProcessValue := 1.0;
    Input.CycleTime := 0.1;
    Output := Controller.Update(Input := Input);
END_PROGRAM
"#;
    let config = plants(&["pid:Main.Controller"]);
    let mut harness = TestHarness::from_sources(&[CONTROL_LIBRARY, program]).unwrap();
    let mut system = PlantSystem::new(&config.plant_system);
    harness.cycle();
    let traces = record_cycles(harness.runtime_mut(), 2);
    harness.cycle();
    system
        .advance(Duration::from_millis(100), harness.runtime())
        .unwrap();
    system.publish(harness.runtime_mut()).unwrap();
    harness.cycle();

    let trace = ended_trace(&traces);
    assert_eq!(trace.cycles[0].changes, vec![]);
    assert_eq!(
        trace.cycles[1].changes,
        vec![
            plant_wrote("Main.Controller.MeasurementHeld", "BOOL", &[0]),
            plant_wrote(
                "Main.Controller.HeldMeasurement",
                "LREAL",
                &[0, 0, 0, 0, 0, 0, 0x45, 0x40]
            ),
            plant_wrote("Main.Controller.MeasurementHeld", "BOOL", &[1]),
        ]
    );
}
