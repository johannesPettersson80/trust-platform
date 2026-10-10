//! Ported assertions of `control-studio-manual-controller-candidate.test.ts` for the manual candidate: the engineer's complete
//! PI values are kept as entered, and the candidate they make is unapplied. The fixture is the parameter binding of the
//! `TypeScript` `pidLoop()`: `Main.Kp`, `Main.Ki` and `Main.Kd`, each an `LREAL` from 0 to 100.

use std::collections::BTreeMap;

use crate::tuning::proportional::GainBounds;
use crate::tuning::settings_pins::{
    apply_pins, merge_manual_entry, ControllerSettingsPins, GainKey, MappedParameter,
};

use super::*;

/// The loop revision of the `TypeScript` fixture (`control-loop.json`).
const LOOP_REVISION: &str = "sha256:control-studio-engineer-v3";

/// The parameter binding of the `TypeScript` `pidLoop()` fixture: `Main.Kp`, `Main.Ki` and `Main.Kd`, each from 0 to 100.
fn pid_loop_parameters() -> Vec<MappedParameter> {
    [
        ("Main.Kp", GainKey::ProportionalGain),
        ("Main.Ki", GainKey::IntegralGain),
        ("Main.Kd", GainKey::DerivativeGain),
    ]
    .into_iter()
    .map(|(symbol, key)| MappedParameter {
        symbol: symbol.to_string(),
        key,
        bounds: GainBounds {
            minimum: 0.0,
            maximum: 100.0,
        },
        real: false,
    })
    .collect()
}

/// Whether two values are the same number bit for bit. The `TypeScript` test compares with `deepStrictEqual`, so a tolerance
/// would weaken it.
fn same_number(actual: f64, expected: f64) -> bool {
    actual.to_bits() == expected.to_bits()
}

// control-studio-manual-controller-candidate: "uses the engineer's complete PI values without running an automatic tuning rule"
#[test]
fn the_engineers_complete_pi_values_make_an_unapplied_candidate_as_entered() {
    let parameters = pid_loop_parameters();
    let entered = BTreeMap::from([
        ("Main.Kp".to_string(), 1.234_567_890_123_45),
        ("Main.Ki".to_string(), 0.789_012_345_678_9),
    ]);
    let values = merge_manual_entry(Structure::PI, &entered, &BTreeMap::new(), &parameters)
        .expect("a PI entry with Kp and Ki is complete");
    let pins = ControllerSettingsPins {
        loop_revision: LOOP_REVISION.to_string(),
        family: Structure::PI,
        values,
    };
    let no_gains = BTreeMap::from([
        (GainKey::ProportionalGain, 0.0),
        (GainKey::IntegralGain, 0.0),
        (GainKey::DerivativeGain, 0.0),
    ]);
    let kept = apply_pins(
        LOOP_REVISION,
        Structure::PI,
        &no_gains,
        Some(&pins),
        &parameters,
    )
    .expect("the entered values are within their bounds");
    assert_eq!(kept.len(), 3, "the values are exactly Kp, Ki and Kd");
    let proportional = kept[&GainKey::ProportionalGain];
    let integral = kept[&GainKey::IntegralGain];
    let derivative = kept[&GainKey::DerivativeGain];
    assert!(
        same_number(proportional, 1.234_567_890_123_45),
        "Kp is kept as entered, got {proportional}"
    );
    assert!(
        same_number(integral, 0.789_012_345_678_9),
        "Ki is kept as entered, got {integral}"
    );
    assert!(
        same_number(derivative, 0.0),
        "Kd is zero for PI, got {derivative}"
    );

    let record = ControlPidParameters {
        proportional_gain: proportional,
        integral_gain: integral,
        derivative_gain: derivative,
        // The other fields are not part of the TypeScript expectation, and no assertion below reads them. The setpoint weight
        // and the tracking time are stand-ins: the TypeScript test does not state them, and spec 35 C0 requires both for a
        // manual PI. The bounds are the calculated defaults.
        proportional_setpoint_weight: 1.0,
        derivative_setpoint_weight: 0.0,
        bias: 0.0,
        output_minimum: DEFAULT_OUTPUT_MINIMUM,
        output_maximum: DEFAULT_OUTPUT_MAXIMUM,
        rate_minimum: -DEFAULT_RATE_LIMIT_PER_S,
        rate_maximum: DEFAULT_RATE_LIMIT_PER_S,
        tracking_time: 1.0,
        anti_windup: AntiWindup::BackCalculation,
    };
    let candidate = candidate_from_control_pid(record, CandidateSource::Manual)
        .expect("a complete PI record is a valid candidate");
    // Spec 35 B4.1: Tune never writes to the running controller, so a candidate is never applied.
    assert!(
        !candidate.applied,
        "a manual candidate is never applied (B4.1)"
    );
    assert_eq!(candidate.structure, Structure::PI);
}
