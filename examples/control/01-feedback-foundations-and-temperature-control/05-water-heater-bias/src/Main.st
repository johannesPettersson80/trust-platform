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
    OperatingPointBias : LREAL := 75.0;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    ManualHeaterPower : LREAL := 0.0;
    RequestedHeaterPower : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
END_VAR
AutoMode := ExperimentEnabled AND InputsValid AND NOT SafetyAbort;

IF NOT Initialized THEN
    Parameters.ProportionalGain := ProportionalGain;
    Parameters.IntegralGain := LREAL#0.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    (* The 75% bias is the measured power that balances nominal heat loss at
       35 degC. It is feed-independent and therefore cannot reject a new load. *)
    Parameters.Bias := OperatingPointBias;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    (* Rate limiting is intentionally non-binding here. Lesson 9 introduces it
       as an independent actuator constraint. *)
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    (* Initialization mirrors the disabled safe state. Bias is introduced by
       the control law only after the learner explicitly enables Auto. *)
    InitialState.NominalOutput := LREAL#0.0;
    InitialState.SaturatedOutput := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
    RequestedHeaterPower := OperatingPointBias
        + ProportionalGain * (Setpoint - ProcessValue);
ELSE
    Input.Mode := PIDMode#Manual;
    RequestedHeaterPower := ManualHeaterPower;
END_IF;
(* Manual demand defaults to zero for restoration, but Control Studio can apply
   a bounded temporary demand while measuring the nominal holding output. *)
Input.ManualOutput := ManualHeaterPower;
Input.TrackingOutput := ManualHeaterPower;
Input.CycleTime := LREAL#0.1;
AppliedHeaterPower := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX40.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ProportionalGain AT %ML8 : LREAL;
    P1.OperatingPointBias AT %ML16 : LREAL;
    P1.RequestedHeaterPower AT %ML24 : LREAL;
    P1.ManualHeaterPower AT %ML32 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.ExperimentEnabled AT %MX40.1 : BOOL;
    P1.SafetyAbort AT %MX40.2 : BOOL;
    P1.InputsValid AT %MX40.3 : BOOL;
END_VAR
END_CONFIGURATION
