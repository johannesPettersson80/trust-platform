USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    FirstFilter : Control.LowPass;
    SecondFilter : Control.LowPass;
    FirstParameters : Control.LowPassParameters;
    SecondParameters : Control.LowPassParameters;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    ReconfigureRequested : BOOL := FALSE;
    FilterSelection : INT := 0;
    FilterTimeConstant : LREAL := 0.05;
    Setpoint : LREAL := 0.0;
    RawPosition : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    ControllerPosition : LREAL := 0.0;
    MotorTorque : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    (* Both filter orders use the selected time constant. This lets the lesson
       compare cutoff first, then compare order at one fixed cutoff. *)
    FirstParameters.TimeConstant := FilterTimeConstant;
    FirstParameters.Order := FilterOrder#FirstOrder;
    FirstFilter.Configure(Parameters := FirstParameters);
    FirstFilter.Initialize(Input := RawPosition);
    SecondParameters.TimeConstant := FilterTimeConstant;
    SecondParameters.Order := FilterOrder#SecondOrder;
    SecondFilter.Configure(Parameters := SecondParameters);
    SecondFilter.Initialize(Input := RawPosition);

    Parameters.ProportionalGain := LREAL#8.0;
    Parameters.IntegralGain := LREAL#2.0;
    Parameters.DerivativeGain := LREAL#3.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-10.0;
    Parameters.OutputMaximum := LREAL#10.0;
    Parameters.RateMinimum := LREAL#-500.0;
    Parameters.RateMaximum := LREAL#500.0;
    Parameters.TrackingTime := LREAL#0.2;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := RawPosition;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Control Studio writes only this Boolean authority. The project owns the
   selected filter, setpoint, mode, and restoration so the reviewed Test and
   the executable sequence cannot disagree. *)
ReconfigureRequested := FALSE;
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    FilterSelection := 1;
    FilterTimeConstant := LREAL#0.05;
    AutoMode := TRUE;
    ReconfigureRequested := TRUE;
ELSIF ExperimentEnabled THEN
    AutoMode := TRUE;
ELSIF ExperimentWasEnabled THEN
    FilterSelection := 0;
    FilterTimeConstant := LREAL#0.05;
    Setpoint := LREAL#0.0;
    AutoMode := FALSE;
    ReconfigureRequested := TRUE;
END_IF;

InputsValid := RawPosition >= LREAL#-4.0 AND RawPosition <= LREAL#4.0
    AND Velocity >= LREAL#-20.0 AND Velocity <= LREAL#20.0;
IF ReconfigureRequested THEN
    FirstParameters.TimeConstant := FilterTimeConstant;
    FirstParameters.Order := FilterOrder#FirstOrder;
    FirstFilter.Configure(Parameters := FirstParameters);
    FirstFilter.Initialize(Input := RawPosition);
    SecondParameters.TimeConstant := FilterTimeConstant;
    SecondParameters.Order := FilterOrder#SecondOrder;
    SecondFilter.Configure(Parameters := SecondParameters);
    SecondFilter.Initialize(Input := RawPosition);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := RawPosition;
    InitialState.NominalOutput := MotorTorque;
    InitialState.SaturatedOutput := MotorTorque;
    Controller.Initialize(State := InitialState);
END_IF;
IF ExperimentEnabled THEN Setpoint := LREAL#0.5; END_IF;

IF FilterSelection = 1 THEN
    ControllerPosition := FirstFilter.Update(Input := RawPosition, CycleTime := LREAL#0.02);
ELSIF FilterSelection = 2 THEN
    ControllerPosition := SecondFilter.Update(Input := RawPosition, CycleTime := LREAL#0.02);
ELSE
    ControllerPosition := RawPosition;
END_IF;

Input.Setpoint := Setpoint;
(* The raw encoder remains visible; only this explicit controller-facing copy
   is filtered, preventing a smooth plot from hiding sensor evidence. *)
Input.ProcessValue := ControllerPosition;
IF AutoMode AND InputsValid AND NOT SafetyAbort THEN
    Input.Mode := PIDMode#Auto;
    ManualMode := FALSE;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    ManualMode := TRUE;
END_IF;
Input.CycleTime := LREAL#0.02;
MotorTorque := Controller.Update(Input := Input);
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.ExperimentEnabled AT %MX16.1 : BOOL;
    P1.SafetyAbort AT %MX16.2 : BOOL;
    P1.InputsValid AT %MX16.3 : BOOL;
    P1.ManualMode AT %MX16.4 : BOOL;
    P1.FilterSelection AT %MW32 : INT;
    P1.Setpoint AT %ML40 : LREAL;
    P1.FilterTimeConstant AT %ML48 : LREAL;
    P1.RawPosition AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.ControllerPosition AT %ML0 : LREAL;
    P1.MotorTorque AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
