//! Plant library specification §9.1 and §9.3: condition sets report a first-out text held until
//! every condition is healthy again, and which conditions are in use; an interlock bypass makes its condition healthy and ends by
//! itself; protection follows its conditions; the alarm manager summarises its sources' alarms,
//! sounds the horn on a new alarm until silenced and acknowledges only the alarms present.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    LevelOk : BOOL := TRUE;
    PressureOk : BOOL := TRUE;
    DoorClosed : BOOL := TRUE;
    MotorTrip : BOOL;
    PumpTrip : BOOL;
    CmdBypass : BOOL;
    CmdRemoveBypass : BOOL;
    CmdSilence : BOOL;
    CmdAcknowledgeAll : BOOL;
    BypassAccepted : BOOL;
    Trips : Plant.FB_Interlocks := (BypassTime := T#2s);
    Guard : Plant.FB_Protection;
    P1 : Plant.FB_Pump := (HasFeedback := FALSE);
    P2 : Plant.FB_Pump := (HasFeedback := FALSE);
    Alarms : Plant.FB_AlarmManager;
END_VAR
    IF CmdBypass THEN
        BypassAccepted := Trips.Bypass(Index := 1);
        CmdBypass := FALSE;
    END_IF;
    IF CmdRemoveBypass THEN
        Trips.RemoveBypass(Index := 1);
        CmdRemoveBypass := FALSE;
    END_IF;
    IF CmdSilence THEN
        Alarms.Silence();
        CmdSilence := FALSE;
    END_IF;
    IF CmdAcknowledgeAll THEN
        Alarms.AcknowledgeAll();
        CmdAcknowledgeAll := FALSE;
    END_IF;
    Trips(Condition1 := LevelOk, Text1 := 'Tank level low', Condition2 := PressureOk,
        Text2 := 'Suction pressure low', Condition3 := DoorClosed);
    Guard(Condition1 := PressureOk, Text1 := 'Motor overtemperature');
    P1(Trip := MotorTrip);
    P2(Trip := PumpTrip);
    Alarms(Source1 := P1, Source2 := P2);
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn the_first_out_text_is_held_until_every_condition_is_healthy() {
    let mut rig = rig();
    assert!(rig.flag("Trips.Healthy") && !rig.flag("Trips.Active"));
    assert_eq!(rig.text("Trips.Text"), "");
    rig.set_bool("PressureOk", false);
    rig.scan();
    assert!(rig.flag("Trips.Active") && !rig.flag("Trips.Healthy"));
    assert_eq!(rig.text("Trips.Text"), "Suction pressure low");
    assert_eq!(rig.int("Trips.FirstOut"), 2);
    rig.set_bool("LevelOk", false);
    rig.scan();
    assert_eq!(
        rig.text("Trips.Text"),
        "Suction pressure low",
        "a later condition does not replace the first out"
    );
    rig.set_bool("PressureOk", true);
    rig.scan();
    assert_eq!(
        rig.text("Trips.Text"),
        "Suction pressure low",
        "the first out is held while any condition is unhealthy"
    );
    assert!(rig.flag("Trips.Active"));
    rig.set_bool("LevelOk", true);
    rig.scan();
    assert!(rig.flag("Trips.Healthy"));
    assert_eq!(rig.text("Trips.Text"), "", "all healthy again");
    assert_eq!(rig.int("Trips.FirstOut"), 0);
}

#[test]
fn a_condition_is_in_use_while_it_has_a_text_or_is_not_healthy() {
    let mut rig = rig();
    let in_use = |rig: &Rig| -> Vec<bool> {
        (1..=4)
            .map(|index| rig.flag(&format!("Trips.InUse[{index}]")))
            .collect()
    };
    assert_eq!(
        in_use(&rig),
        [true, true, false, false],
        "the two conditions with texts"
    );
    rig.set_bool("DoorClosed", false);
    rig.scan();
    assert_eq!(
        in_use(&rig),
        [true, true, true, false],
        "an unhealthy condition without a text is shown too"
    );
}

#[test]
fn a_condition_without_text_is_named_by_its_number() {
    let mut rig = rig();
    rig.set_bool("DoorClosed", false);
    rig.scan();
    assert_eq!(rig.text("Trips.Text"), "Condition 3");
}

#[test]
fn a_bypass_makes_a_condition_healthy_and_ends_by_itself() {
    let mut rig = rig();
    rig.set_bool("LevelOk", false);
    rig.scan();
    assert!(rig.flag("Trips.Active"));
    rig.press("CmdBypass");
    assert!(rig.flag("BypassAccepted"));
    assert!(
        rig.flag("Trips.Healthy"),
        "a bypassed condition counts as healthy"
    );
    assert!(rig.flag("Trips.Bypassed[1]") && rig.flag("Trips.BypassActive"));
    rig.scans(14);
    assert!(
        rig.flag("Trips.Healthy"),
        "1.4 s is within the 2 s bypass time"
    );
    rig.scans(10);
    assert!(
        !rig.flag("Trips.Bypassed[1]") && !rig.flag("Trips.BypassActive"),
        "the bypass ended by itself after the bypass time"
    );
    assert!(rig.flag("Trips.Active"), "the condition holds again");
    assert_eq!(rig.text("Trips.Text"), "Tank level low");
}

#[test]
fn remove_bypass_ends_a_bypass_early() {
    let mut rig = rig();
    rig.set_bool("LevelOk", false);
    rig.press("CmdBypass");
    assert!(rig.flag("Trips.Healthy"));
    rig.press("CmdRemoveBypass");
    assert!(!rig.flag("Trips.Bypassed[1]") && rig.flag("Trips.Active"));
}

#[test]
fn protection_follows_its_condition() {
    let mut rig = rig();
    assert!(rig.flag("Guard.Healthy"));
    rig.set_bool("PressureOk", false);
    rig.scan();
    assert!(rig.flag("Guard.Active") && !rig.flag("Guard.Healthy"));
    assert_eq!(rig.text("Guard.Text"), "Motor overtemperature");
    rig.set_bool("PressureOk", true);
    rig.scan();
    assert!(rig.flag("Guard.Healthy"));
}

#[test]
fn the_alarm_manager_counts_the_active_and_unacknowledged_alarms_of_its_sources() {
    let mut rig = rig();
    assert_eq!(rig.int("Alarms.ActiveCount"), 0);
    rig.set_bool("MotorTrip", true);
    rig.scan();
    assert_eq!(rig.int("Alarms.ActiveCount"), 1, "P1's trip alarm");
    assert_eq!(rig.int("Alarms.UnacknowledgedCount"), 1);
    rig.set_bool("PumpTrip", true);
    rig.scan();
    assert_eq!(rig.int("Alarms.ActiveCount"), 2, "and P2's");
    assert_eq!(rig.int("Alarms.UnacknowledgedCount"), 2);
}

#[test]
fn the_horn_sounds_on_a_new_alarm_until_silenced() {
    let mut rig = rig();
    assert!(!rig.flag("Alarms.Horn"));
    rig.set_bool("MotorTrip", true);
    rig.scan();
    assert!(rig.flag("Alarms.Horn"), "a new alarm sounds the horn");
    rig.press("CmdSilence");
    assert!(!rig.flag("Alarms.Horn"), "Silence stops the horn");
    assert_eq!(
        rig.int("Alarms.UnacknowledgedCount"),
        1,
        "silencing does not acknowledge"
    );
    rig.scans(3);
    assert!(!rig.flag("Alarms.Horn"), "the horn stays silent");
    rig.set_bool("PumpTrip", true);
    rig.scan();
    assert!(
        rig.flag("Alarms.Horn"),
        "the next new alarm sounds it again"
    );
}

#[test]
fn acknowledge_all_acknowledges_only_the_alarms_present_now() {
    let mut rig = rig();
    rig.set_bool("MotorTrip", true);
    rig.scan();
    rig.press("CmdAcknowledgeAll");
    assert_eq!(rig.int("Alarms.UnacknowledgedCount"), 0);
    assert_eq!(
        rig.int("Alarms.ActiveCount"),
        1,
        "acknowledged alarms stay active"
    );
    assert!(!rig.flag("P1.TripAlarm.Unacknowledged"));
    rig.set_bool("PumpTrip", true);
    rig.scan();
    assert_eq!(
        rig.int("Alarms.UnacknowledgedCount"),
        1,
        "an alarm raised after the acknowledgement stays unacknowledged"
    );
    assert!(rig.flag("P2.TripAlarm.Unacknowledged"));
    assert!(!rig.flag("P1.TripAlarm.Unacknowledged"));
}
