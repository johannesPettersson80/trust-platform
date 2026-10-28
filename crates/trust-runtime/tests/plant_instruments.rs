//! Plant library specification §7 and §14.11: analog scaling, quality and the held value, one
//! signal alarm with the limit alarms suppressed while bad, limit alarms with a deadband, the
//! optional filter; digital debounce and inversion; the totaliser and the counter; the weigh
//! scale's tare; the tank showing its level instrument by composition.

mod plant_support;

use plant_support::{assert_close, Rig};

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    Signal : LREAL := 12.0;
    SignalBroken : BOOL;
    Direct : LREAL := 50.0;
    SwitchContact : BOOL;
    FlowRate : LREAL;
    FlowRateGood : BOOL := TRUE;
    PulseIn : BOOL;
    WeightSignal : LREAL := 12.0;
    LevelSignal : LREAL := 12.0;
    CmdTare : BOOL;
    TareAccepted : BOOL;
    PT101 : Plant.FB_PressureInput := (Unit := 'bar', RangeHigh := 16.0);
    (* A channel already in percent, so the limit values below are exact. *)
    LT102 : Plant.FB_LevelInput := (RawLow := 0.0, RawHigh := 100.0, BadBelow := -10.0,
        BadAbove := 110.0, HighEnabled := TRUE, HighLimit := 90.0, Deadband := 2.0,
        LowEnabled := TRUE, LowLimit := 10.0);
    TT103 : Plant.FB_TemperatureInput := (FilterTime := T#1s);
    LS104 : Plant.FB_LevelSwitch := (Debounce := T#500ms);
    PS105 : Plant.FB_PressureSwitch := (Invert := TRUE);
    FQ106 : Plant.FB_Totalizer;
    CT107 : Plant.FB_Counter;
    WT108 : Plant.FB_WeighScale := (Unit := 'kg', RangeHigh := 500.0, StableTime := T#1s);
    LT109 : Plant.FB_LevelInput := (Unit := 'm', RangeHigh := 4.0);
    T110 : Plant.FB_Tank := (Height := 4.0, Capacity := 12.0);
END_VAR
    IF CmdTare THEN
        TareAccepted := WT108.Tare();
        CmdTare := FALSE;
    END_IF;
    PT101(Raw := Signal, SignalFault := SignalBroken);
    LT102(Raw := Direct);
    TT103(Raw := Signal);
    LS104(Input := SwitchContact);
    PS105(Input := SwitchContact);
    FQ106(Rate := FlowRate, RateGood := FlowRateGood);
    CT107(Pulse := PulseIn);
    WT108(Raw := WeightSignal);
    LT109(Raw := LevelSignal);
    T110(LevelInstrument := LT109);
END_PROGRAM
"#;

const ANALOG_ALARMS: [&str; 5] = [
    "SignalAlarm",
    "HighHighAlarm",
    "HighAlarm",
    "LowAlarm",
    "LowLowAlarm",
];

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn an_analog_input_scales_four_to_twenty_milliamps_to_its_range() {
    let mut rig = rig();
    assert_eq!(
        rig.real("PT101.Value"),
        8.0,
        "12 mA is mid-range of 0-16 bar"
    );
    assert_eq!(rig.real("PT101.Percent"), 50.0);
    assert_eq!(rig.state("PT101.Quality"), "Good");
    assert!(!rig.flag("PT101.Fault"));
    for (milliamps, bar) in [(4.0, 0.0), (20.0, 16.0), (8.0, 4.0)] {
        rig.set_real("Signal", milliamps);
        rig.scan();
        assert_eq!(rig.real("PT101.Value"), bar, "{milliamps} mA");
    }
}

#[test]
fn a_signal_below_bad_below_is_bad_and_holds_the_last_good_value() {
    let mut rig = rig();
    rig.set_real("Signal", 3.5);
    rig.scan();
    assert_eq!(rig.state("PT101.Quality"), "Bad", "3.5 mA is below 3.6 mA");
    assert!(rig.flag("PT101.Fault"));
    assert_eq!(
        rig.real("PT101.Value"),
        8.0,
        "the value holds its last good reading"
    );
    assert!(
        rig.flag("PT101.SignalAlarm.Active"),
        "the signal alarm is active"
    );
}

#[test]
fn a_signal_above_bad_above_or_a_module_fault_is_bad() {
    let mut rig = rig();
    rig.set_real("Signal", 21.5);
    rig.scan();
    assert_eq!(rig.state("PT101.Quality"), "Bad", "21.5 mA is above 21 mA");
    rig.set_real("Signal", 12.0);
    rig.set_bool("SignalBroken", true);
    rig.scan();
    assert_eq!(
        rig.state("PT101.Quality"),
        "Bad",
        "the I/O module's diagnosis makes it bad"
    );
    rig.set_bool("SignalBroken", false);
    rig.scan();
    assert_eq!(rig.state("PT101.Quality"), "Good");
    assert!(
        !rig.flag("PT101.SignalAlarm.Active"),
        "the signal alarm clears with the signal"
    );
}

#[test]
fn while_bad_one_signal_alarm_is_raised_and_the_limit_alarms_are_suppressed() {
    let mut rig = rig();
    rig.set_real("Direct", 95.0);
    rig.scan();
    assert!(rig.flag("LT102.High") && rig.flag("LT102.HighAlarm.Active"));
    rig.set_real("Direct", 120.0);
    rig.scan();
    assert_eq!(rig.state("LT102.Quality"), "Bad");
    assert_eq!(
        rig.real("LT102.Value"),
        95.0,
        "the value holds its last good reading"
    );
    assert!(rig.flag("LT102.SignalAlarm.Active"));
    assert!(
        rig.flag("LT102.HighAlarm.SuppressedByDesign"),
        "the limit alarms are suppressed while the signal is bad"
    );
    assert!(
        !rig.flag("LT102.High"),
        "the limit outputs are off while bad"
    );
    let annunciating: Vec<&str> = ANALOG_ALARMS
        .into_iter()
        .filter(|alarm| rig.flag(&format!("LT102.{alarm}.Annunciating")))
        .collect();
    assert_eq!(
        annunciating,
        ["SignalAlarm"],
        "a broken wire raises one alarm, not several"
    );
}

#[test]
fn a_limit_alarm_clears_only_beyond_its_deadband() {
    let mut rig = rig();
    for (value, high) in [(91.0, true), (89.0, true), (87.5, false)] {
        rig.set_real("Direct", value);
        rig.scan();
        assert_eq!(
            rig.flag("LT102.High"),
            high,
            "high limit 90 with a deadband of 2, at {value}"
        );
    }
    for (value, low) in [(9.0, true), (11.0, true), (12.5, false)] {
        rig.set_real("Direct", value);
        rig.scan();
        assert_eq!(
            rig.flag("LT102.Low"),
            low,
            "low limit 10 with a deadband of 2, at {value}"
        );
    }
}

#[test]
fn a_filter_time_smooths_a_step() {
    let mut rig = rig();
    assert_eq!(
        rig.real("TT103.Value"),
        50.0,
        "the first reading is taken as it is"
    );
    rig.set_real("Signal", 20.0);
    rig.scan();
    let filtered = rig.real("TT103.Value");
    assert!(
        filtered > 50.0 && filtered < 100.0,
        "one scan after a step from 50 to 100 the filtered value lies between: {filtered}"
    );
    assert_eq!(
        rig.real("PT101.Value"),
        16.0,
        "an instrument without filter time follows the step at once"
    );
    rig.scans(100);
    assert!(
        rig.real("TT103.Value") > 99.0,
        "after ten filter times the value has settled"
    );
}

#[test]
fn a_digital_input_is_debounced() {
    let mut rig = rig();
    rig.set_bool("SwitchContact", true);
    rig.scans(3);
    assert!(
        !rig.flag("LS104.Value"),
        "300 ms is shorter than the 500 ms debounce"
    );
    rig.scans(3);
    assert!(
        rig.flag("LS104.Value"),
        "after 500 ms the switch reads TRUE"
    );
    rig.set_bool("SwitchContact", false);
    rig.scans(3);
    assert!(rig.flag("LS104.Value"), "the release is debounced too");
    rig.scans(3);
    assert!(!rig.flag("LS104.Value"));
}

#[test]
fn a_digital_input_can_be_inverted() {
    let mut rig = rig();
    assert!(
        rig.flag("PS105.Value"),
        "an open contact reads TRUE when inverted"
    );
    rig.set_bool("SwitchContact", true);
    rig.scan();
    assert!(!rig.flag("PS105.Value"));
    assert_eq!(rig.state("PS105.Quality"), "Good");
}

#[test]
fn a_totalizer_integrates_its_rate_per_hour_and_skips_a_bad_rate() {
    let mut rig = rig();
    rig.set_real("FlowRate", 36.0);
    for _ in 0..10 {
        rig.step_ms(10_000);
    }
    assert_close(rig.real("FQ106.Total"), 1.0, "36 per hour for 100 s");
    rig.set_bool("FlowRateGood", false);
    for _ in 0..10 {
        rig.step_ms(10_000);
    }
    assert_close(rig.real("FQ106.Total"), 1.0, "a bad rate is not counted");
    rig.set_bool("FlowRateGood", true);
    rig.set_real("FlowRate", 0.0);
    for _ in 0..10 {
        rig.step_ms(10_000);
    }
    assert_close(
        rig.real("FQ106.Total"),
        1.0,
        "a rate at the cut-off is not counted",
    );
}

#[test]
fn a_counter_counts_rising_edges() {
    let mut rig = rig();
    for pulse in [true, false, true, true, false, true] {
        rig.set_bool("PulseIn", pulse);
        rig.scan();
    }
    assert_eq!(rig.count("CT107.Count"), 3, "three rising edges");
}

#[test]
fn a_pulse_already_high_when_the_counter_starts_is_not_counted() {
    let mut rig = Rig::new(PROGRAM);
    rig.set_bool("PulseIn", true);
    rig.scan();
    assert_eq!(
        rig.count("CT107.Count"),
        0,
        "a pulse high at the first scan is not counted"
    );
    rig.set_bool("PulseIn", false);
    rig.scan();
    rig.set_bool("PulseIn", true);
    rig.scan();
    assert_eq!(rig.count("CT107.Count"), 1);
    rig.warm_restart();
    rig.set_bool("PulseIn", true);
    rig.scan();
    assert_eq!(
        rig.count("CT107.Count"),
        1,
        "the count is retained and a pulse high after the restart is not counted"
    );
}

#[test]
fn a_weigh_scale_tares_only_while_stable() {
    let mut rig = rig();
    rig.press("CmdTare");
    assert!(
        !rig.flag("TareAccepted"),
        "a tare is refused before the reading is stable"
    );
    assert_eq!(rig.real("WT108.TareWeight"), 0.0);
    rig.scans(15);
    assert!(
        rig.flag("WT108.Stable"),
        "the reading has stayed within its band for the stable time"
    );
    rig.press("CmdTare");
    assert!(rig.flag("TareAccepted"));
    assert_eq!(rig.real("WT108.TareWeight"), 250.0);
    assert_eq!(rig.real("WT108.Net"), 0.0);
    rig.set_real("WeightSignal", 13.0);
    rig.scan();
    assert_eq!(
        rig.real("WT108.Net"),
        31.25,
        "net is the value less the tare"
    );
    assert!(!rig.flag("WT108.Stable"), "a moving reading is not stable");
    rig.press("CmdTare");
    assert!(
        !rig.flag("TareAccepted"),
        "a tare is refused while the reading moves"
    );
    assert_eq!(rig.real("WT108.TareWeight"), 250.0);
}

#[test]
fn a_tank_shows_its_level_instrument_with_percent_and_volume() {
    let mut rig = rig();
    assert_eq!(rig.real("T110.Level"), 2.0);
    assert_eq!(rig.real("T110.Percent"), 50.0);
    assert_eq!(rig.real("T110.Volume"), 6.0);
    assert_eq!(rig.state("T110.Quality"), "Good");
    rig.set_real("LevelSignal", 20.0);
    rig.scan();
    assert_eq!(rig.real("T110.Level"), 4.0);
    assert_eq!(rig.real("T110.Volume"), 12.0);
    rig.set_real("LevelSignal", 2.0);
    rig.scan();
    assert_eq!(
        rig.state("T110.Quality"),
        "Bad",
        "the tank reports its instrument's quality"
    );
    assert_eq!(rig.real("T110.Level"), 4.0, "and its held value");
}
