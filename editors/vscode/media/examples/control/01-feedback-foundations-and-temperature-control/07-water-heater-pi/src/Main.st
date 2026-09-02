USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    LastAutoMode : BOOL := FALSE;
    ProportionalGain : LREAL := 8.0;
    IntegralGain : LREAL := 4.0;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    ManualHeaterPower : LREAL := 0.0;
    MeasuredHoldingOutput : LREAL := 75.0;
    RequestedHeaterPower : LREAL := 0.0;
    ProportionalIncrement : LREAL := 0.0;
    IntegralIncrement : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
    PreviousSetpoint : LREAL := 35.0;
    PreviousProcessValue : LREAL := 20.0;
END_VAR

IF NOT Initialized THEN
    MeasuredHoldingOutput := LREAL#75.0;
    (* Lesson 7 combines only the P and I actions already observed separately.
       Bias stays zero so the integral term must learn the holding power. *)
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
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    PreviousSetpoint := Setpoint;
    PreviousProcessValue := ProcessValue;
    Initialized := TRUE;
END_IF;

(* Control Studio gain edits are adopted together at the next scan boundary.
   Configure changes parameters without resetting the retained PI state. *)
Parameters.ProportionalGain := ProportionalGain;
Parameters.IntegralGain := IntegralGain;
Controller.Configure(Parameters := Parameters);

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
Input.ManualOutput := ManualHeaterPower;
IF AutoMode THEN
    IF NOT LastAutoMode THEN
        (* Seed PI from the measured Manual holding output so Auto starts
           from the same physical operating point without an output bump. *)
        InitialState.Setpoint := Setpoint;
        InitialState.ProcessValue := ProcessValue;
        InitialState.NominalOutput := MeasuredHoldingOutput;
        InitialState.SaturatedOutput := MeasuredHoldingOutput;
        Controller.Initialize(State := InitialState);
        PreviousSetpoint := Setpoint;
        PreviousProcessValue := ProcessValue;
    END_IF;
    ProportionalIncrement := ProportionalGain
        * ((Setpoint - PreviousSetpoint) - (ProcessValue - PreviousProcessValue));
    IntegralIncrement := IntegralGain * (Setpoint - ProcessValue) * LREAL#0.1;
    IF NOT LastAutoMode THEN
        RequestedHeaterPower := MeasuredHoldingOutput
            + ProportionalIncrement + IntegralIncrement;
    ELSE
        RequestedHeaterPower := AppliedHeaterPower
            + ProportionalIncrement + IntegralIncrement;
    END_IF;
    Input.Mode := PIDMode#Auto;
ELSE
    ProportionalIncrement := LREAL#0.0;
    IntegralIncrement := LREAL#0.0;
    RequestedHeaterPower := ManualHeaterPower;
    Input.Mode := PIDMode#Manual;
END_IF;
Input.CycleTime := LREAL#0.1;
AppliedHeaterPower := Controller.Update(Input := Input);
PreviousSetpoint := Setpoint;
PreviousProcessValue := ProcessValue;
LastAutoMode := AutoMode;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ProportionalGain AT %ML8 : LREAL;
    P1.IntegralGain AT %ML16 : LREAL;
    P1.ManualHeaterPower AT %ML24 : LREAL;
    P1.RequestedHeaterPower AT %ML32 : LREAL;
    P1.ProportionalIncrement AT %ML40 : LREAL;
    P1.IntegralIncrement AT %ML48 : LREAL;
    P1.MeasuredHoldingOutput AT %ML56 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
