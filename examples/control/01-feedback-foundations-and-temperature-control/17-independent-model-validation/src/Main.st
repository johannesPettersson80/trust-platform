USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentInput : LREAL := 20.0;
    TestAuthorityEnabled : BOOL := FALSE;
    TestCycle : UDINT := 0;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    Setpoint : LREAL := 31.0;
    ProcessValue : LREAL := 30.0;
    ManipulatedValue : LREAL := 20.0;
END_VAR

IF NOT Initialized THEN
    (* Keep the accepted Lesson 16 model frozen. This program applies only the
       independently approved MV record through the normal PID manual path. *)
    Parameters.ProportionalGain := LREAL#1.0;
    Parameters.IntegralGain := LREAL#0.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#20.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    (* This lesson tests the plant model, not an actuator slew limit. The wide
       bounds preserve the commanded 20-to-25-percent validation step. *)
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := LREAL#20.0;
    InitialState.SaturatedOutput := LREAL#20.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Test writes only Boolean authority. The project owns the exact baseline and
   validation step used by both the interactive journey and native evidence. *)
IF TestAuthorityEnabled THEN
    TestCycle := TestCycle + UDINT#1;
    IF TestCycle <= UDINT#100 THEN
        ExperimentInput := LREAL#20.0;
    ELSE
        ExperimentInput := LREAL#25.0;
    END_IF;
ELSE
    TestCycle := UDINT#0;
    ExperimentInput := LREAL#20.0;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
InputsValid := ProcessValue >= LREAL#0.0
    AND ProcessValue <= LREAL#40.0
    AND ExperimentInput >= LREAL#0.0
    AND ExperimentInput <= LREAL#100.0;
(* Validation remains in the ordinary Manual controller path. Test authority
   starts only the bounded project-owned request; abort and invalid data win. *)
IF SafetyAbort OR NOT InputsValid THEN
    Input.ManualOutput := LREAL#0.0;
ELSE
    Input.ManualOutput := ExperimentInput;
END_IF;
Input.Mode := PIDMode#Manual;
ManualMode := TRUE;
Input.CycleTime := LREAL#0.1;
(* Prediction and residual analysis belong to Control Studio so measured PV is
   never overwritten by the model that is being evaluated. *)
ManipulatedValue := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentInput AT %ML0 : LREAL;
    P1.Setpoint AT %ML8 : LREAL;
    P1.TestAuthorityEnabled AT %MX16.0 : BOOL;
    P1.SafetyAbort AT %MX16.1 : BOOL;
    P1.InputsValid AT %MX16.2 : BOOL;
    P1.ManualMode AT %MX16.3 : BOOL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
