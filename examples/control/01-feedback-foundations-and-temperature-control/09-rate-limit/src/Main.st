USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    AutoMode : BOOL := FALSE;
    Setpoint : LREAL := 0.0;
    ProcessValue : LREAL := 0.0;
    RequestedValvePosition : LREAL := 0.0;
    AppliedValvePosition : LREAL := 0.0;
    PreviousAppliedValvePosition : LREAL := 0.0;
    PreviousPositionUsedForUpdate : LREAL := 0.0;
    MeasuredTravelRate : LREAL := 0.0;
    MaximumOpeningRate : LREAL := 1.5;
    MaximumClosingRate : LREAL := 0.5;
    ConfiguredCycleTime : LREAL := 0.1;
    LowerAdmissiblePosition : LREAL := 0.0;
    UpperAdmissiblePosition : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    (* Establish mapped commissioning values before the first calculation.
       Otherwise a zero task interval would make the measured travel-rate
       calculation invalid on targets that zero their process image. *)
    Setpoint := LREAL#0.0;
    AutoMode := FALSE;
    MaximumOpeningRate := LREAL#1.5;
    MaximumClosingRate := LREAL#0.5;
    ConfiguredCycleTime := LREAL#0.1;
    (* The PID sees the valve's amplitude range but not its motor speed. Keeping
       the unconstrained request lets the learner observe the rate-limit gap
       that Lesson 10 must feed back into anti-windup. *)
    Parameters.ProportionalGain := LREAL#0.667;
    Parameters.IntegralGain := LREAL#0.667;
    Parameters.DerivativeGain := LREAL#0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000000.0;
    Parameters.RateMaximum := LREAL#1000000.0;
    Parameters.TrackingTime := LREAL#1;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* One reviewed Test authority drives the same opening, closing, and safe
   sequence retained by the native evidence. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    IF ExperimentCycle = ULINT#1 THEN
        Setpoint := LREAL#0.0;
        AutoMode := FALSE;
    ELSIF ExperimentCycle <= ULINT#151 THEN
        Setpoint := LREAL#10.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#900 THEN
        Setpoint := LREAL#0.0;
        AutoMode := TRUE;
    ELSE
        Setpoint := LREAL#0.0;
        AutoMode := FALSE;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    Setpoint := LREAL#0.0;
    AutoMode := FALSE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
Input.ManualOutput := LREAL#0.0;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.CycleTime := ConfiguredCycleTime;
RequestedValvePosition := Controller.Update(Input := Input);

(* A motor-operated valve may close more slowly than it opens. Deriving the
   admissible interval from the configured task interval expresses its motion
   in percent per second. Lesson 14 adds measured timing and jitter. *)
PreviousPositionUsedForUpdate := PreviousAppliedValvePosition;
LowerAdmissiblePosition := MAX(
    LREAL#0.0,
    PreviousPositionUsedForUpdate - MaximumClosingRate * ConfiguredCycleTime
);
UpperAdmissiblePosition := MIN(
    LREAL#100.0,
    PreviousPositionUsedForUpdate + MaximumOpeningRate * ConfiguredCycleTime
);
AppliedValvePosition := MAX(
    MIN(RequestedValvePosition, UpperAdmissiblePosition),
    LowerAdmissiblePosition
);
MeasuredTravelRate :=
    (AppliedValvePosition - PreviousPositionUsedForUpdate) / ConfiguredCycleTime;
PreviousAppliedValvePosition := AppliedValvePosition;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX72.0 : BOOL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.RequestedValvePosition AT %ML8 : LREAL;
    P1.MeasuredTravelRate AT %ML16 : LREAL;
    P1.MaximumOpeningRate AT %ML24 : LREAL;
    P1.MaximumClosingRate AT %ML32 : LREAL;
    P1.ConfiguredCycleTime AT %ML40 : LREAL;
    P1.PreviousPositionUsedForUpdate AT %ML48 : LREAL;
    P1.LowerAdmissiblePosition AT %ML56 : LREAL;
    P1.UpperAdmissiblePosition AT %ML64 : LREAL;
    P1.AppliedValvePosition AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
