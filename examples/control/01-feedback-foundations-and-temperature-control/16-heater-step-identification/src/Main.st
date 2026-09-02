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
    ExperimentCycle : UDINT := 0;
    SafetyAbort : BOOL := FALSE;
    ExperimentInput : LREAL := 20.0;
    Setpoint : LREAL := 30.0;
    ProcessValue : LREAL := 30.0;
    ManipulatedValue : LREAL := 20.0;
    InputsValid : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    (* Addressed simulator memory begins at zero. Establish the declared
       30 degC / 20% Manual operating point before initializing the PID. *)
    ExperimentInput := LREAL#20.0;
    Setpoint := LREAL#30.0;
    AutoMode := FALSE;
    (* The PID instance is kept in Manual so this lesson exercises the same
       controller-facing MV path used by a commissioned loop. Identification
       must never obtain actuator authority through a private simulation path. *)
    Parameters.ProportionalGain := LREAL#1.0;
    Parameters.IntegralGain := LREAL#0.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := ExperimentInput;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-100.0;
    Parameters.RateMaximum := LREAL#100.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ExperimentInput;
    InitialState.SaturatedOutput := ExperimentInput;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Control Studio writes one reviewed Boolean. The project owns the complete
   timing so the interactive Test and native evidence execute the same event. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := UDINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + UDINT#1;
    IF ExperimentCycle <= UDINT#100 THEN
        ExperimentInput := LREAL#20.0;
    ELSE
        ExperimentInput := LREAL#30.0;
    END_IF;
ELSE
    ExperimentCycle := UDINT#0;
    ExperimentInput := LREAL#20.0;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
(* Control Studio interprets FALSE as Manual and TRUE as Auto. Keep this
   explicit signal false so the reviewed output-step contract can verify mode. *)
AutoMode := FALSE;
InputsValid := ProcessValue >= LREAL#0.0
    AND ProcessValue <= LREAL#40.0
    AND ExperimentInput >= LREAL#0.0
    AND ExperimentInput <= LREAL#100.0;
IF SafetyAbort OR NOT InputsValid THEN
    Input.ManualOutput := LREAL#0.0;
ELSE
    Input.ManualOutput := ExperimentInput;
END_IF;
Input.Mode := PIDMode#Manual;
Input.CycleTime := LREAL#0.1;
(* Control Studio owns qualification, approval, recording, and model fitting;
   this program only applies the reviewed bounded manual experiment command. *)
ManipulatedValue := Controller.Update(Input := Input);
IF SafetyAbort OR NOT InputsValid THEN
    (* Invalid finite inputs bypass controller rate handling so the physical
       command reaches the declared safe state in the same scan. *)
    ManipulatedValue := LREAL#0.0;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
    P1.ExperimentInput AT %ML0 : LREAL;
    P1.Setpoint AT %ML8 : LREAL;
    P1.SafetyAbort AT %MX16.1 : BOOL;
    P1.InputsValid AT %MX16.2 : BOOL;
    P1.AutoMode AT %MX16.3 : BOOL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
