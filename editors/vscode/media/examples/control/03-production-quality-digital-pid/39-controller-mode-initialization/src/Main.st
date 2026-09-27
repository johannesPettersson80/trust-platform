USING Control;

FUNCTION Lesson39EquilibriumPressure : LREAL
VAR_INPUT
    ZeroCommandPressure : LREAL;
    PressureGain : LREAL;
    ValveCommand : LREAL;
END_VAR
Lesson39EquilibriumPressure := ZeroCommandPressure
    + PressureGain * ValveCommand;
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    PressureFilter : Control.LowPass;
    FilterParameters : Control.LowPassParameters;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    InitializeRequest : BOOL := FALSE;
    UseMeasuredInitialization : BOOL := TRUE;
    RequireSettledFilter : BOOL := TRUE;
    RequestedMode : INT := 0;
    Setpoint : LREAL := 6.0;
    RawPressure : LREAL := 6.0;
    ActualValvePosition : LREAL := 40.0;
    ManualOutput : LREAL := 40.0;
    TrackingOutput : LREAL := 40.0;
    ValveCommand : LREAL := 30.0;
    FilteredPressure : LREAL := 6.0;
    ControllerReady : BOOL := FALSE;
    FilterSettled : BOOL := FALSE;
    StartupBlocked : BOOL := FALSE;
    ActiveAuthority : INT := 0;
    FilterSettledCount : INT := 0;
END_VAR

(* Control Studio writes one reviewed authority. Its rising edge produces one
   measured-state initialization request; project supervision owns the Manual
   settle interlock, Auto request, and Disabled safe restoration. *)
IF ExperimentEnabled THEN
    UseMeasuredInitialization := TRUE;
    RequireSettledFilter := TRUE;
    Setpoint := LREAL#6.0;
    ActualValvePosition := LREAL#40.0;
    ManualOutput := LREAL#40.0;
    TrackingOutput := LREAL#40.0;
    RequestedMode := 2;
    InitializeRequest := NOT ExperimentWasEnabled;
ELSIF ExperimentWasEnabled THEN
    InitializeRequest := FALSE;
    RequestedMode := 0;
END_IF;

IF InitializeRequest THEN
    Parameters.ProportionalGain := LREAL#15.0;
    Parameters.IntegralGain := LREAL#3.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-100.0;
    Parameters.RateMaximum := LREAL#100.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);

    FilterParameters.TimeConstant := LREAL#0.5;
    FilterParameters.Order := FilterOrder#SecondOrder;
    PressureFilter.Configure(Parameters := FilterParameters);

    IF UseMeasuredInitialization THEN
        (* A running station starts from measured pressure and the independent
           valve-position feedback, never from assumed zero histories. *)
        PressureFilter.Initialize(Input := RawPressure);
        FilteredPressure := RawPressure;
        InitialState.Setpoint := Setpoint;
        InitialState.ProcessValue := RawPressure;
        InitialState.NominalOutput := ActualValvePosition;
        InitialState.SaturatedOutput := ActualValvePosition;
    ELSE
        (* This deliberately unsafe baseline exists only to expose the startup
           transient produced by arbitrary zero controller and filter state. *)
        PressureFilter.Initialize(Input := LREAL#0.0);
        FilteredPressure := LREAL#0.0;
        InitialState.Setpoint := LREAL#0.0;
        InitialState.ProcessValue := LREAL#0.0;
        InitialState.NominalOutput := LREAL#0.0;
        InitialState.SaturatedOutput := LREAL#0.0;
    END_IF;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    FilterSettledCount := 0;
    FilterSettled := FALSE;
    ControllerReady := TRUE;
END_IF;

IF ControllerReady THEN
    FilteredPressure := PressureFilter.Update(
        Input := RawPressure,
        CycleTime := LREAL#0.1
    );
    IF ABS(FilteredPressure - RawPressure) <= LREAL#0.01 THEN
        IF FilterSettledCount < 3 THEN
            FilterSettledCount := FilterSettledCount + 1;
        END_IF;
    ELSE
        FilterSettledCount := 0;
    END_IF;
    FilterSettled := FilterSettledCount >= 3;

    Input.Setpoint := Setpoint;
    Input.ProcessValue := FilteredPressure;
    Input.CycleTime := LREAL#0.1;
END_IF;

StartupBlocked := FALSE;
InputsValid := RawPressure >= LREAL#0.0 AND RawPressure <= LREAL#12.0
    AND ActualValvePosition >= LREAL#0.0
    AND ActualValvePosition <= LREAL#100.0;
IF SafetyAbort OR NOT InputsValid THEN
    RequestedMode := 0;
END_IF;
CASE RequestedMode OF
    1:
        IF ControllerReady THEN
            Input.Mode := PIDMode#Manual;
            Input.ManualOutput := ManualOutput;
            ValveCommand := Controller.Update(Input := Input);
            ActiveAuthority := 1;
        ELSE
            ValveCommand := LREAL#30.0;
            ActiveAuthority := 0;
        END_IF;
    2:
        IF NOT ControllerReady THEN
            ValveCommand := LREAL#30.0;
            ActiveAuthority := 0;
            StartupBlocked := TRUE;
        ELSIF RequireSettledFilter AND NOT FilterSettled THEN
            (* While Auto is blocked, Manual retains the actual valve output
               and current signal histories without granting feedback authority. *)
            Input.Mode := PIDMode#Manual;
            Input.ManualOutput := ActualValvePosition;
            ValveCommand := Controller.Update(Input := Input);
            ActiveAuthority := 1;
            StartupBlocked := TRUE;
        ELSE
            Input.Mode := PIDMode#Auto;
            ValveCommand := Controller.Update(Input := Input);
            ActiveAuthority := 2;
        END_IF;
    3:
        IF ControllerReady THEN
            Input.Mode := PIDMode#Track;
            Input.TrackingOutput := TrackingOutput;
            ValveCommand := Controller.Update(Input := Input);
            ActiveAuthority := 3;
        ELSE
            ValveCommand := LREAL#30.0;
            ActiveAuthority := 0;
        END_IF;
ELSE
    (* Disabled belongs to station supervision, not Control.PID.PIDMode. The
       controller is not called and the declared safe recirculation command wins. *)
    ValveCommand := LREAL#30.0;
    ActiveAuthority := 0;
END_CASE;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InitializeRequest AT %MX40.0 : BOOL;
    P1.UseMeasuredInitialization AT %MX40.1 : BOOL;
    P1.RequireSettledFilter AT %MX40.2 : BOOL;
    P1.ExperimentEnabled AT %MX40.3 : BOOL;
    P1.SafetyAbort AT %MX40.4 : BOOL;
    P1.InputsValid AT %MX40.5 : BOOL;
    P1.ControllerReady AT %MX41.0 : BOOL;
    P1.FilterSettled AT %MX41.1 : BOOL;
    P1.StartupBlocked AT %MX41.2 : BOOL;
    P1.RequestedMode AT %MW42 : INT;
    P1.ActiveAuthority AT %MW44 : INT;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ActualValvePosition AT %ML8 : LREAL;
    P1.ManualOutput AT %ML16 : LREAL;
    P1.TrackingOutput AT %ML24 : LREAL;
    P1.FilteredPressure AT %ML32 : LREAL;
    P1.ValveCommand AT %QL0 : LREAL;
    P1.RawPressure AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
