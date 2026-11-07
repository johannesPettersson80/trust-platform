//! A program's own law in standard form (specification 35 C2, numerics annex N6.8): Tune designs it as the parallel law and writes
//! `Kp`, `Ti = Kp/Ki` and `Td = Kd/Kp`; a pinned or entered time is a time in seconds; and every reader of the law's variables turns
//! the times back into gains.

#![allow(clippy::float_cmp)] // Gains that pass through unchanged are compared exactly.

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::fixture_loops::{project_pi_loop_on_p1, project_standard_pi_loop_on_p1};
use crate::commissioning::project_law::gain_symbols;
use crate::loop_definition::LoopDefinition;
use crate::messages::TuneRequest;
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::manual::manual_candidate;
use crate::study::tune::tune_held;
use crate::tuning::candidate::LawCandidate;
use crate::tuning::objective::TuningObjective;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::project_seed::{seed_values, GainForm, ParallelPidSeed};
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::fixtures::{requirements, smith_heater_model};

/// The loop with the heater's measurement unit, `degC`, the unit of the model's output.
fn heater(mut loop_definition: LoopDefinition) -> LoopDefinition {
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition
}

/// The heater `0.2/(5s + 1)` with a one-second delay, supplied rather than fitted, so it is current for every loop.
fn current() -> CurrentModel {
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model: smith_heater_model(),
        identify: None,
    }
}

fn request() -> TuneRequest {
    TuneRequest {
        project_root: "/projects/heater".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "loop:tank".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method: None,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

/// The candidate Tune designs for `loop_definition` on the heater, with `pins`: a 0.5 °C change settled within 10 s in a 30 s test.
fn designed(
    loop_definition: &LoopDefinition,
    pins: Option<&ControllerSettingsPins>,
) -> ProjectCandidate {
    let model = current();
    let request = request();
    let inputs = TuneInputs {
        loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins,
    };
    let mut task = requirements(0.5, 0.1, 0.05, 10.0, 30.0);
    task.tuning_objective = Some(TuningObjective::Balanced);
    tune_held(&inputs, Some(&task), None)
        .expect("the heater's PI is designed")
        .section
        .project_candidate
        .expect("a program's own law gives a project candidate")
}

/// N6.8: the standard law is designed as the parallel one, and holds the same law as `Kp` and `Ti = Kp/Ki`. Catches the integral gain
/// written into the reset time variable (the program would integrate with `Kp/Ki` where it needs `Ki`), and a design that differs
/// between the two forms.
#[test]
fn a_standard_law_is_designed_as_the_parallel_law_and_holds_kp_and_ti() {
    let parallel = designed(&heater(project_pi_loop_on_p1()), None);
    let standard = designed(&heater(project_standard_pi_loop_on_p1()), None);
    assert_eq!(standard.gains, parallel.gains, "one law in two forms");
    let ParallelGains {
        proportional_gain,
        integral_gain,
        ..
    } = parallel.gains;
    assert!(
        proportional_gain > 0.0 && integral_gain > 0.0,
        "{:?}",
        parallel.gains
    );
    assert_eq!(standard.values["P1.Kp"], proportional_gain);
    assert_eq!(standard.values["P1.Ti"], proportional_gain / integral_gain);
    assert_eq!(parallel.values["P1.Ki"], integral_gain);
}

/// C2: a pinned reset time is a time in seconds, which the design keeps beside its gain: `Ki = Kp/Ti` with the designed `Kp`.
/// Catches the pinned 8 s taken as an integral gain of 8, and the designed gain changed by the pin.
#[test]
fn a_pinned_reset_time_keeps_the_designed_gain() {
    let loop_definition = heater(project_standard_pi_loop_on_p1());
    let free = designed(&loop_definition, None);
    let pins = ControllerSettingsPins {
        loop_revision: loop_definition.revision.clone(),
        family: Structure::PI,
        values: BTreeMap::from([("P1.Ti".to_owned(), 8.0)]),
    };
    let pinned = designed(&loop_definition, Some(&pins));
    let gain = free.gains.proportional_gain;
    assert_eq!(pinned.gains.proportional_gain, gain);
    assert_eq!(pinned.gains.integral_gain, gain / 8.0);
    assert_eq!(pinned.values["P1.Ti"], 8.0);
}

/// C2: an entered setting of a standard law is `Kp` and the time `Ti` in seconds. Worked by hand: `Kp = 2`, `Ti = 4 s` give
/// `Ki = 0.5`. Catches the entered 4 s taken as an integral gain.
#[test]
fn an_entered_reset_time_gives_the_integral_gain() {
    let loop_definition = heater(project_standard_pi_loop_on_p1());
    let entered = BTreeMap::from([("P1.Kp".to_owned(), 2.0), ("P1.Ti".to_owned(), 4.0)]);
    let candidate = manual_candidate(&loop_definition, Structure::PI, &entered, None)
        .expect("a complete PI entry is a candidate");
    let LawCandidate::Project(candidate) = candidate else {
        panic!("a program's own law gives a project candidate");
    };
    assert_eq!(candidate.gains.proportional_gain, 2.0);
    assert_eq!(candidate.gains.integral_gain, 0.5);
    assert_eq!(candidate.values["P1.Ti"], 4.0);
}

/// N6.8: the transfers and allowances read the law's variables as gains, so a standard law's reset time gives `Ki = Kp/Ti`. Worked
/// by hand: `Kp = 2`, `Ti = 4 s` give `Ki = 0.5`. Catches the reset time read as the integral gain (an entry allowance eight times
/// too wide).
#[test]
fn the_readers_of_a_standard_law_take_its_reset_time_as_kp_over_ti() {
    let loop_definition = project_standard_pi_loop_on_p1();
    let symbols = gain_symbols(&loop_definition).expect("a program's own law names its variables");
    assert_eq!(symbols.form, GainForm::Standard);
    let values = BTreeMap::from([("P1.Kp".to_owned(), 2.0), ("P1.Ti".to_owned(), 4.0)]);
    let value = |symbol: &str| values.get(symbol).copied();
    assert_eq!(symbols.integral_gain(value), Some(0.5));
    assert_eq!(
        symbols.derivative_gain(value),
        None,
        "a PI has no derivative term"
    );
}

/// N6.8 for a reverse-acting loop: the gains are negative (N6.4), and the times stay above zero. Worked by hand: `Kp = −2`,
/// `Ki = −0.5`, `Kd = −1` give `Ti = 4 s` and `Td = 0.5 s`. Catches a sign carried into the times.
#[test]
fn a_reverse_law_writes_positive_times() {
    let seed: ParallelPidSeed = crate::json::decode_value(serde_json::json!({
        "family": "PID", "form": "standard", "proportionalSymbol": "Main.Kp",
        "resetTimeSymbol": "Main.Ti", "derivativeTimeSymbol": "Main.Td"
    }))
    .expect("a standard seed");
    let values = seed_values(
        &seed,
        ParallelGains {
            proportional_gain: -2.0,
            integral_gain: -0.5,
            derivative_gain: -1.0,
        },
    );
    assert_eq!(
        values,
        BTreeMap::from([
            ("Main.Kp".to_owned(), -2.0),
            ("Main.Td".to_owned(), 0.5),
            ("Main.Ti".to_owned(), 4.0)
        ])
    );
}
