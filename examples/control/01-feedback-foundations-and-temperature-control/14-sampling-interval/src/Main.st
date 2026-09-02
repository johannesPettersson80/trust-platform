USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    UseActualCycleTime : BOOL := TRUE;
    ResetTimingFault : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : DINT := 0;
    TimingCase : INT := 1;
    ActualCycleTime : LREAL := 0.1;
    ControllerCycleTime : LREAL := 0.1;
    MaximumCycleTime : LREAL := 0.25;
    TimingValid : BOOL := TRUE;
    TimingFaultLatched : BOOL := FALSE;
    BeltSpeed : LREAL := 0.5;
    Setpoint : LREAL := 0.0;
    ProcessValue : LREAL := 0.0;
    RequestedHeaterTrim : LREAL := 0.0;
    AppliedHeaterTrim : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    TimingFaultLatched := FALSE;
    Parameters.ProportionalGain := LREAL#0.667;
    Parameters.IntegralGain := LREAL#0.667;
    Parameters.DerivativeGain := LREAL#0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-10;
    Parameters.OutputMaximum := LREAL#10;
    Parameters.RateMinimum := LREAL#-100;
    Parameters.RateMaximum := LREAL#100;
    Parameters.TrackingTime := LREAL#1;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Control Studio changes only ExperimentEnabled. The project owns the selected
   measured-100-ms baseline, setpoint response, invalid-interval exposure,
   latched state, Manual reset, separate Auto re-enable, and safe ending. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := 0;
        TimingCase := 1;
        Setpoint := LREAL#0.0;
        ActualCycleTime := LREAL#0.1;
        MaximumCycleTime := LREAL#0.25;
        UseActualCycleTime := TRUE;
        ResetTimingFault := FALSE;
        AutoMode := FALSE;
    END_IF;

    ExperimentCycle := ExperimentCycle + 1;
    BeltSpeed := LREAL#0.5;
    MaximumCycleTime := LREAL#0.25;
    UseActualCycleTime := TRUE;
    ResetTimingFault := FALSE;

    IF ExperimentCycle <= 10 THEN
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#0.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= 300 THEN
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#1.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle = 301 THEN
        ActualCycleTime := LREAL#0.3;
        Setpoint := LREAL#1.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle = 302 THEN
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#1.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle = 303 THEN
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#1.0;
        AutoMode := FALSE;
        ResetTimingFault := TRUE;
    ELSIF ExperimentCycle = 304 THEN
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#1.0;
        AutoMode := FALSE;
    ELSIF ExperimentCycle = 305 THEN
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#1.0;
        AutoMode := TRUE;
    ELSE
        ActualCycleTime := LREAL#0.1;
        Setpoint := LREAL#0.0;
        AutoMode := FALSE;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := 0;
    ActualCycleTime := LREAL#0.1;
    Setpoint := LREAL#0.0;
    AutoMode := FALSE;
    ResetTimingFault := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;

Input.Setpoint := Setpoint;
Input.ProcessValue := ProcessValue;
Input.ManualOutput := LREAL#0.0;
(* Ordered bounds also reject nonfinite values. An invalid interval is a fault,
   not a value to clamp into the controller's reviewed timing envelope. *)
TimingValid := ActualCycleTime > LREAL#0.0
    AND ActualCycleTime <= MaximumCycleTime
    AND MaximumCycleTime > LREAL#0.0
    (* Configuration may tighten, but never widen, the reviewed 250 ms ceiling. *)
    AND MaximumCycleTime <= LREAL#0.25
    AND Setpoint >= LREAL#-5.0 AND Setpoint <= LREAL#5.0
    AND ProcessValue >= LREAL#-10.0 AND ProcessValue <= LREAL#10.0
    AND BeltSpeed >= LREAL#0.0 AND BeltSpeed <= LREAL#2.0;

IF TimingValid THEN
    IF UseActualCycleTime THEN
        ControllerCycleTime := ActualCycleTime;
    ELSE
        (* This selectable bad baseline demonstrates why a fixed nominal scan
           interval gives the wrong integral increment when elapsed time moves. *)
        ControllerCycleTime := LREAL#0.1;
    END_IF;
ELSE
    ControllerCycleTime := LREAL#0.0;
    TimingFaultLatched := TRUE;
END_IF;

(* Recovery is deliberate: first remove automatic authority, restore valid
   timing, then reset incremental state around the present safe operating point. *)
IF ResetTimingFault AND TimingValid AND NOT AutoMode THEN
    InitialState.Setpoint := Setpoint;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValue := ProcessValue;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    InitialState.NominalOutput := LREAL#0.0;
    InitialState.SaturatedOutput := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    TimingFaultLatched := FALSE;
END_IF;

IF TimingValid THEN
    IF NOT TimingFaultLatched THEN
        IF AutoMode THEN
            Input.Mode := PIDMode#Auto;
        ELSE
            Input.Mode := PIDMode#Manual;
        END_IF;
        Input.CycleTime := ControllerCycleTime;
        RequestedHeaterTrim := Controller.Update(Input := Input);
        (* The simulated power stage is ideal in this lesson, so request and
           applied trim agree at an update and the plant holds this value. *)
        AppliedHeaterTrim := RequestedHeaterTrim;
    ELSE
        RequestedHeaterTrim := LREAL#0.0;
        AppliedHeaterTrim := LREAL#0.0;
    END_IF;
ELSE
    RequestedHeaterTrim := LREAL#0.0;
    AppliedHeaterTrim := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX64.0 : BOOL;
    P1.UseActualCycleTime AT %MX64.1 : BOOL;
    P1.ResetTimingFault AT %MX64.4 : BOOL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
    P1.TimingCase AT %MW72 : INT;
    P1.ActualCycleTime AT %ML8 : LREAL;
    P1.ControllerCycleTime AT %ML24 : LREAL;
    P1.MaximumCycleTime AT %ML16 : LREAL;
    P1.TimingValid AT %MX64.2 : BOOL;
    P1.TimingFaultLatched AT %MX64.3 : BOOL;
    P1.BeltSpeed AT %IL16 : LREAL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.RequestedHeaterTrim AT %ML32 : LREAL;
    P1.AppliedHeaterTrim AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
