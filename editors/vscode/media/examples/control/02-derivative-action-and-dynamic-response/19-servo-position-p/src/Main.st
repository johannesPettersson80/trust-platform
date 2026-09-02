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
    AutoMode : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ManualMode : BOOL := TRUE;
    ReconfigureRequested : BOOL := FALSE;
    ProportionalGain : LREAL := 2.0;
    Setpoint : LREAL := 0.0;
    Position : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    MotorTorque : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    (* With Ki=Kd=0, proportional position error acts like a virtual spring.
       Increasing Kp raises stiffness but adds no mechanical damping. *)
    IF ProportionalGain <= LREAL#0.0 THEN
        ProportionalGain := LREAL#2.0;
    END_IF;
    Parameters.ProportionalGain := ProportionalGain;
    Parameters.IntegralGain := LREAL#0.0;
    Parameters.DerivativeGain := LREAL#0.0;
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
    InitialState.ProcessValue := Position;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

(* Control Studio writes one reviewed Boolean. Its rising edge owns the
   selected Kp=8 Simulation run. Removing authority restores the Kp=2
   baseline, zero setpoint, and Manual safe torque. *)
ReconfigureRequested := FALSE;
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    ProportionalGain := LREAL#8.0;
    Setpoint := LREAL#1.0;
    AutoMode := TRUE;
    ReconfigureRequested := TRUE;
ELSIF ExperimentEnabled THEN
    Setpoint := LREAL#1.0;
    AutoMode := TRUE;
ELSIF ExperimentWasEnabled THEN
    ProportionalGain := LREAL#2.0;
    Setpoint := LREAL#0.0;
    AutoMode := FALSE;
    ReconfigureRequested := TRUE;
END_IF;

InputsValid := Position >= LREAL#-4.0 AND Position <= LREAL#4.0
    AND Velocity >= LREAL#-20.0 AND Velocity <= LREAL#20.0;

IF ReconfigureRequested THEN
    Parameters.ProportionalGain := ProportionalGain;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := Position;
    InitialState.NominalOutput := MotorTorque;
    InitialState.SaturatedOutput := MotorTorque;
    Controller.Initialize(State := InitialState);
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := Position;
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
IF SafetyAbort OR NOT InputsValid OR NOT AutoMode THEN
    MotorTorque := LREAL#0.0;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX32.1 : BOOL;
    P1.AutoMode AT %MX32.0 : BOOL;
    P1.SafetyAbort AT %MX32.2 : BOOL;
    P1.InputsValid AT %MX32.3 : BOOL;
    P1.ManualMode AT %MX32.4 : BOOL;
    P1.ProportionalGain AT %ML16 : LREAL;
    P1.Setpoint AT %ML24 : LREAL;
    P1.Position AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.MotorTorque AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
