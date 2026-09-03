USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ProportionalGain : LREAL := 8.0;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    RequestedManipulatedValue : LREAL := 0.0;
    ManipulatedValue : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
END_VAR
AutoMode := ExperimentEnabled AND InputsValid AND NOT SafetyAbort;

IF NOT Initialized THEN
    (* Start from the declared 20 degC, zero-power plant state with the selected
       35 degC target and Kp 8 candidate ready but without automatic authority. *)
    Setpoint := LREAL#35.0;
    ProportionalGain := LREAL#8.0;
    RequestedManipulatedValue := LREAL#0.0;
    (* Enable only proportional action. The zero fixed output is deliberate:
       it lets the experiment reveal why this heater needs nonzero error to
       produce the power that balances heat loss. *)
    Parameters.IntegralGain := LREAL#0.0;
    Parameters.DerivativeGain := LREAL#0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0;
    Parameters.OutputMaximum := LREAL#100;
    (* This lesson isolates proportional action. At a 100 ms cycle, +/-1000 %/s
       permits the complete 0..100% output range in one update, so the later
       rate-limit lesson cannot silently alter the prediction made here. *)
    Parameters.RateMinimum := LREAL#-1000;
    Parameters.RateMaximum := LREAL#1000;
    Parameters.TrackingTime := LREAL#1;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Gain is a live lesson setting. Reconfigure before each update so a value
   entered in Control Studio takes effect on the next 100 ms scan. *)
Parameters.ProportionalGain := ProportionalGain;
Controller.Configure(Parameters := Parameters);
Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
    (* Keep the unclamped request visible so saturation is evidence, not an
       inference from the bounded actuator command. *)
    RequestedManipulatedValue := ProportionalGain * (Setpoint - ProcessValue);
ELSE
    Input.Mode := PIDMode#Manual;
    RequestedManipulatedValue := Input.ManualOutput;
END_IF;
(* CycleTime is part of the controller contract and matches MainTask's 100 ms
   interval. Later lessons vary timing explicitly; this lesson holds it fixed. *)
Input.CycleTime := LREAL#0.1;
ManipulatedValue := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX24.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ProportionalGain AT %ML8 : LREAL;
    P1.RequestedManipulatedValue AT %ML16 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.ExperimentEnabled AT %MX24.1 : BOOL;
    P1.SafetyAbort AT %MX24.2 : BOOL;
    P1.InputsValid AT %MX24.3 : BOOL;
END_VAR
END_CONFIGURATION
