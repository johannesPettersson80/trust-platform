USING Control;

FUNCTION Lesson40TransferStep : LREAL
VAR_INPUT
    FirstNewRequest : LREAL;
    LastAppliedOutput : LREAL;
END_VAR
Lesson40TransferStep := ABS(FirstNewRequest - LastAppliedOutput);
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    InitializeRequest : BOOL := FALSE;
    RequestedAuthority : INT := 0;
    Setpoint : LREAL := 6.0;
    ProcessValue : LREAL := 6.0;
    ActualValvePosition : LREAL := 40.0;
    ExternalOutput : LREAL := 40.0;
    ManualOutput : LREAL := 40.0;
    TrackingOutput : LREAL := 40.0;
    ValveCommand : LREAL := 30.0;
    ControllerReady : BOOL := FALSE;
    ActiveAuthority : INT := 0;
END_VAR

(* One reviewed Boolean owns the safe tracked handover. Exact untracked and
   Manual counterfactuals remain fresh-state native-test cases. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
        ExternalOutput := LREAL#40.0;
        TrackingOutput := LREAL#40.0;
        ActualValvePosition := LREAL#40.0;
        Setpoint := LREAL#6.0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    InitializeRequest := ExperimentCycle = ULINT#1;
    (* Follow current pressure during the isolated transfer so feedback error
       and its increment do not masquerade as an authority bump. *)
    Setpoint := ProcessValue;
    IF ExperimentCycle = ULINT#1 THEN
        RequestedAuthority := 3;
    ELSIF ExperimentCycle <= ULINT#20 THEN
        ExternalOutput := LREAL#60.0;
        (* This teaching simulator has nonbinding actuator limits, so its
           accepted external output is the applied-output proxy. Physical
           equipment must replace this assignment with fresh valve feedback. *)
        ActualValvePosition := ExternalOutput;
        TrackingOutput := ActualValvePosition;
        RequestedAuthority := 3;
    ELSE
        TrackingOutput := ActualValvePosition;
        RequestedAuthority := 2;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    InitializeRequest := FALSE;
    RequestedAuthority := 0;
    ExternalOutput := LREAL#40.0;
    ActualValvePosition := LREAL#40.0;
    TrackingOutput := LREAL#40.0;
    ManualOutput := LREAL#40.0;
    Setpoint := LREAL#6.0;
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
    (* Rate limiting is deliberately nonbinding in this handover lesson so an
       output step cannot be mistaken for a successfully aligned state. *)
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := ActualValvePosition;
    InitialState.SaturatedOutput := ActualValvePosition;
    Controller.Initialize(State := InitialState);
    ControllerReady := TRUE;
END_IF;

IF ControllerReady THEN
    Input.Setpoint := Setpoint;
    Input.ProcessValue := ProcessValue;
    Input.CycleTime := LREAL#0.1;
END_IF;

InputsValid := ProcessValue >= LREAL#0.0 AND ProcessValue <= LREAL#12.0
    AND ExternalOutput >= LREAL#0.0 AND ExternalOutput <= LREAL#100.0
    AND TrackingOutput >= LREAL#0.0 AND TrackingOutput <= LREAL#100.0;
IF SafetyAbort OR NOT InputsValid THEN
    RequestedAuthority := 0;
END_IF;
CASE RequestedAuthority OF
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
        IF ControllerReady THEN
            Input.Mode := PIDMode#Auto;
            ValveCommand := Controller.Update(Input := Input);
            ActiveAuthority := 2;
        ELSE
            ValveCommand := LREAL#30.0;
            ActiveAuthority := 0;
        END_IF;
    3:
        IF ControllerReady THEN
            (* TrackingOutput must be the independently observed applied valve
               value. A pre-limit request would align the wrong state. *)
            Input.Mode := PIDMode#Track;
            Input.TrackingOutput := TrackingOutput;
            ValveCommand := Controller.Update(Input := Input);
            ActiveAuthority := 3;
        ELSE
            ValveCommand := LREAL#30.0;
            ActiveAuthority := 0;
        END_IF;
    4:
        (* Simulation-only failure case: external authority moves the valve
           while the inactive controller receives no tracking updates. *)
        ValveCommand := ExternalOutput;
        ActiveAuthority := 4;
ELSE
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
    P1.InitializeRequest AT %MX48.0 : BOOL;
    P1.ControllerReady AT %MX48.1 : BOOL;
    P1.ExperimentEnabled AT %MX48.2 : BOOL;
    P1.SafetyAbort AT %MX48.3 : BOOL;
    P1.InputsValid AT %MX48.4 : BOOL;
    P1.RequestedAuthority AT %MW50 : INT;
    P1.ActiveAuthority AT %MW52 : INT;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ActualValvePosition AT %ML8 : LREAL;
    P1.ExternalOutput AT %ML16 : LREAL;
    P1.ManualOutput AT %ML24 : LREAL;
    P1.TrackingOutput AT %ML32 : LREAL;
    P1.ValveCommand AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
