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
    LastAutoMode : BOOL := FALSE;
    ProportionalGain : LREAL := 8.0;
    IntegralGain : LREAL := 4.0;
    Setpoint : LREAL := 35.0;
    ProcessValue : LREAL := 20.0;
    ManualHeaterPower : LREAL := 0.0;
    RequestedHeaterPower : LREAL := 0.0;
    ProportionalIncrement : LREAL := 0.0;
    IntegralIncrement : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
    SaturationError : LREAL := 0.0;
    PreviousSetpoint : LREAL := 35.0;
    PreviousProcessValue : LREAL := 20.0;
END_VAR

IF NOT Initialized THEN
    (* These operator-facing values are mapped into the process image. Set
       their lesson defaults explicitly because a real target may start mapped
       memory at zero instead of applying declaration initializers. *)
    Setpoint := LREAL#35.0;
    AutoMode := FALSE;
    ProportionalGain := LREAL#8.0;
    IntegralGain := LREAL#4.0;
    ManualHeaterPower := LREAL#0.0;
    (* Lesson 8 deliberately lets the PID retain its unconstrained request.
       The physical clamp below is hidden from the controller so windup can be
       observed before Lesson 10 introduces anti-windup. *)
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-1000000.0;
    Parameters.OutputMaximum := LREAL#1000000.0;
    Parameters.RateMinimum := LREAL#-1000000.0;
    Parameters.RateMaximum := LREAL#1000000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    PreviousSetpoint := Setpoint;
    PreviousProcessValue := ProcessValue;
    Initialized := TRUE;
END_IF;

(* Control Studio writes one reviewed authority. The project then owns every
   preparation, exposure, recovery, and safe-restoration transition so the
   shipped Test executes the same sequence as the retained native evidence. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    IF ExperimentCycle = ULINT#1 THEN
        Setpoint := LREAL#35.0;
        ManualHeaterPower := LREAL#0.0;
        AutoMode := FALSE;
    ELSIF ExperimentCycle <= ULINT#500 THEN
        Setpoint := LREAL#35.0;
        ManualHeaterPower := LREAL#75.0;
        AutoMode := FALSE;
    ELSIF ExperimentCycle <= ULINT#550 THEN
        Setpoint := LREAL#35.0;
        ManualHeaterPower := LREAL#75.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#650 THEN
        Setpoint := LREAL#45.0;
        ManualHeaterPower := LREAL#75.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#950 THEN
        Setpoint := LREAL#35.0;
        ManualHeaterPower := LREAL#75.0;
        AutoMode := TRUE;
    ELSE
        Setpoint := LREAL#35.0;
        ManualHeaterPower := LREAL#0.0;
        AutoMode := FALSE;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    Setpoint := LREAL#35.0;
    ManualHeaterPower := LREAL#0.0;
    AutoMode := FALSE;
END_IF;

(* Control Studio gain edits reach both the diagnostics and PID at this scan
   boundary without clearing the integral state. *)
Parameters.ProportionalGain := ProportionalGain;
Parameters.IntegralGain := IntegralGain;
Controller.Configure(Parameters := Parameters);

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
Input.ManualOutput := ManualHeaterPower;
IF AutoMode THEN
    IF NOT LastAutoMode THEN
        (* Start Auto from the measured Manual holding command. This avoids
           confusing saturation with a mode-transfer bump. *)
        InitialState.Setpoint := PreviousSetpoint;
        InitialState.ProcessValue := PreviousProcessValue;
        InitialState.NominalOutput := ManualHeaterPower;
        InitialState.SaturatedOutput := ManualHeaterPower;
        Controller.Initialize(State := InitialState);
    END_IF;
    ProportionalIncrement := ProportionalGain
        * ((Setpoint - PreviousSetpoint) - (ProcessValue - PreviousProcessValue));
    IntegralIncrement := IntegralGain * (Setpoint - ProcessValue) * LREAL#0.1;
    Input.Mode := PIDMode#Auto;
ELSE
    ProportionalIncrement := LREAL#0.0;
    IntegralIncrement := LREAL#0.0;
    Input.Mode := PIDMode#Manual;
END_IF;
Input.CycleTime := LREAL#0.1;
RequestedHeaterPower := Controller.Update(Input := Input);
AppliedHeaterPower := MAX(MIN(RequestedHeaterPower, LREAL#100.0), LREAL#0.0);
SaturationError := RequestedHeaterPower - AppliedHeaterPower;
PreviousSetpoint := Setpoint;
PreviousProcessValue := ProcessValue;
LastAutoMode := AutoMode;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX64.0 : BOOL;
    P1.ExperimentEnabled AT %MX72.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.RequestedHeaterPower AT %ML8 : LREAL;
    P1.SaturationError AT %ML16 : LREAL;
    P1.ProportionalGain AT %ML24 : LREAL;
    P1.IntegralGain AT %ML32 : LREAL;
    P1.ManualHeaterPower AT %ML40 : LREAL;
    P1.ProportionalIncrement AT %ML48 : LREAL;
    P1.IntegralIncrement AT %ML56 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
