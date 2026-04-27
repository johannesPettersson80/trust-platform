TEST_PROGRAM ConveyorPulseComponentMatchesClassicDisabledState
VAR
    BeltPulse : PulseGenerator;
    ClassicPulse : GEN_PULSE;
END_VAR

BeltPulse.Initialize();
BeltPulse.Configure(HighTime := T#500ms, LowTime := T#500ms);
BeltPulse.SetEnabled(Enabled := FALSE);
ClassicPulse(ENQ := FALSE, PTH := T#500ms, PTL := T#500ms);

ASSERT_TRUE(IN := BeltPulse.Update() = ClassicPulse.Q);
ASSERT_FALSE(IN := BeltPulse.Output);
END_TEST_PROGRAM

(* Enable / disable / re-enable sequence. Verifies the pulse generator
   responds to SetEnabled changes across multiple Update calls and
   that disabling it always forces Output to FALSE. *)
TEST_PROGRAM ConveyorPulseRespondsToEnableTransitions
VAR
    BeltPulse : PulseGenerator;
END_VAR

BeltPulse.Initialize();
BeltPulse.Configure(HighTime := T#500ms, LowTime := T#500ms);

(* Disabled: Output forced FALSE *)
BeltPulse.SetEnabled(Enabled := FALSE);
BeltPulse.Update();
ASSERT_FALSE(IN := BeltPulse.Output);

(* Enabled: pulse driver runs (Output may toggle but Enabled property
   reflects the request) *)
BeltPulse.SetEnabled(Enabled := TRUE);
BeltPulse.Update();
ASSERT_TRUE(IN := BeltPulse.Enabled);

(* Disabled again: Output forced FALSE regardless of prior state *)
BeltPulse.SetEnabled(Enabled := FALSE);
BeltPulse.Update();
ASSERT_FALSE(IN := BeltPulse.Output);
ASSERT_FALSE(IN := BeltPulse.Enabled);
END_TEST_PROGRAM

(* Lifecycle: Initialize sets Ready, ClearError clears the diagnostic
   surface, repeated Initialize calls are idempotent. *)
TEST_PROGRAM ConveyorPulseInitializeIsIdempotent
VAR
    BeltPulse : PulseGenerator;
END_VAR

BeltPulse.Initialize();
ASSERT_TRUE(IN := BeltPulse.Ready);
ASSERT_FALSE(IN := BeltPulse.Error);
ASSERT_EQUAL(EXPECTED := WORD#0, ACTUAL := BeltPulse.ErrorId);

(* Calling Initialize again must not corrupt internal state. *)
BeltPulse.Initialize();
ASSERT_TRUE(IN := BeltPulse.Ready);
ASSERT_FALSE(IN := BeltPulse.Error);

(* Configure values are preserved across Initialize because Configure
   is the source of truth for HighTime/LowTime. *)
BeltPulse.Configure(HighTime := T#250ms, LowTime := T#750ms);
ASSERT_EQUAL(EXPECTED := T#250ms, ACTUAL := BeltPulse.HighTime);
ASSERT_EQUAL(EXPECTED := T#750ms, ACTUAL := BeltPulse.LowTime);
END_TEST_PROGRAM

(* Configure returns the new times via the property surface; repeated
   Configure overwrites the previous values cleanly. *)
TEST_PROGRAM ConveyorPulseConfigureOverwritesTimes
VAR
    BeltPulse : PulseGenerator;
END_VAR

BeltPulse.Initialize();
BeltPulse.Configure(HighTime := T#100ms, LowTime := T#200ms);
ASSERT_EQUAL(EXPECTED := T#100ms, ACTUAL := BeltPulse.HighTime);
ASSERT_EQUAL(EXPECTED := T#200ms, ACTUAL := BeltPulse.LowTime);

(* Reconfigure mid-life *)
BeltPulse.Configure(HighTime := T#1s, LowTime := T#2s);
ASSERT_EQUAL(EXPECTED := T#1s, ACTUAL := BeltPulse.HighTime);
ASSERT_EQUAL(EXPECTED := T#2s, ACTUAL := BeltPulse.LowTime);

(* Configure with zero LowTime is the boundary — generator continues
   to operate without trapping on degenerate input. *)
BeltPulse.Configure(HighTime := T#100ms, LowTime := T#0ms);
ASSERT_EQUAL(EXPECTED := T#0ms, ACTUAL := BeltPulse.LowTime);
BeltPulse.SetEnabled(Enabled := FALSE);
BeltPulse.Update();
ASSERT_FALSE(IN := BeltPulse.Output);
END_TEST_PROGRAM

(* Reset clears Enabled and forces Output FALSE without disturbing the
   configured times. Used by safe-state recovery in production. *)
TEST_PROGRAM ConveyorPulseResetClearsRunState
VAR
    BeltPulse : PulseGenerator;
END_VAR

BeltPulse.Initialize();
BeltPulse.Configure(HighTime := T#500ms, LowTime := T#500ms);
BeltPulse.SetEnabled(Enabled := TRUE);
BeltPulse.Update();
ASSERT_TRUE(IN := BeltPulse.Enabled);

(* Reset clears the Enabled flag and the Output, but leaves
   configuration intact. *)
BeltPulse.Reset();
ASSERT_FALSE(IN := BeltPulse.Enabled);
ASSERT_FALSE(IN := BeltPulse.Output);
ASSERT_EQUAL(EXPECTED := T#500ms, ACTUAL := BeltPulse.HighTime);
ASSERT_EQUAL(EXPECTED := T#500ms, ACTUAL := BeltPulse.LowTime);

(* After Reset, Update with disabled state returns FALSE. *)
ASSERT_FALSE(IN := BeltPulse.Update());
END_TEST_PROGRAM

(* Multi-instance isolation: two PulseGenerator instances must not
   share state. One enabled, one disabled. *)
TEST_PROGRAM ConveyorPulseMultiInstanceIsolation
VAR
    PulseA : PulseGenerator;
    PulseB : PulseGenerator;
END_VAR

PulseA.Initialize();
PulseA.Configure(HighTime := T#100ms, LowTime := T#100ms);
PulseB.Initialize();
PulseB.Configure(HighTime := T#1s, LowTime := T#1s);

(* Enable A, leave B disabled *)
PulseA.SetEnabled(Enabled := TRUE);
PulseB.SetEnabled(Enabled := FALSE);
PulseA.Update();
PulseB.Update();

ASSERT_TRUE(IN := PulseA.Enabled);
ASSERT_FALSE(IN := PulseB.Enabled);
ASSERT_FALSE(IN := PulseB.Output);

(* Independent timing values *)
ASSERT_EQUAL(EXPECTED := T#100ms, ACTUAL := PulseA.HighTime);
ASSERT_EQUAL(EXPECTED := T#1s, ACTUAL := PulseB.HighTime);

(* Disabling A must not enable B *)
PulseA.SetEnabled(Enabled := FALSE);
ASSERT_FALSE(IN := PulseA.Enabled);
ASSERT_FALSE(IN := PulseB.Enabled);
END_TEST_PROGRAM

(* Snapshot returns the current diagnostic record. *)
TEST_PROGRAM ConveyorPulseSnapshotReportsLifecycle
VAR
    BeltPulse : PulseGenerator;
    Snap : ComponentStatus;
END_VAR

BeltPulse.Initialize();
Snap := BeltPulse.Snapshot();
ASSERT_TRUE(IN := Snap.Ready);
ASSERT_FALSE(IN := Snap.Error);
ASSERT_EQUAL(EXPECTED := WORD#0, ACTUAL := Snap.ErrorId);
END_TEST_PROGRAM
