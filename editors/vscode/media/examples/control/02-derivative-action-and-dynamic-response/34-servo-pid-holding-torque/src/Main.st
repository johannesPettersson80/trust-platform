USING Control;

FUNCTION Lesson34GravityTorque : LREAL
VAR_INPUT
    PeakGravityTorque : LREAL;
    Position : LREAL;
END_VAR
Lesson34GravityTorque := PeakGravityTorque * SIN(Position);
END_FUNCTION

FUNCTION Lesson34ApproximatePdPosition : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    PeakGravityTorque : LREAL;
    Setpoint : LREAL;
END_VAR
Lesson34ApproximatePdPosition := ProportionalGain * Setpoint
    / (ProportionalGain + PeakGravityTorque);
END_FUNCTION

FUNCTION Lesson34IntegralGain : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    IntegralTime : LREAL;
END_VAR
Lesson34IntegralGain := ProportionalGain / IntegralTime;
END_FUNCTION

FUNCTION Lesson34RequiredIntegralState : LREAL
VAR_INPUT
    PeakGravityTorque : LREAL;
    Setpoint : LREAL;
    IntegralGain : LREAL;
END_VAR
Lesson34RequiredIntegralState := Lesson34GravityTorque(
    PeakGravityTorque := PeakGravityTorque,
    Position := Setpoint
) / IntegralGain;
END_FUNCTION

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
    IntegralGain : LREAL := 0.0;
    ManualHoldingTorque : LREAL := 0.0;
    Setpoint : LREAL := 0.0;
    Position : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    MotorTorque : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#16.0;
    (* Setting Ki to zero reproduces Lesson 20's PD baseline. A positive Ki
       learns the sustained payload torque that P and D cannot supply at zero
       position error. *)
    Parameters.IntegralGain := IntegralGain;
    Parameters.DerivativeGain := LREAL#7.0;
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
    InitialState.NominalOutput := MotorTorque;
    InitialState.SaturatedOutput := MotorTorque;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

ReconfigureRequested := FALSE;
IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    IntegralGain := Lesson34IntegralGain(
        ProportionalGain := LREAL#16.0,
        IntegralTime := LREAL#4.0
    );
    Setpoint := LREAL#0.5;
    AutoMode := TRUE;
    ReconfigureRequested := TRUE;
ELSIF ExperimentEnabled THEN
    Setpoint := LREAL#0.5;
    AutoMode := TRUE;
ELSIF ExperimentWasEnabled THEN
    IntegralGain := LREAL#0.0;
    Setpoint := LREAL#0.0;
    AutoMode := FALSE;
    ReconfigureRequested := TRUE;
END_IF;

InputsValid := Position >= LREAL#-4.0 AND Position <= LREAL#4.0
    AND Velocity >= LREAL#-20.0 AND Velocity <= LREAL#20.0;
IF ReconfigureRequested THEN
    Parameters.IntegralGain := IntegralGain;
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
    (* Gravity torque is zero at the restored zero-radian position in this
       simulation. A physical gravity-loaded axis needs a reviewed brake or support. *)
    Input.ManualOutput := ManualHoldingTorque;
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
    P1.AutoMode AT %MX32.0 : BOOL;
    P1.ExperimentEnabled AT %MX32.1 : BOOL;
    P1.SafetyAbort AT %MX32.2 : BOOL;
    P1.InputsValid AT %MX32.3 : BOOL;
    P1.ManualMode AT %MX32.4 : BOOL;
    P1.IntegralGain AT %ML16 : LREAL;
    P1.Setpoint AT %ML24 : LREAL;
    P1.Position AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.MotorTorque AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
