//! Plant library specification §9.4 and §14.10: an application sequence extends FB_Sequence and
//! writes its steps as a CASE on CurrentStep. The tests run its lifecycle (start, steps, complete;
//! hold and resume with the step time frozen; stop through Stopping; abort; reset from an end
//! state), the refusals, the step timeout, and the sequence manager's counts and StopAll.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

(* Fill, mix, complete; a stop drains the tank first. *)
FUNCTION_BLOCK MixingSequence EXTENDS Plant.FB_Sequence
VAR_INPUT
    Filled : BOOL;
    Mixed : BOOL;
    Drained : BOOL;
END_VAR
VAR_OUTPUT
    OpenInlet : BOOL;
    RunMixer : BOOL;
    OpenOutlet : BOOL;
END_VAR
    OpenInlet := FALSE;
    RunMixer := FALSE;
    OpenOutlet := FALSE;
    IF Execute() THEN
        CASE CurrentStep OF
            0:
                GoTo(Number := 10, Name := 'Fill');
            10:
                OpenInlet := TRUE;
                Wait(Text := 'Filling');
                IF Filled THEN
                    GoTo(Number := 20, Name := 'Mix');
                ELSE
                    StepTimeout(Limit := T#5s, Text := 'The tank did not fill within 5 s');
                END_IF;
            20:
                RunMixer := TRUE;
                IF Mixed THEN
                    Complete();
                END_IF;
        END_CASE;
    ELSIF State = E_SequenceState#Stopping THEN
        OpenOutlet := TRUE;
        Wait(Text := 'Draining before the stop');
        IF Drained THEN
            StopDone();
        END_IF;
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Filled : BOOL;
    Mixed : BOOL;
    Drained : BOOL;
    CmdStart : BOOL;
    CmdHold : BOOL;
    CmdResume : BOOL;
    CmdStop : BOOL;
    CmdAbort : BOOL;
    CmdReset : BOOL;
    CmdStartSecond : BOOL;
    CmdHoldSecond : BOOL;
    CmdStopAll : BOOL;
    Accepted : BOOL;
    StopAllAccepted : BOOL;
    Batch : MixingSequence;
    Second : MixingSequence;
    Overview : Plant.FB_SequenceManager;
END_VAR
    IF CmdStart THEN
        Accepted := Batch.Start();
        CmdStart := FALSE;
    END_IF;
    IF CmdHold THEN
        Accepted := Batch.Hold();
        CmdHold := FALSE;
    END_IF;
    IF CmdResume THEN
        Accepted := Batch.Resume();
        CmdResume := FALSE;
    END_IF;
    IF CmdStop THEN
        Accepted := Batch.Stop();
        CmdStop := FALSE;
    END_IF;
    IF CmdAbort THEN
        Accepted := Batch.Abort();
        CmdAbort := FALSE;
    END_IF;
    IF CmdReset THEN
        Accepted := Batch.Reset();
        CmdReset := FALSE;
    END_IF;
    IF CmdStartSecond THEN
        Second.Start();
        CmdStartSecond := FALSE;
    END_IF;
    IF CmdHoldSecond THEN
        Second.Hold();
        CmdHoldSecond := FALSE;
    END_IF;
    IF CmdStopAll THEN
        StopAllAccepted := Overview.StopAll();
        CmdStopAll := FALSE;
    END_IF;
    Batch(Filled := Filled, Mixed := Mixed, Drained := Drained);
    Second(Filled := FALSE, Mixed := FALSE, Drained := Drained);
    Overview(Sequence1 := Batch, Sequence2 := Second);
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

/// Presses a command and returns whether the sequence accepted it.
fn command(rig: &mut Rig, name: &str) -> bool {
    rig.press(name);
    rig.flag("Accepted")
}

#[test]
fn a_sequence_runs_from_start_through_its_steps_to_complete() {
    let mut rig = rig();
    assert_eq!(rig.state("Batch.State"), "Idle");
    assert!(rig.flag("Batch.CanStart"));
    assert!(command(&mut rig, "CmdStart"), "Start is accepted in Idle");
    assert_eq!(rig.state("Batch.State"), "Running");
    assert_eq!(rig.text("Batch.StateText"), "Running");
    assert_eq!(
        rig.int("Batch.CurrentStep"),
        10,
        "step 0 went on to the first step"
    );
    assert_eq!(rig.text("Batch.StepName"), "Fill");
    rig.scan();
    assert!(
        rig.flag("Batch.OpenInlet"),
        "the step commands its equipment"
    );
    assert_eq!(rig.text("Batch.Message"), "Filling");
    rig.set_bool("Filled", true);
    rig.scan();
    assert_eq!(rig.int("Batch.CurrentStep"), 20);
    assert_eq!(rig.text("Batch.StepName"), "Mix");
    rig.scan();
    assert!(rig.flag("Batch.RunMixer") && !rig.flag("Batch.OpenInlet"));
    rig.set_bool("Mixed", true);
    rig.scan();
    assert_eq!(rig.state("Batch.State"), "Completed");
    assert_eq!(rig.text("Batch.StateText"), "Completed");
    assert!(rig.flag("Batch.CanReset") && !rig.flag("Batch.CanStart"));
    rig.scan();
    assert!(
        !rig.flag("Batch.RunMixer"),
        "a completed sequence commands nothing"
    );
}

#[test]
fn hold_freezes_the_step_time_and_resume_continues_it() {
    let mut rig = rig();
    command(&mut rig, "CmdStart");
    rig.scans(10);
    let before = rig.millis("Batch.StepTime");
    assert_eq!(before, 1_000, "ten scans in the step");
    assert!(
        command(&mut rig, "CmdHold"),
        "Hold is accepted while running"
    );
    assert_eq!(rig.state("Batch.State"), "Held");
    assert!(rig.flag("Batch.CanResume"));
    assert!(
        !rig.flag("Batch.OpenInlet"),
        "a held sequence commands nothing"
    );
    rig.scans(20);
    assert_eq!(
        rig.millis("Batch.StepTime"),
        before,
        "the step time stands still while held"
    );
    assert!(
        command(&mut rig, "CmdResume"),
        "Resume is accepted while held"
    );
    assert_eq!(rig.state("Batch.State"), "Running");
    assert_eq!(
        rig.millis("Batch.StepTime"),
        before + 100,
        "the step time continues where it stopped"
    );
    assert!(rig.flag("Batch.OpenInlet"));
}

#[test]
fn stop_goes_through_stopping_to_stopped_after_stop_done() {
    let mut rig = rig();
    command(&mut rig, "CmdStart");
    rig.scan();
    assert!(
        command(&mut rig, "CmdStop"),
        "Stop is accepted while running"
    );
    assert_eq!(rig.state("Batch.State"), "Stopping");
    assert!(
        rig.flag("Batch.OpenOutlet"),
        "the application's stop steps run"
    );
    assert_eq!(rig.text("Batch.Message"), "Draining before the stop");
    assert!(rig.flag("Batch.CanAbort") && !rig.flag("Batch.CanStop"));
    rig.scans(5);
    assert_eq!(
        rig.state("Batch.State"),
        "Stopping",
        "stopping lasts until the application calls StopDone"
    );
    rig.set_bool("Drained", true);
    rig.scan();
    assert_eq!(rig.state("Batch.State"), "Stopped");
    assert!(rig.flag("Batch.CanReset"));
}

#[test]
fn abort_ends_a_running_sequence() {
    let mut rig = rig();
    command(&mut rig, "CmdStart");
    assert!(
        command(&mut rig, "CmdAbort"),
        "Abort is accepted while running"
    );
    assert_eq!(rig.state("Batch.State"), "Aborted");
    assert_eq!(rig.text("Batch.Message"), "Aborted by the operator");
    rig.scan();
    assert!(!rig.flag("Batch.OpenInlet"));
}

#[test]
fn reset_is_accepted_only_from_an_end_state() {
    let mut rig = rig();
    assert!(!command(&mut rig, "CmdReset"), "Reset is refused in Idle");
    command(&mut rig, "CmdStart");
    assert!(
        !command(&mut rig, "CmdReset"),
        "Reset is refused while running"
    );
    assert_eq!(rig.state("Batch.State"), "Running");
    command(&mut rig, "CmdAbort");
    assert!(
        command(&mut rig, "CmdReset"),
        "Reset is accepted when aborted"
    );
    assert_eq!(rig.state("Batch.State"), "Idle");
    assert_eq!(rig.int("Batch.CurrentStep"), 0);
    assert!(rig.flag("Batch.CanStart"));
}

#[test]
fn commands_that_do_not_apply_are_refused() {
    let mut rig = rig();
    for refused in ["CmdHold", "CmdResume", "CmdStop", "CmdAbort"] {
        assert!(!command(&mut rig, refused), "{refused} is refused in Idle");
        assert_eq!(rig.state("Batch.State"), "Idle");
    }
    assert!(command(&mut rig, "CmdStart"));
    assert!(
        !command(&mut rig, "CmdStart"),
        "Start is refused while running"
    );
    assert!(
        !command(&mut rig, "CmdResume"),
        "Resume is refused while running"
    );
    assert_eq!(rig.state("Batch.State"), "Running");
}

#[test]
fn a_step_timeout_fails_the_sequence_with_its_text() {
    let mut rig = rig();
    command(&mut rig, "CmdStart");
    rig.scans(40);
    assert_eq!(
        rig.state("Batch.State"),
        "Running",
        "4 s is within the 5 s step limit"
    );
    rig.scans(20);
    assert_eq!(rig.state("Batch.State"), "Failed");
    assert_eq!(
        rig.text("Batch.Message"),
        "The tank did not fill within 5 s"
    );
    assert_eq!(rig.text("Batch.StateText"), "Failed");
    assert!(rig.flag("Batch.CanReset"));
}

#[test]
fn the_sequence_manager_counts_states_and_stop_all_stops_every_sequence() {
    let mut rig = rig();
    command(&mut rig, "CmdStart");
    rig.press("CmdStartSecond");
    assert_eq!(rig.int("Overview.RunningCount"), 2);
    rig.press("CmdHoldSecond");
    assert_eq!(rig.int("Overview.RunningCount"), 1);
    assert_eq!(rig.int("Overview.HeldCount"), 1);
    rig.press("CmdStopAll");
    assert!(rig.flag("StopAllAccepted"));
    assert_eq!(
        rig.state("Batch.State"),
        "Stopping",
        "a running sequence stops"
    );
    assert_eq!(
        rig.state("Second.State"),
        "Stopping",
        "and so does a held one"
    );
    rig.set_bool("Drained", true);
    rig.scan();
    assert_eq!(rig.state("Batch.State"), "Stopped");
    assert_eq!(rig.state("Second.State"), "Stopped");
    assert_eq!(rig.int("Overview.RunningCount"), 0);
    assert_eq!(rig.int("Overview.HeldCount"), 0);
}

#[test]
fn the_sequence_manager_counts_a_failed_sequence() {
    let mut rig = rig();
    rig.press("CmdStartSecond");
    rig.scans(60);
    assert_eq!(rig.state("Second.State"), "Failed");
    assert_eq!(rig.int("Overview.FailedCount"), 1);
    assert_eq!(rig.int("Overview.RunningCount"), 0);
}

#[test]
fn a_sequence_is_idle_after_a_restart() {
    let mut rig = rig();
    command(&mut rig, "CmdStart");
    rig.scans(3);
    rig.warm_restart();
    rig.scan();
    assert_eq!(
        rig.state("Batch.State"),
        "Idle",
        "nothing of a sequence is retained"
    );
    assert_eq!(rig.int("Batch.CurrentStep"), 0);
}
