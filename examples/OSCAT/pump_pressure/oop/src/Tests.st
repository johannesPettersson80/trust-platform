TEST_PROGRAM PumpPressureOopComposeControllerAndAlarm
VAR
    PressureController : PidController;
    PressureAlarm : HysteresisSwitch;
    ClassicController : CTRL_PID;
    ClassicAlarm : HYST;
    ControllerLimits : RealRange;
    ControllerGains : PidGains;
    AlarmLimits : RealRange;
END_VAR

ControllerLimits.Low := REAL#0.0;
ControllerLimits.High := REAL#100.0;
ControllerGains.Kp := REAL#1.5;
ControllerGains.Tn := REAL#1.0;
ControllerGains.Tv := REAL#0.0;
PressureController.Initialize();
PressureController.Configure(
    Gains := ControllerGains,
    Limits := ControllerLimits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#60.0);

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);

ClassicController(
    ACT := REAL#12.0,
    SET_POINT := REAL#8.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#60.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#1.5,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
ClassicAlarm(In := REAL#12.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#3.0);

ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := PressureController.Update(Actual := REAL#12.0, Target := REAL#8.0), DELTA := REAL#0.001);
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := REAL#12.0) = ClassicAlarm.Q);
END_TEST_PROGRAM

(* Multi-pressure alarm-cycle test: drives the same alarm switch
   through high -> in-window -> low pressures. Verifies the alarm Q
   transitions instead of latching. *)
TEST_PROGRAM PumpPressureAlarmRespondsToReadingTransitions
VAR
    PressureAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);

(* High pressure -> trip *)
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := REAL#12.0));

(* In window -> retain, Window TRUE *)
PressureAlarm.Update(MeasuredValue := REAL#7.0);
ASSERT_TRUE(IN := PressureAlarm.Window);

(* Low pressure -> clear *)
ASSERT_FALSE(IN := PressureAlarm.Update(MeasuredValue := REAL#1.0));
END_TEST_PROGRAM

(* Manual mode: Update returns ManualInput and ControlError reflects
   (Target - Actual) regardless of mode. ManualInput is exposed as a
   property the operator faceplate can read. *)
TEST_PROGRAM PumpPressureControllerManualModeOutputsManualInput
VAR
    PressureController : PidController;
    ControllerLimits : RealRange;
    ControllerGains : PidGains;
    Output : REAL;
END_VAR

ControllerLimits.Low := REAL#0.0;
ControllerLimits.High := REAL#100.0;
ControllerGains.Kp := REAL#1.5;
ControllerGains.Tn := REAL#1.0;
ControllerGains.Tv := REAL#0.0;
PressureController.Initialize();
PressureController.Configure(
    Gains := ControllerGains,
    Limits := ControllerLimits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#75.0);
ASSERT_TRUE(IN := PressureController.Manual);
ASSERT_NEAR(EXPECTED := REAL#75.0, ACTUAL := PressureController.ManualInput, DELTA := REAL#0.001);

(* In manual mode the Update output equals the manual setpoint *)
Output := PressureController.Update(Actual := REAL#5.0, Target := REAL#8.0);
ASSERT_NEAR(EXPECTED := REAL#75.0, ACTUAL := Output, DELTA := REAL#0.001);

(* Operator changes manual input mid-flight; output follows *)
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#40.0);
Output := PressureController.Update(Actual := REAL#5.0, Target := REAL#8.0);
ASSERT_NEAR(EXPECTED := REAL#40.0, ACTUAL := Output, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Configuration validation handshake: PidController.Configure with
   inverted limits raises Error / InvalidConfiguration. *)
TEST_PROGRAM PumpPressureControllerRejectsInvertedLimits
VAR
    PressureController : PidController;
    InvertedLimits : RealRange;
    GoodLimits : RealRange;
    ControllerGains : PidGains;
    Status : ComponentStatus;
END_VAR

InvertedLimits.Low := REAL#100.0;
InvertedLimits.High := REAL#0.0;
GoodLimits.Low := REAL#0.0;
GoodLimits.High := REAL#100.0;
ControllerGains.Kp := REAL#1.5;
ControllerGains.Tn := REAL#1.0;
ControllerGains.Tv := REAL#0.0;

PressureController.Initialize();
PressureController.Configure(Gains := ControllerGains, Limits := InvertedLimits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
Status := PressureController.Snapshot();
ASSERT_TRUE(IN := Status.Error);
ASSERT_EQUAL(EXPECTED := ComponentErrorInvalidConfiguration, ACTUAL := Status.ErrorId);

(* Apply correct limits via SetLimits - error must clear *)
PressureController.ClearError();
PressureController.SetLimits(Limits := GoodLimits);
Status := PressureController.Snapshot();
ASSERT_FALSE(IN := Status.Error);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := PressureController.HighLimit, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Two pumps are independent: separate controllers and separate
   alarms. Same input value drives different outputs because each
   loop is configured differently. *)
TEST_PROGRAM PumpPressureTwoPumpsAreIndependent
VAR
    PumpA : PidController;
    PumpB : PidController;
    AlarmA : HysteresisSwitch;
    AlarmB : HysteresisSwitch;
    LimitsA : RealRange;
    LimitsB : RealRange;
    GainsA : PidGains;
    GainsB : PidGains;
    AlarmRangeA : RealRange;
    AlarmRangeB : RealRange;
END_VAR

(* PumpA: 0..100 limit, manual at 30 *)
LimitsA.Low := REAL#0.0;
LimitsA.High := REAL#100.0;
GainsA.Kp := REAL#1.0;
GainsA.Tn := REAL#1.0;
GainsA.Tv := REAL#0.0;
PumpA.Initialize();
PumpA.Configure(Gains := GainsA, Limits := LimitsA, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
PumpA.SetManual(Manual := TRUE, ManualInput := REAL#30.0);

(* PumpB: 20..80 limit, manual at 70 *)
LimitsB.Low := REAL#20.0;
LimitsB.High := REAL#80.0;
GainsB.Kp := REAL#2.5;
GainsB.Tn := REAL#0.5;
GainsB.Tv := REAL#0.0;
PumpB.Initialize();
PumpB.Configure(Gains := GainsB, Limits := LimitsB, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
PumpB.SetManual(Manual := TRUE, ManualInput := REAL#70.0);

(* Per-pump configuration is preserved *)
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := PumpA.HighLimit, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#80.0, ACTUAL := PumpB.HighLimit, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := PumpA.Kp, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#2.5, ACTUAL := PumpB.Kp, DELTA := REAL#0.001);

(* Drive both with the same input - manual outputs differ *)
ASSERT_NEAR(EXPECTED := REAL#30.0, ACTUAL := PumpA.Update(Actual := REAL#5.0, Target := REAL#8.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#70.0, ACTUAL := PumpB.Update(Actual := REAL#5.0, Target := REAL#8.0), DELTA := REAL#0.001);

(* Independent alarms with different limits *)
AlarmRangeA.Low := REAL#3.0;
AlarmRangeA.High := REAL#10.0;
AlarmRangeB.Low := REAL#1.0;
AlarmRangeB.High := REAL#5.0;
AlarmA.Initialize();
AlarmB.Initialize();
AlarmA.SetLimits(Limits := AlarmRangeA);
AlarmB.SetLimits(Limits := AlarmRangeB);

(* Same 6.0 reading: A in window, B trips *)
AlarmA.Update(MeasuredValue := REAL#6.0);
AlarmB.Update(MeasuredValue := REAL#6.0);
ASSERT_FALSE(IN := AlarmA.Q);
ASSERT_TRUE(IN := AlarmA.Window);
ASSERT_TRUE(IN := AlarmB.Q);
END_TEST_PROGRAM

(* Limited property: when manual output is forced beyond the high
   limit, the Update output saturates at HighLimit and Limited is
   reported. The OOP wrapper exposes these via properties; the classic
   FB exposed them as positional outputs. *)
TEST_PROGRAM PumpPressureControllerSaturatesAtHighLimit
VAR
    PressureController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    Output : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#1.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
PressureController.Initialize();
PressureController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);

(* Manual output above HighLimit saturates at 100 *)
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#150.0);
Output := PressureController.Update(Actual := REAL#5.0, Target := REAL#8.0);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Output, DELTA := REAL#0.001);
ASSERT_TRUE(IN := PressureController.Limited);

(* Manual output below LowLimit saturates at 0 *)
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#-25.0);
Output := PressureController.Update(Actual := REAL#5.0, Target := REAL#8.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Output, DELTA := REAL#0.001);
ASSERT_TRUE(IN := PressureController.Limited);

(* Manual output inside the band - no saturation *)
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#50.0);
Output := PressureController.Update(Actual := REAL#5.0, Target := REAL#8.0);
ASSERT_NEAR(EXPECTED := REAL#50.0, ACTUAL := Output, DELTA := REAL#0.001);
ASSERT_FALSE(IN := PressureController.Limited);
END_TEST_PROGRAM
