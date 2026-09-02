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
    SelectedObjective : INT := 0;
    AutoMode : BOOL := FALSE;
    Setpoint : LREAL := 80.0;
    ProcessValue : LREAL := 80.0;
    ManipulatedValue : LREAL := 50.0;
    ManualOutput : LREAL := 50.0;
    ProportionalGainUsed : LREAL := 0.0;
    IntegralGainUsed : LREAL := 0.0;
    DerivativeGainUsed : LREAL := 0.0;
    ProportionalSetpointWeightUsed : LREAL := 0.0;
    BiasUsed : LREAL := 0.0;
    ConfiguredCycleTime : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    Setpoint := LREAL#80.0;
    ManualOutput := LREAL#50.0;
    AutoMode := FALSE;
    (* This deliberately regulatory-focused tuning rejects feed-temperature
       load promptly. With full proportional setpoint weight it also exposes
       the servo overshoot that Lesson 12 addresses without retuning PI. *)
    ProportionalGainUsed := LREAL#8.0;
    IntegralGainUsed := LREAL#2.0;
    DerivativeGainUsed := LREAL#0.0;
    ProportionalSetpointWeightUsed := LREAL#1.0;
    BiasUsed := LREAL#50.0;
    ConfiguredCycleTime := LREAL#0.1;
    Parameters.ProportionalGain := ProportionalGainUsed;
    Parameters.IntegralGain := IntegralGainUsed;
    Parameters.DerivativeGain := DerivativeGainUsed;
    Parameters.ProportionalSetpointWeight := ProportionalSetpointWeightUsed;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := BiasUsed;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-50.0;
    Parameters.RateMaximum := LREAL#50.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ManipulatedValue;
    InitialState.SaturatedOutput := ManipulatedValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Objective 0 is the shipped learner-visible servo run. The native evidence
   selects objective 1 in a fresh runtime for the regulatory comparison. Both
   use this project-owned timing and safe-restoration path. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    IF SelectedObjective = 0 THEN
        IF ExperimentCycle <= ULINT#50 THEN
            Setpoint := LREAL#80.0;
            ManualOutput := LREAL#50.0;
            AutoMode := TRUE;
        ELSIF ExperimentCycle <= ULINT#550 THEN
            Setpoint := LREAL#85.0;
            ManualOutput := LREAL#50.0;
            AutoMode := TRUE;
        ELSE
            Setpoint := LREAL#80.0;
            ManualOutput := LREAL#0.0;
            AutoMode := FALSE;
        END_IF;
    ELSE
        IF ExperimentCycle <= ULINT#1200 THEN
            Setpoint := LREAL#80.0;
            ManualOutput := LREAL#50.0;
            AutoMode := TRUE;
        ELSE
            Setpoint := LREAL#80.0;
            ManualOutput := LREAL#0.0;
            AutoMode := FALSE;
        END_IF;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    Setpoint := LREAL#80.0;
    ManualOutput := LREAL#0.0;
    AutoMode := FALSE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := ManualOutput;
Input.CycleTime := ConfiguredCycleTime;
ManipulatedValue := Controller.Update(Input := Input);
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManualOutput AT %ML8 : LREAL;
    P1.AutoMode AT %MX72.0 : BOOL;
    P1.SelectedObjective AT %MW64 : INT;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
