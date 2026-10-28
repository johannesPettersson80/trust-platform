USING Control;

(* One temperature reading: its value and the time it was acquired, in milliseconds on the controller's clock. *)
TYPE SensorReading :
STRUCT
    ValueC : LREAL;
    AcquiredMs : UDINT;
END_STRUCT
END_TYPE

(* The heater's rules for its two temperature sensors, A (primary) and B (monitor). A reading is valid if it is 0–80 °C and
   was acquired at most 500 ms before now, never after. With both valid, a difference of more than 0.4 °C is a disagreement;
   one that lasts 200 ms latches the fault, because a disagreement does not show which sensor is right. Otherwise A is used
   when valid and B when only B is. With no valid sensor the fault latches too. A latched fault clears only after three
   acquisitions in which both sensors are valid and agree, each counted once, followed by a new Auto request. Source is 0 for
   none, 1 for A and 2 for B; Switched marks the update in which the source changes, so the controller can realign. *)
FUNCTION_BLOCK SensorSelector
VAR_INPUT
    NowMs : UDINT;
    A : SensorReading;
    B : SensorReading;
    AutoRequest : BOOL;
END_VAR
VAR_OUTPUT
    SelectedC : LREAL := 0.0;
    Source : INT := 1;
    FaultLatched : BOOL := FALSE;
    DisagreementMs : UDINT := 0;
    Agreeing : UDINT := 0;
    Qualified : BOOL := FALSE;
    Switched : BOOL := FALSE;
END_VAR
VAR
    ValidA : BOOL := FALSE;
    ValidB : BOOL := FALSE;
    Excessive : BOOL := FALSE;
    Disagreeing : BOOL := FALSE;
    DisagreeSinceMs : UDINT := 0;
    CountedA : UDINT := 0;
    CountedB : UDINT := 0;
    AutoBefore : BOOL := FALSE;
    PreviousSource : INT := 1;
END_VAR

(* Both sensors expected, A selected, no fault. *)
METHOD PUBLIC Reset
Source := 1;
PreviousSource := 1;
FaultLatched := FALSE;
Disagreeing := FALSE;
DisagreementMs := UDINT#0;
Agreeing := UDINT#0;
Qualified := FALSE;
Switched := FALSE;
CountedA := UDINT#0;
CountedB := UDINT#0;
AutoBefore := FALSE;
END_METHOD

ValidA := A.ValueC >= LREAL#0.0 AND A.ValueC <= LREAL#80.0 AND A.AcquiredMs <= NowMs AND NowMs - A.AcquiredMs <= UDINT#500;
ValidB := B.ValueC >= LREAL#0.0 AND B.ValueC <= LREAL#80.0 AND B.AcquiredMs <= NowMs AND NowMs - B.AcquiredMs <= UDINT#500;
Excessive := ValidA AND ValidB AND ABS(A.ValueC - B.ValueC) > LREAL#0.4;
IF FaultLatched THEN
    IF ValidA AND ValidB AND NOT Excessive THEN
        IF A.AcquiredMs > CountedA AND B.AcquiredMs > CountedB THEN
            Agreeing := Agreeing + UDINT#1;
            CountedA := A.AcquiredMs;
            CountedB := B.AcquiredMs;
        END_IF;
    ELSE
        Agreeing := UDINT#0;
    END_IF;
    Qualified := Agreeing >= UDINT#3;
    IF Qualified AND AutoRequest AND NOT AutoBefore THEN
        FaultLatched := FALSE;
        Qualified := FALSE;
        Disagreeing := FALSE;
        DisagreementMs := UDINT#0;
    END_IF;
ELSE
    IF Excessive THEN
        IF NOT Disagreeing THEN
            Disagreeing := TRUE;
            DisagreeSinceMs := NowMs;
        END_IF;
        DisagreementMs := NowMs - DisagreeSinceMs;
        IF DisagreementMs >= UDINT#200 THEN
            FaultLatched := TRUE;
        END_IF;
    ELSE
        Disagreeing := FALSE;
        DisagreementMs := UDINT#0;
    END_IF;
    IF NOT ValidA AND NOT ValidB THEN
        FaultLatched := TRUE;
    END_IF;
    IF FaultLatched THEN
        Agreeing := UDINT#0;
        CountedA := A.AcquiredMs;
        CountedB := B.AcquiredMs;
    END_IF;
END_IF;
IF FaultLatched THEN
    Source := 0;
ELSIF ValidA THEN
    Source := 1;
    SelectedC := A.ValueC;
ELSE
    Source := 2;
    SelectedC := B.ValueC;
END_IF;
Switched := Source <> PreviousSource AND Source <> 0;
PreviousSource := Source;
AutoBefore := AutoRequest;
END_FUNCTION_BLOCK

(* What every heater needs from the experiment: its first update, whether it runs, a latched clock fault, the updates since
   the start (100 ms each) and the Auto request time, read only at the start. *)
TYPE ExperimentState :
STRUCT
    Start : BOOL;
    Running : BOOL;
    Faulted : BOOL;
    Updates : DINT;
    AutoRequestMs : UDINT;
END_STRUCT
END_TYPE

(* The results over the heaters that kept a valid measurement: passed cases, the highest temperature, the largest distance
   from 36 °C from 30 s, the largest command change at a change of sensor or restart, and whether every heater started at
   rest. *)
TYPE SensorResults :
STRUCT
    Passed : LREAL;
    HighestPeakC : LREAL;
    LargestStepErrorC : LREAL;
    LargestJumpPct : LREAL;
    StartedAtRest : BOOL;
END_STRUCT
END_TYPE

(* One heater, its two sensors and its Control.PID. The selector is the part to reuse; the sensors are simulated here.
   Sensors: both read the heater's temperature every 100 ms. After 5 s each case's sensors fail:
   0 stale: A stops acquiring at 5.0 s;
   1 range: A reads 85 °C from 5.0 s;
   2 disagreement: B reads 0.6 °C high from 5.0 s;
   3 short disagreements: B reads 0.6 °C high at 5.0, 5.1, 5.3 and 5.4 s only;
   4 both invalid: A reads 85 °C from 5.0 s and B stops acquiring at 5.0 s;
   5 restored: B reads 0.6 °C high from 5.0 to 9.9 s; the operator requests Auto at 8.0 s and at the experiment setting.
   Controller: Loop, a Control.PID, controls on the selected reading. When the source changes or control restarts after a
   latched fault, Loop is re-initialized with the applied command and the new reading, so the command does not jump; a latched
   fault commands zero heat. The record: the highest temperature from 0 s to 60 s, the largest distance from 36 °C from 30 s
   to 60 s, and the largest command change at a change of sensor or restart. At 60 s the case passes if it started at rest,
   never latched, peaked at most at 37 °C and stayed within 0.2 °C from 30 s. *)
FUNCTION_BLOCK SensedHeater
VAR_INPUT
    Run : ExperimentState;
    TemperatureC : LREAL;
    Scenario : INT;
END_VAR
VAR_IN_OUT
    Results : SensorResults;
END_VAR
VAR_OUTPUT
    CommandPct : LREAL := 75.0;
    Source : LREAL := 1.0;
    SelectedC : LREAL := 35.0;
    FaultLatched : BOOL := FALSE;
    DisagreementS : LREAL := 0.0;
    Agreeing : LREAL := 0.0;
    Qualified : BOOL := FALSE;
END_VAR
VAR
    Selector : SensorSelector;
    Loop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
        IntegralGain := LREAL#0.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    ReadingA : SensorReading;
    ReadingB : SensorReading;
    Update : UDINT := 0;
    NowMs : UDINT := 0;
    PreviousPct : LREAL := 75.0;
    WasLatched : BOOL := FALSE;
    EverLatched : BOOL := FALSE;
    PeakC : LREAL := 35.0;
    LateErrorC : LREAL := 0.0;
    JumpPct : LREAL := 0.0;
    AtRest : BOOL := FALSE;
    Passed : BOOL := FALSE;
END_VAR

(* Loop configured and initialized at the rest state (setpoint 35 °C, the current temperature, 75 %), both sensors reading
   the current temperature, and the selector reset. *)
METHOD PRIVATE ResetToRest
Loop.Configure(Parameters := Parameters);
State.Setpoint := LREAL#35.0;
State.ProcessValue := TemperatureC;
State.NominalOutput := LREAL#75.0;
State.SaturatedOutput := LREAL#75.0;
Loop.Initialize(State := State);
ReadingA.ValueC := TemperatureC;
ReadingA.AcquiredMs := UDINT#0;
ReadingB := ReadingA;
Selector.Reset();
END_METHOD

IF Run.Faulted THEN
    CommandPct := LREAL#0.0;
ELSIF Run.Running THEN
    IF Run.Start THEN
        ResetToRest();
        PreviousPct := LREAL#75.0;
        WasLatched := FALSE;
        EverLatched := FALSE;
        AtRest := ABS(TemperatureC - LREAL#35.0) <= LREAL#0.01;
        PeakC := TemperatureC;
        LateErrorC := LREAL#0.0;
        JumpPct := LREAL#0.0;
        Passed := FALSE;
    END_IF;
    Update := DINT_TO_UDINT(Run.Updates);
    NowMs := Update * UDINT#100;

    (* Sensors: each acquires the heater's temperature unless it has stopped acquiring, then the case's fault. *)
    IF NOT (Scenario = 0 AND Update > UDINT#50) THEN
        ReadingA.ValueC := TemperatureC;
        ReadingA.AcquiredMs := NowMs;
    END_IF;
    IF NOT (Scenario = 4 AND Update > UDINT#50) THEN
        ReadingB.ValueC := TemperatureC;
        ReadingB.AcquiredMs := NowMs;
    END_IF;
    CASE Scenario OF
        1, 4:
            IF Update >= UDINT#50 THEN
                ReadingA.ValueC := LREAL#85.0;
            END_IF;
        2:
            IF Update >= UDINT#50 THEN
                ReadingB.ValueC := TemperatureC + LREAL#0.6;
            END_IF;
        3:
            IF Update = UDINT#50 OR Update = UDINT#51 OR Update = UDINT#53 OR Update = UDINT#54 THEN
                ReadingB.ValueC := TemperatureC + LREAL#0.6;
            END_IF;
        5:
            IF Update >= UDINT#50 AND Update <= UDINT#99 THEN
                ReadingB.ValueC := TemperatureC + LREAL#0.6;
            END_IF;
    END_CASE;

    (* Selection and control. *)
    Selector(NowMs := NowMs, A := ReadingA, B := ReadingB,
        AutoRequest := Scenario = 5 AND (NowMs = UDINT#8000 OR NowMs = Run.AutoRequestMs));
    IF Selector.FaultLatched THEN
        CommandPct := LREAL#0.0;
        EverLatched := TRUE;
    ELSE
        IF Selector.Switched OR WasLatched THEN
            State.Setpoint := LREAL#36.0;
            State.ProcessValue := Selector.SelectedC;
            State.NominalOutput := PreviousPct;
            State.SaturatedOutput := PreviousPct;
            Loop.Initialize(State := State);
        END_IF;
        Input.Mode := PIDMode#Auto;
        Input.CycleTime := LREAL#0.1;
        Input.Setpoint := LREAL#36.0;
        Input.ProcessValue := Selector.SelectedC;
        CommandPct := Loop.Update(Input := Input);
        IF Selector.Switched OR WasLatched THEN
            JumpPct := MAX(JumpPct, ABS(CommandPct - PreviousPct));
        END_IF;
    END_IF;
    WasLatched := Selector.FaultLatched;
    PreviousPct := CommandPct;

    IF Run.Updates <= 600 THEN
        PeakC := MAX(PeakC, TemperatureC);
        IF Run.Updates >= 300 THEN
            LateErrorC := MAX(LateErrorC, ABS(LREAL#36.0 - TemperatureC));
        END_IF;
        IF Run.Updates = 600 THEN
            Passed := AtRest AND NOT EverLatched AND PeakC <= LREAL#37.0 AND LateErrorC <= LREAL#0.2;
        END_IF;
    END_IF;
ELSE
    (* At rest 75 % holds 35 °C, and the loop and both sensors wait at the rest state. *)
    ResetToRest();
    Selector(NowMs := UDINT#0, A := ReadingA, B := ReadingB, AutoRequest := FALSE);
    CommandPct := LREAL#75.0;
END_IF;
Source := INT_TO_LREAL(Selector.Source);
SelectedC := Selector.SelectedC;
FaultLatched := Selector.FaultLatched;
DisagreementS := UDINT_TO_LREAL(Selector.DisagreementMs) / LREAL#1000.0;
Agreeing := UDINT_TO_LREAL(Selector.Agreeing);
Qualified := Selector.Qualified;
IF NOT EverLatched THEN
    Results.HighestPeakC := MAX(Results.HighestPeakC, PeakC);
    Results.LargestStepErrorC := MAX(Results.LargestStepErrorC, LateErrorC);
    Results.LargestJumpPct := MAX(Results.LargestJumpPct, JumpPct);
END_IF;
IF Passed THEN
    Results.Passed := Results.Passed + LREAL#1.0;
END_IF;
Results.StartedAtRest := Results.StartedAtRest AND AtRest;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Six of Lesson 13's heaters, 10 kW/°C with 1 s delivery delay, each with two temperature sensors and a Control.PID. All
       take the setpoint step from 35 to 36 °C; each case's sensors fail after 5 s. An update interval outside
       100 ms ± 10 ms latches zero heat on every heater. *)
    StaleC, RangeC, DisagreementC, ShortC, BothC, RestoredC : LREAL := 35.0;
    StalePct, RangePct, DisagreementPct, ShortPct, BothPct, RestoredPct : LREAL := 75.0;
    (* When the restored case's operator requests Auto, a Control Studio experiment setting frozen at the start. *)
    AutoRequestTime : LREAL := 12.0;
    Cases : ARRAY[0..5] OF SensedHeater;
    Temperatures : ARRAY[0..5] OF LREAL;
    Commands : ARRAY[0..5] OF LREAL;
    K : DINT;
    Run : ExperimentState;
    Results : SensorResults;
    NoResults : SensorResults := (Passed := LREAL#0.0, HighestPeakC := LREAL#0.0, LargestStepErrorC := LREAL#0.0,
        LargestJumpPct := LREAL#0.0, StartedAtRest := TRUE);
    Updates : DINT := 0;
    (* The published results: the setpoint, the sources, timers and faults of the cases that show them, the restored case's
       agreeing readings, the stale case's selected reading, and the results over the heaters that kept a measurement. *)
    SetpointC : LREAL := 35.0;
    StaleSource : LREAL := 1.0;
    RangeSource : LREAL := 1.0;
    BothSource : LREAL := 1.0;
    DisagreementTimerS : LREAL := 0.0;
    ShortTimerS : LREAL := 0.0;
    RestoredAgreeing : LREAL := 0.0;
    StaleSelectedC : LREAL := 35.0;
    LargestJumpPct : LREAL := 0.0;
    Passed : LREAL := 0.0;
    HighestPeakC : LREAL := 35.0;
    LargestStepErrorC : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Start : BOOL := FALSE;
    StartedAtRest : BOOL := FALSE;
    ClockFault : BOOL := FALSE;
    DisagreementFault : BOOL := FALSE;
    BothFault : BOOL := FALSE;
    RestoredFault : BOOL := FALSE;
    RestoredQualified : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
NowTime := TIME();
IF ClockPrimed AND ((NowTime - PreviousTime) < T#90ms OR (NowTime - PreviousTime) > T#110ms) THEN
    ClockFault := TRUE;
END_IF;
Temperatures[0] := StaleC;
Temperatures[1] := RangeC;
Temperatures[2] := DisagreementC;
Temperatures[3] := ShortC;
Temperatures[4] := BothC;
Temperatures[5] := RestoredC;
Start := ExperimentEnabled AND NOT WasEnabled;
IF Start THEN
    Updates := 0;
    IF AutoRequestTime >= LREAL#0.0 AND AutoRequestTime <= LREAL#60.0 THEN
        Run.AutoRequestMs := LREAL_TO_UDINT(AutoRequestTime * LREAL#1000.0);
    ELSE
        Run.AutoRequestMs := UDINT#16#FFFFFFFF;
    END_IF;
ELSIF ExperimentEnabled THEN
    Updates := Updates + 1;
END_IF;
Run.Start := Start;
Run.Running := ExperimentEnabled AND NOT ClockFault;
Run.Faulted := ClockFault;
Run.Updates := Updates;
IF Run.Running THEN
    SetpointC := LREAL#36.0;
ELSE
    SetpointC := LREAL#35.0;
END_IF;
Results := NoResults;
FOR K := 0 TO 5 DO
    Cases[K](Run := Run, TemperatureC := Temperatures[K], Scenario := DINT_TO_INT(K), Results := Results,
        CommandPct => Commands[K]);
END_FOR;
StalePct := Commands[0];
RangePct := Commands[1];
DisagreementPct := Commands[2];
ShortPct := Commands[3];
BothPct := Commands[4];
RestoredPct := Commands[5];
StaleSource := Cases[0].Source;
RangeSource := Cases[1].Source;
BothSource := Cases[4].Source;
DisagreementTimerS := Cases[2].DisagreementS;
ShortTimerS := Cases[3].DisagreementS;
RestoredAgreeing := Cases[5].Agreeing;
StaleSelectedC := Cases[0].SelectedC;
DisagreementFault := Cases[2].FaultLatched;
BothFault := Cases[4].FaultLatched;
RestoredFault := Cases[5].FaultLatched;
RestoredQualified := Cases[5].Qualified;
Passed := Results.Passed;
LargestJumpPct := Results.LargestJumpPct;
HighestPeakC := Results.HighestPeakC;
LargestStepErrorC := Results.LargestStepErrorC;
StartedAtRest := Results.StartedAtRest;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.StaleC AT %IL0 : LREAL;
    P1.RangeC AT %IL8 : LREAL;
    P1.DisagreementC AT %IL16 : LREAL;
    P1.ShortC AT %IL24 : LREAL;
    P1.BothC AT %IL32 : LREAL;
    P1.RestoredC AT %IL40 : LREAL;
    P1.StalePct AT %QL0 : LREAL;
    P1.RangePct AT %QL8 : LREAL;
    P1.DisagreementPct AT %QL16 : LREAL;
    P1.ShortPct AT %QL24 : LREAL;
    P1.BothPct AT %QL32 : LREAL;
    P1.RestoredPct AT %QL40 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.StaleSource AT %ML8 : LREAL;
    P1.RangeSource AT %ML16 : LREAL;
    P1.BothSource AT %ML24 : LREAL;
    P1.DisagreementTimerS AT %ML32 : LREAL;
    P1.ShortTimerS AT %ML40 : LREAL;
    P1.RestoredAgreeing AT %ML48 : LREAL;
    P1.StaleSelectedC AT %ML56 : LREAL;
    P1.LargestJumpPct AT %ML64 : LREAL;
    P1.Passed AT %ML72 : LREAL;
    P1.HighestPeakC AT %ML80 : LREAL;
    P1.LargestStepErrorC AT %ML88 : LREAL;
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.StartedAtRest AT %MX96.1 : BOOL;
    P1.ClockFault AT %MX96.2 : BOOL;
    P1.DisagreementFault AT %MX96.3 : BOOL;
    P1.BothFault AT %MX96.4 : BOOL;
    P1.RestoredFault AT %MX96.5 : BOOL;
    P1.RestoredQualified AT %MX96.6 : BOOL;
END_VAR
END_CONFIGURATION
