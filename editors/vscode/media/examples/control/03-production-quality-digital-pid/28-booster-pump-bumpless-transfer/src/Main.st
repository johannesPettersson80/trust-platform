USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
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
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.InitializeRequest AT %MX48.0 : BOOL;
    P1.ControllerReady AT %MX48.1 : BOOL;
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
