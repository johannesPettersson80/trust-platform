USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    PreviousAutoMode : BOOL := FALSE;
    IntegralGain : LREAL := 0.5;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 35.0;
    ManualHeaterPower : LREAL := 0.0;
    MeasuredHoldingOutput : LREAL := 75.0;
    Error : LREAL := 0.0;
    IntegralIncrement : LREAL := 0.0;
    IntegralContribution : LREAL := 0.0;
    RequestedHeaterPower : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    (* Lesson 5 established this measured nominal holding output. Keep it
       visible as inherited lesson data while the safe Manual command is zero. *)
    MeasuredHoldingOutput := LREAL#75.0;
    Parameters.ProportionalGain := LREAL#0.0;
    Parameters.IntegralGain := IntegralGain;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
IF AutoMode THEN
    IF NOT PreviousAutoMode THEN
        (* Preload the integral state with the measured holding command so the
           Manual-to-Auto transfer does not discard the operating point. *)
        InitialState.Setpoint := Setpoint;
        InitialState.ProcessValue := ProcessValue;
        InitialState.NominalOutput := MeasuredHoldingOutput;
        InitialState.SaturatedOutput := MeasuredHoldingOutput;
        Controller.Initialize(State := InitialState);
    END_IF;
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.ManualOutput := ManualHeaterPower;
Input.CycleTime := LREAL#0.1;
Error := Setpoint - ProcessValue;
IntegralIncrement := IntegralGain * Error * Input.CycleTime;

(* The displayed request is the next integral-only command before actuator
   limits. In Manual it is the operator's bounded holding command. *)
IF AutoMode THEN
    IF NOT PreviousAutoMode THEN
        RequestedHeaterPower := MeasuredHoldingOutput + IntegralIncrement;
    ELSE
        RequestedHeaterPower := AppliedHeaterPower + IntegralIncrement;
    END_IF;
ELSE
    RequestedHeaterPower := ManualHeaterPower;
END_IF;

AppliedHeaterPower := Controller.Update(Input := Input);
IF AutoMode THEN
    (* With P, D, and bias disabled, the bounded stored output is the integral
       contribution that remains when error returns to zero. *)
    IntegralContribution := AppliedHeaterPower;
ELSE
    IntegralContribution := ManualHeaterPower;
END_IF;
PreviousAutoMode := AutoMode;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.IntegralGain AT %ML8 : LREAL;
    P1.ManualHeaterPower AT %ML16 : LREAL;
    P1.RequestedHeaterPower AT %ML24 : LREAL;
    P1.IntegralContribution AT %ML32 : LREAL;
    P1.MeasuredHoldingOutput AT %ML40 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
