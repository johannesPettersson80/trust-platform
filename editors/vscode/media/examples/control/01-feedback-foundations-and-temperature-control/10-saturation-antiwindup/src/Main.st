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
    ManipulatedValue : LREAL := 0.0;
    AntiWindupEnabled : BOOL := TRUE;
    AntiWindupStrategy : INT := -1;
    SelectedStrategy : INT := 2;
    RequestedOutput : LREAL := 0.0;
    TrackingTimeUsed : LREAL := 0.0;
    ProportionalGainUsed : LREAL := 0.0;
    IntegralGainUsed : LREAL := 0.0;
    DerivativeGainUsed : LREAL := 0.0;
    ConfiguredCycleTime : LREAL := 0.1;
END_VAR

IF NOT Initialized THEN
    AutoMode := FALSE;
    Setpoint := LREAL#0.0;
    ConfiguredCycleTime := LREAL#0.1;
    Parameters.ProportionalGain := LREAL#2.83;
    Parameters.IntegralGain := LREAL#0.8707692307692307;
    (* Derivative action is introduced in Lesson 20. Keeping Kd at zero here
       isolates anti-windup state handling in the PI controller. *)
    Parameters.DerivativeGain := LREAL#0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    (* Values 0..3 select none, signal clamping, back calculation, or combined.
       The -1 default preserves the earlier boolean exercise for saved projects. *)
    IF AntiWindupStrategy >= 0 AND AntiWindupStrategy <= 3 THEN
        SelectedStrategy := AntiWindupStrategy;
    ELSIF AntiWindupEnabled THEN
        SelectedStrategy := 3;
    ELSE
        SelectedStrategy := 0;
    END_IF;

    (* Strategy 0 hides the downstream physical clamp so windup remains the
       baseline. Every protected strategy gives the same real limits to PID. *)
    IF SelectedStrategy > 0 THEN
        Parameters.OutputMinimum := LREAL#-3.5;
        Parameters.OutputMaximum := LREAL#3.5;
        Parameters.RateMinimum := LREAL#-100;
        Parameters.RateMaximum := LREAL#100;
    ELSE
        Parameters.OutputMinimum := LREAL#-1000000;
        Parameters.OutputMaximum := LREAL#1000000;
        Parameters.RateMinimum := LREAL#-1000000;
        Parameters.RateMaximum := LREAL#1000000;
    END_IF;
    IF SelectedStrategy = 1 THEN
        (* Back calculation with Tt=dt moves retained demand directly to the
           saturated candidate and therefore implements signal clamping. *)
        Parameters.TrackingTime := LREAL#0.1;
        Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
    ELSIF SelectedStrategy = 2 THEN
        Parameters.TrackingTime := LREAL#3.25;
        Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
    ELSE
        Parameters.TrackingTime := LREAL#3.25;
        Parameters.AntiWindup := PIDAntiWindup#Combined;
    END_IF;
    TrackingTimeUsed := Parameters.TrackingTime;
    ProportionalGainUsed := Parameters.ProportionalGain;
    IntegralGainUsed := Parameters.IntegralGain;
    DerivativeGainUsed := Parameters.DerivativeGain;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Monitor retains the initial Manual-zero sample. One reviewed Test authority
   then drives the same baseline, unreachable request, recovery, and final
   Manual-zero sequence used by every fresh native comparison run. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    IF ExperimentCycle < ULINT#10 THEN
        Setpoint := LREAL#0.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle < ULINT#200 THEN
        Setpoint := LREAL#4.5;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#500 THEN
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
RequestedOutput := Controller.Update(Input := Input);
ManipulatedValue := MAX(MIN(RequestedOutput, LREAL#3.5), LREAL#-3.5);
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX64.0 : BOOL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.AntiWindupEnabled AT %MX64.1 : BOOL;
    P1.AntiWindupStrategy AT %MW40 : INT;
    P1.RequestedOutput AT %ML8 : LREAL;
    P1.TrackingTimeUsed AT %ML16 : LREAL;
    P1.ProportionalGainUsed AT %ML24 : LREAL;
    P1.IntegralGainUsed AT %ML32 : LREAL;
    P1.DerivativeGainUsed AT %ML48 : LREAL;
    P1.ConfiguredCycleTime AT %ML56 : LREAL;
END_VAR
END_CONFIGURATION
