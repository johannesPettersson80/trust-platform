//! Plant library specification §9.2 and §14.9: groups command interchangeable members by
//! composition. Duty/standby starts the available member with the fewest run hours, replaces a
//! member that fails, and raises a shortfall when none is available; lead/lag runs the required
//! number; a staged group follows its demand with hysteresis; a member in Manual is not used.

mod plant_support;

use plant_support::Rig;

/// Two pumps with feedback in a duty/standby group; each contactor answers in the next scan
/// while its pump is healthy.
const DUTY_PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    RunGroup : BOOL;
    P1Healthy : BOOL := TRUE;
    P2Healthy : BOOL := TRUE;
    CmdP1Manual : BOOL;
    CmdP1Start : BOOL;
    CmdP1Stop : BOOL;
    CmdP1Auto : BOOL;
    CmdP2Manual : BOOL;
    P1 : Plant.FB_Pump := (StartTimeout := T#2s);
    P2 : Plant.FB_Pump := (StartTimeout := T#2s);
    Duty : Plant.FB_DutyStandbyGroup := (ShortfallDelay := T#3s);
END_VAR
    IF CmdP1Manual THEN
        P1.SetMode(Value := E_Mode#Manual);
        CmdP1Manual := FALSE;
    END_IF;
    IF CmdP1Start THEN
        P1.Start();
        CmdP1Start := FALSE;
    END_IF;
    IF CmdP1Stop THEN
        P1.Stop();
        CmdP1Stop := FALSE;
    END_IF;
    IF CmdP1Auto THEN
        P1.SetMode(Value := E_Mode#Auto);
        CmdP1Auto := FALSE;
    END_IF;
    IF CmdP2Manual THEN
        P2.SetMode(Value := E_Mode#Manual);
        CmdP2Manual := FALSE;
    END_IF;
    Duty(Run := RunGroup, Member1 := P1, Member2 := P2);
    P1(RunningFeedback := P1.RunCommand AND P1Healthy);
    P2(RunningFeedback := P2.RunCommand AND P2Healthy);
END_PROGRAM
"#;

/// A lead/lag group of three pumps and a staged group of two, changing members without delay.
const STAGING_PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    RequiredPumps : INT;
    DemandPercent : LREAL;
    A1 : Plant.FB_Pump := (HasFeedback := FALSE);
    A2 : Plant.FB_Pump := (HasFeedback := FALSE);
    A3 : Plant.FB_Pump := (HasFeedback := FALSE);
    B1 : Plant.FB_Pump := (HasFeedback := FALSE);
    B2 : Plant.FB_Pump := (HasFeedback := FALSE);
    LeadLag : Plant.FB_LeadLagGroup := (StartDelay := T#0s, StopDelay := T#0s);
    Staged : Plant.FB_StagedGroup := (StartDelay := T#0s, StopDelay := T#0s);
END_VAR
    LeadLag(Required := RequiredPumps, Member1 := A1, Member2 := A2, Member3 := A3);
    Staged(Demand := DemandPercent, Member1 := B1, Member2 := B2);
    A1();
    A2();
    A3();
    B1();
    B2();
END_PROGRAM
"#;

fn running(rig: &Rig, pumps: &[&str]) -> usize {
    pumps
        .iter()
        .filter(|pump| rig.flag(&format!("{pump}.RunCommand")))
        .count()
}

#[test]
fn duty_standby_starts_the_member_with_fewer_run_hours() {
    let mut rig = Rig::new(DUTY_PROGRAM);
    rig.scan();
    // P1 runs for an hour by hand, then returns to Auto.
    rig.press("CmdP1Manual");
    rig.press("CmdP1Start");
    rig.scan();
    rig.step_ms(3_600_000);
    rig.press("CmdP1Stop");
    rig.press("CmdP1Auto");
    rig.scan();
    assert!(rig.real("P1.RunHours") > 0.99, "P1 has run for an hour");
    rig.set_bool("RunGroup", true);
    rig.scans(3);
    assert!(
        rig.flag("Duty.Selected[2]") && !rig.flag("Duty.Selected[1]"),
        "the group selects P2, the member with fewer run hours"
    );
    assert!(rig.flag("P2.Running") && !rig.flag("P1.RunCommand"));
    assert_eq!(rig.int("Duty.RunningCount"), 1);
}

#[test]
fn duty_standby_replaces_a_member_that_fails_to_start() {
    let mut rig = Rig::new(DUTY_PROGRAM);
    rig.scan();
    rig.set_bool("P1Healthy", false);
    rig.set_bool("RunGroup", true);
    rig.scan();
    assert!(
        rig.flag("Duty.Selected[1]") && rig.flag("P1.RunCommand"),
        "with equal run hours the group starts the first member"
    );
    rig.scans(30);
    assert!(
        rig.flag("P1.FailedToStart") && !rig.flag("P1.RunCommand"),
        "P1 failed to start: no feedback within its start timeout"
    );
    assert!(
        rig.flag("Duty.Selected[2]") && !rig.flag("Duty.Selected[1]"),
        "the group replaced the failed member"
    );
    assert!(rig.flag("P2.Running"), "the standby pump runs");
    assert!(!rig.flag("Duty.Shortfall"), "the replacement came in time");
}

#[test]
fn duty_standby_raises_a_shortfall_when_no_member_is_available() {
    let mut rig = Rig::new(DUTY_PROGRAM);
    rig.scan();
    rig.press("CmdP1Manual");
    rig.press("CmdP2Manual");
    rig.set_bool("RunGroup", true);
    rig.scans(20);
    assert_eq!(
        rig.int("Duty.AvailableCount"),
        0,
        "members in Manual are not available"
    );
    assert!(
        !rig.flag("Duty.Shortfall"),
        "2 s is within the 3 s shortfall delay"
    );
    rig.scans(15);
    assert!(
        rig.flag("Duty.Shortfall"),
        "no member runs for longer than the shortfall delay"
    );
    assert!(rig.flag("Duty.ShortfallAlarm.Active"));
    assert_eq!(running(&rig, &["P1", "P2"]), 0);
}

#[test]
fn a_member_in_manual_is_not_used_by_the_group() {
    let mut rig = Rig::new(DUTY_PROGRAM);
    rig.scan();
    rig.press("CmdP1Manual");
    rig.set_bool("RunGroup", true);
    rig.scans(3);
    assert!(
        !rig.flag("P1.RunCommand") && !rig.flag("Duty.Selected[1]"),
        "P1 in Manual is not used, although it is the first member with the same run hours"
    );
    assert!(rig.flag("P2.RunCommand") && rig.flag("Duty.Selected[2]"));
}

#[test]
fn lead_lag_runs_the_required_number_of_members() {
    let mut rig = Rig::new(STAGING_PROGRAM);
    rig.scan();
    let pumps = ["A1", "A2", "A3"];
    for required in [2_i16, 3, 1, 0] {
        rig.set_int("RequiredPumps", required);
        rig.scans(5);
        assert_eq!(
            running(&rig, &pumps),
            usize::try_from(required).expect("a count"),
            "pumps running with {required} required"
        );
        assert_eq!(rig.int("LeadLag.RunningCount"), i64::from(required));
        assert_eq!(rig.int("LeadLag.Target"), i64::from(required));
    }
}

#[test]
fn a_staged_group_follows_its_demand_with_hysteresis() {
    let mut rig = Rig::new(STAGING_PROGRAM);
    rig.scan();
    let pumps = ["B1", "B2"];
    // Two members: a stage is 50 %; a stage drops 5 % (the hysteresis) below its threshold.
    for (demand, stages) in [(30.0, 1), (60.0, 2), (46.0, 2), (44.0, 1), (0.0, 0)] {
        rig.set_real("DemandPercent", demand);
        rig.scans(5);
        assert_eq!(rig.int("Staged.Target"), stages, "stages at {demand} %");
        assert_eq!(
            running(&rig, &pumps),
            usize::try_from(stages).expect("a count"),
            "members running at {demand} %"
        );
    }
}
