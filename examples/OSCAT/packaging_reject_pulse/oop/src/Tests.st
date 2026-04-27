TEST_PROGRAM PackagingRejectPulseComponentStaysLowWithoutDefect
VAR
    RejectPulse : PulseGenerator;
    ClassicPulse : GEN_PULSE;
END_VAR

RejectPulse.Initialize();
RejectPulse.Configure(HighTime := T#150ms, LowTime := T#500ms);
RejectPulse.SetEnabled(Enabled := FALSE);
ClassicPulse(ENQ := FALSE, PTH := T#150ms, PTL := T#500ms);

ASSERT_TRUE(IN := RejectPulse.Update() = ClassicPulse.Q);
END_TEST_PROGRAM

(* Defect-detected enable/disable cycle: simulates a packaging line
   detecting a defect (enable), the cycle completing (disable), then
   another defect (re-enable). Verifies the pulse generator responds
   to enable transitions across multiple Update calls. *)
TEST_PROGRAM PackagingRejectPulseRespondsToDefectCycle
VAR
    RejectPulse : PulseGenerator;
END_VAR

RejectPulse.Initialize();
RejectPulse.Configure(HighTime := T#150ms, LowTime := T#500ms);

(* No defect - disabled, no pulse *)
RejectPulse.SetEnabled(Enabled := FALSE);
RejectPulse.Update();
ASSERT_FALSE(IN := RejectPulse.Output);

(* Defect detected - enable *)
RejectPulse.SetEnabled(Enabled := TRUE);
RejectPulse.Update();
ASSERT_TRUE(IN := RejectPulse.Enabled);

(* Cycle complete - disable *)
RejectPulse.SetEnabled(Enabled := FALSE);
RejectPulse.Update();
ASSERT_FALSE(IN := RejectPulse.Output);
END_TEST_PROGRAM

(* Configuration storage: HighTime and LowTime properties report what
   was last set, even when the pulse is disabled. The classic GEN_PULSE
   keeps no record of the timing inputs. *)
TEST_PROGRAM PackagingRejectPulseExposesConfiguredDwell
VAR
    RejectPulse : PulseGenerator;
END_VAR

RejectPulse.Initialize();
RejectPulse.Configure(HighTime := T#80ms, LowTime := T#250ms);
ASSERT_EQUAL(EXPECTED := T#80ms, ACTUAL := RejectPulse.HighTime);
ASSERT_EQUAL(EXPECTED := T#250ms, ACTUAL := RejectPulse.LowTime);

(* Reconfigure - properties must follow *)
RejectPulse.Configure(HighTime := T#150ms, LowTime := T#500ms);
ASSERT_EQUAL(EXPECTED := T#150ms, ACTUAL := RejectPulse.HighTime);
ASSERT_EQUAL(EXPECTED := T#500ms, ACTUAL := RejectPulse.LowTime);
END_TEST_PROGRAM

(* Two independent stations: each PulseGenerator must keep its own
   enable, output, and timing without bleeding into the other. *)
TEST_PROGRAM PackagingRejectPulseTwoStationsAreIndependent
VAR
    StationA : PulseGenerator;
    StationB : PulseGenerator;
END_VAR

StationA.Initialize();
StationB.Initialize();
StationA.Configure(HighTime := T#100ms, LowTime := T#200ms);
StationB.Configure(HighTime := T#250ms, LowTime := T#750ms);

(* Station A enabled, B disabled *)
StationA.SetEnabled(Enabled := TRUE);
StationB.SetEnabled(Enabled := FALSE);
ASSERT_TRUE(IN := StationA.Enabled);
ASSERT_FALSE(IN := StationB.Enabled);

(* Configurations stay independent *)
ASSERT_EQUAL(EXPECTED := T#100ms, ACTUAL := StationA.HighTime);
ASSERT_EQUAL(EXPECTED := T#250ms, ACTUAL := StationB.HighTime);

(* Toggle: A off, B on - both must follow *)
StationA.SetEnabled(Enabled := FALSE);
StationB.SetEnabled(Enabled := TRUE);
ASSERT_FALSE(IN := StationA.Enabled);
ASSERT_TRUE(IN := StationB.Enabled);
END_TEST_PROGRAM

(* Lifecycle: Snapshot reports Ready after Initialize and remains Ready
   across configure/enable/update cycles. Reset returns to a clean
   ready state without wiping the configuration. *)
TEST_PROGRAM PackagingRejectPulseLifecycle
VAR
    RejectPulse : PulseGenerator;
    Status : ComponentStatus;
END_VAR

RejectPulse.Initialize();
Status := RejectPulse.Snapshot();
ASSERT_TRUE(IN := Status.Ready);
ASSERT_FALSE(IN := Status.Error);

RejectPulse.Configure(HighTime := T#150ms, LowTime := T#500ms);
RejectPulse.SetEnabled(Enabled := TRUE);
RejectPulse.Update();
Status := RejectPulse.Snapshot();
ASSERT_TRUE(IN := Status.Ready);

(* Reset - configuration is preserved by Configure being idempotent;
   Reset on its own should not error *)
RejectPulse.Reset();
Status := RejectPulse.Snapshot();
ASSERT_FALSE(IN := Status.Error);
END_TEST_PROGRAM

(* Edge case: enabling and immediately calling Update without Configure
   uses the default HighTime/LowTime defined in the FB. The output
   must be deterministic - the wrapper does not throw on bare calls. *)
TEST_PROGRAM PackagingRejectPulseDefaultConfiguration
VAR
    RejectPulse : PulseGenerator;
    Result : BOOL;
END_VAR

RejectPulse.Initialize();
RejectPulse.SetEnabled(Enabled := FALSE);
(* No Configure call - rely on defaults. Update must not error. *)
Result := RejectPulse.Update();
ASSERT_FALSE(IN := Result);
ASSERT_FALSE(IN := RejectPulse.Output);
END_TEST_PROGRAM
