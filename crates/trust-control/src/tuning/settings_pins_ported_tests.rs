//! Ported assertions of `control-studio-manual-controller-candidate.test.ts` for the manual entry and its pins, and for a
//! saved value that is not finite. The fixture is the parameter binding of the `TypeScript` `pidLoop()`: `Main.Kp`,
//! `Main.Ki` and `Main.Kd`, each an `LREAL` from 0 to 100.

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

/// A manual entry with the given settings, as the engineer typed them.
fn entry(settings: &[(&str, f64)]) -> BTreeMap<String, f64> {
    settings
        .iter()
        .map(|&(symbol, value)| (symbol.to_string(), value))
        .collect()
}

/// The two steps of the `TypeScript` manual path (`tuneControllerAdapter` with manual values) for a PI entry on the fixture:
/// the entry is merged (the required settings and the pins), and then pinned (the bounds and the family).
fn manual_pi_path(settings: &[(&str, f64)]) -> Result<BTreeMap<GainKey, f64>, Refusal> {
    let parameters = pid_loop_parameters();
    let values = merge_manual_entry(
        Structure::PI,
        &entry(settings),
        &BTreeMap::new(),
        &parameters,
    )?;
    let pins = ControllerSettingsPins {
        loop_revision: LOOP_REVISION.to_string(),
        family: Structure::PI,
        values,
    };
    apply_pins(
        LOOP_REVISION,
        Structure::PI,
        &BTreeMap::new(),
        Some(&pins),
        &parameters,
    )
}

/// Whether a refusal names a setting, a gain, a bound, a parameter, a family, or an integral or derivative term: the words
/// that the `TypeScript` test accepts for a refused manual value.
fn names_the_refused_setting(sentence: &str) -> bool {
    let lowered = sentence.to_lowercase();
    [
        "setting",
        "gain",
        "bound",
        "parameter",
        "family",
        "integral",
        "derivative",
    ]
    .iter()
    .any(|&word| lowered.contains(word))
}

// control-studio-manual-controller-candidate: "rejects missing, incompatible and out-of-bounds manual values"
#[test]
fn a_manual_pi_entry_is_refused_when_a_setting_is_missing_incompatible_or_out_of_bounds() {
    let missing = manual_pi_path(&[("Main.Kp", 1.0)]).expect_err("Ki is required by PI");
    assert_eq!(missing.0, texts::REQUIRED_SETTING_MISSING);

    let too_large = manual_pi_path(&[("Main.Kp", 1.0), ("Main.Ki", 101.0)])
        .expect_err("Ki = 101 is above its maximum of 100");
    assert!(
        names_the_refused_setting(&too_large.0),
        "the refusal of Ki = 101 names the setting or its bounds: {}",
        too_large.0
    );

    let derivative_on_pi = manual_pi_path(&[("Main.Kp", 1.0), ("Main.Ki", 0.5), ("Main.Kd", 2.0)])
        .expect_err("PI has no derivative term");
    assert_eq!(derivative_on_pi.0, texts::PIN_FAMILY_CONFLICT);
}

// control-studio-manual-controller-candidate: "rejects missing, incompatible and out-of-bounds manual values"
#[test]
fn an_out_of_bounds_integral_gain_is_refused_by_naming_its_setting() {
    // Specification 35 A11.1 over the TypeScript sentence ("Pinned setting Main.Ki does not match the bound ST parameter and
    // representable bounds."): the failure names the setting, its value and the bounds it misses.
    let refusal = manual_pi_path(&[("Main.Kp", 1.0), ("Main.Ki", 101.0)])
        .expect_err("Ki = 101 is above its maximum of 100");
    assert_eq!(
        refusal.0,
        "Pinned setting Main.Ki = 101 is outside its bounds: its magnitude must lie between 0 and 100. Change or clear the pin."
    );
}

// control-studio-manual-controller-candidate: "retains the selected tuning method and manual values on reopening without restoring application eligibility"
#[test]
fn a_saved_manual_value_that_is_not_finite_is_no_pin() {
    // JSON has no Infinity: `JSON.stringify` writes it as `null`, and the TypeScript refuses it when the study is read. This
    // tests the pin record only. The saved method, the reopened view and the stage evidence (spec 35 A8.6, "Reopening") need
    // the study's Tune section and are not asserted here: that part of the TypeScript test is FLOW.
    let finite =
        serde_json::json!({"loopRevision": "r1", "family": "PI", "values": {"Main.Kp": 1.5}});
    assert!(
        decode_pins(&finite).is_some(),
        "a finite saved value is read"
    );
    let not_finite =
        serde_json::json!({"loopRevision": "r1", "family": "PI", "values": {"Main.Kp": null}});
    assert!(
        decode_pins(&not_finite).is_none(),
        "a value that is not finite is no pin"
    );
}
