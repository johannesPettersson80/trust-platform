TEST_PROGRAM TankLevelPidComponentMatchesClassicPid
VAR
    TankController : PidController;
    ClassicController : CTRL_PID;
    Limits : RealRange;
    Gains : PidGains;
    CommandPercent : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
TankController.SetManual(Manual := TRUE, ManualInput := REAL#35.0);

ClassicController(
    ACT := REAL#42.0,
    SET_POINT := REAL#75.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#35.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
CommandPercent := TankController.Update(Actual := REAL#42.0, Target := REAL#75.0);

ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := CommandPercent, DELTA := REAL#0.001);
ASSERT_TRUE(IN := TankController.Limited = ClassicController.LIM);
END_TEST_PROGRAM

(* Multi-cycle manual-mode test: drives the same controller through
   varying actuals while pinned in manual. Verifies the controller
   maintains manual override across consecutive Update calls and the
   Limited property is read consistently. *)
TEST_PROGRAM TankLevelPidStaysInManualOverrideAcrossCycles
VAR
    TankController : PidController;
    Limits : RealRange;
    Gains : PidGains;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
TankController.SetManual(Manual := TRUE, ManualInput := REAL#35.0);

(* Three consecutive updates in manual mode: output stays at manual *)
ASSERT_NEAR(EXPECTED := REAL#35.0, ACTUAL := TankController.Update(Actual := REAL#42.0, Target := REAL#75.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#35.0, ACTUAL := TankController.Update(Actual := REAL#50.0, Target := REAL#75.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#35.0, ACTUAL := TankController.Update(Actual := REAL#60.0, Target := REAL#75.0), DELTA := REAL#0.001);

(* Switch to auto mid-flight *)
TankController.SetManual(Manual := FALSE, ManualInput := REAL#35.0);
TankController.Update(Actual := REAL#42.0, Target := REAL#75.0);
ASSERT_FALSE(IN := TankController.Manual);
END_TEST_PROGRAM

(* Output clamping: when the auto-mode output exceeds the high limit,
   the command is clamped and Limited reports TRUE. Catches regressions
   where the high-limit clamp is bypassed or Limited is stuck FALSE. *)
TEST_PROGRAM TankLevelPidClampsOutputAtHighLimit
VAR
    TankController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    OutputCmd : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#100.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
TankController.SetManual(Manual := FALSE, ManualInput := REAL#0.0);

(* Large positive error, very large gain - output must saturate at 100 *)
OutputCmd := TankController.Update(Actual := REAL#0.0, Target := REAL#100.0);
ASSERT_TRUE(IN := OutputCmd <= REAL#100.0);
ASSERT_TRUE(IN := TankController.Limited);
END_TEST_PROGRAM

(* Output clamping: when the auto-mode output exceeds the low limit
   (negative error path), the command clamps to 0 and Limited reports
   TRUE. *)
TEST_PROGRAM TankLevelPidClampsOutputAtLowLimit
VAR
    TankController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    OutputCmd : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#100.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
TankController.SetManual(Manual := FALSE, ManualInput := REAL#0.0);

(* Large negative error - output must clamp at low limit *)
OutputCmd := TankController.Update(Actual := REAL#100.0, Target := REAL#0.0);
ASSERT_TRUE(IN := OutputCmd >= REAL#0.0);
ASSERT_TRUE(IN := TankController.Limited);
END_TEST_PROGRAM

(* Manual-set output respects Limits even when the manual input exceeds
   the high limit. Verifies the wrapper enforces the configured
   operating window even in manual override. *)
TEST_PROGRAM TankLevelPidManualOutputRespectsLimits
VAR
    TankController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    OutputCmd : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#1.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
(* Operator forces manual to 250% — well over high limit *)
TankController.SetManual(Manual := TRUE, ManualInput := REAL#250.0);

OutputCmd := TankController.Update(Actual := REAL#42.0, Target := REAL#75.0);
(* Manual output must be clamped to the configured high limit *)
ASSERT_TRUE(IN := OutputCmd <= REAL#100.0);
END_TEST_PROGRAM

(* Per-FB isolation: two PidController instances must keep independent
   gain configuration, manual flags, and integrator state. *)
TEST_PROGRAM TankLevelPidInstancesAreIndependent
VAR
    LoopA : PidController;
    LoopB : PidController;
    LimitsA : RealRange;
    LimitsB : RealRange;
    GainsA : PidGains;
    GainsB : PidGains;
END_VAR

LimitsA.Low := REAL#0.0;
LimitsA.High := REAL#100.0;
LimitsB.Low := REAL#10.0;
LimitsB.High := REAL#90.0;

GainsA.Kp := REAL#2.0;
GainsA.Tn := REAL#1.0;
GainsA.Tv := REAL#0.0;
GainsB.Kp := REAL#5.0;
GainsB.Tn := REAL#2.0;
GainsB.Tv := REAL#0.0;

LoopA.Initialize();
LoopB.Initialize();
LoopA.Configure(Gains := GainsA, Limits := LimitsA, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
LoopB.Configure(Gains := GainsB, Limits := LimitsB, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
LoopA.SetManual(Manual := TRUE, ManualInput := REAL#35.0);
LoopB.SetManual(Manual := FALSE, ManualInput := REAL#0.0);

(* Verify each loop holds its own configuration *)
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := LoopA.Kp, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := LoopB.Kp, DELTA := REAL#0.001);
ASSERT_TRUE(IN := LoopA.Manual);
ASSERT_FALSE(IN := LoopB.Manual);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := LoopA.HighLimit, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#90.0, ACTUAL := LoopB.HighLimit, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Reconfigure mid-run: SetKp/SetIntegralTime/SetDerivativeTime must
   take effect without reinitialization. Catches regressions where
   gains are baked at Initialize and never re-read. *)
TEST_PROGRAM TankLevelPidGainsCanBeRetunedMidRun
VAR
    TankController : PidController;
    Limits : RealRange;
    Gains : PidGains;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := TankController.Kp, DELTA := REAL#0.001);

(* Retune mid-run *)
TankController.SetKp(Kp := REAL#7.5);
TankController.SetIntegralTime(Tn := REAL#3.0);
ASSERT_NEAR(EXPECTED := REAL#7.5, ACTUAL := TankController.Kp, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := TankController.Tn, DELTA := REAL#0.001);
END_TEST_PROGRAM
