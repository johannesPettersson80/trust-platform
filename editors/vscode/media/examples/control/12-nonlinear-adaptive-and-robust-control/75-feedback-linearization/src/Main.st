USING Control;

FUNCTION Lesson75ReviewedSetpoint : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ControlAuthority : BOOL;
END_VAR
IF NOT ControlAuthority OR (ExperimentElapsedSeconds < LREAL#1.0) THEN
    Lesson75ReviewedSetpoint := LREAL#0.0;
ELSIF ExperimentElapsedSeconds < LREAL#11.0 THEN
    Lesson75ReviewedSetpoint := LREAL#1.2;
ELSIF ExperimentElapsedSeconds < LREAL#16.0 THEN
    Lesson75ReviewedSetpoint := LREAL#0.0;
ELSIF ExperimentElapsedSeconds < LREAL#26.0 THEN
    Lesson75ReviewedSetpoint := LREAL#-1.2;
ELSE
    Lesson75ReviewedSetpoint := LREAL#0.0;
END_IF;
END_FUNCTION

FUNCTION Lesson75ScheduledProportionalGain : LREAL
VAR_INPUT
    SetpointAngle : LREAL;
END_VAR
VAR
    SchedulingAngle : LREAL;
END_VAR
SchedulingAngle := MIN(ABS(SetpointAngle), LREAL#1.2);
IF SchedulingAngle <= LREAL#0.6 THEN
    Lesson75ScheduledProportionalGain := LREAL#15.19
        + (LREAL#16.903457618 - LREAL#15.19)
            * SchedulingAngle / LREAL#0.6;
ELSE
    Lesson75ScheduledProportionalGain := LREAL#16.903457618
        + (LREAL#21.445270429 - LREAL#16.903457618)
            * (SchedulingAngle - LREAL#0.6) / LREAL#0.6;
END_IF;
END_FUNCTION

FUNCTION Lesson75GravityCompensation : LREAL
VAR_INPUT
    EstimatedGravityCoefficient : LREAL;
    JointAngle : LREAL;
END_VAR
Lesson75GravityCompensation := EstimatedGravityCoefficient * SIN(JointAngle);
END_FUNCTION

PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    ControllerMode : INT := 0;
    ActiveArchitecture : INT := 0;
    ModelEligible : BOOL := TRUE;
    FallbackActive : BOOL := FALSE;
    TorqueSaturated : BOOL := FALSE;
    ControllerInitialized : BOOL := FALSE;
    JointSetpoint : LREAL := 0.0;
    JointAngle : LREAL := 0.0;
    JointRate : LREAL := 0.0;
    SchedulingVariable : LREAL := 0.0;
    FixedProportionalGain : LREAL := 15.19;
    ScheduledProportionalGain : LREAL := 15.19;
    ActiveProportionalGain : LREAL := 15.19;
    EstimatedInertia : LREAL := 1.0;
    EstimatedGravityCoefficient : LREAL := 9.81;
    EstimatedViscousCoefficient : LREAL := 0.2;
    GravityCompensation : LREAL := 0.0;
    ModelCompensation : LREAL := 0.0;
    TorqueRequested : LREAL := 0.0;
    TorqueApplied : LREAL := 0.0;
    Controller : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
END_VAR

(* Control Studio grants only authority. The project owns the scheduled-PID
   wide-angle trajectory; other architectures and payloads stay native tests. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.02;
    Enabled := TRUE;
    ControllerMode := INT#1;
    JointSetpoint := Lesson75ReviewedSetpoint(
        ExperimentElapsedSeconds := ExperimentElapsed,
        ControlAuthority := TRUE);
ELSE
    ExperimentElapsed := LREAL#0.0;
    Enabled := FALSE;
    ControllerMode := INT#1;
    JointSetpoint := LREAL#0.0;
END_IF;

SchedulingVariable := MIN(ABS(JointSetpoint), LREAL#1.2);
ScheduledProportionalGain := Lesson75ScheduledProportionalGain(
    SetpointAngle := JointSetpoint);

ModelEligible := (EstimatedInertia >= LREAL#0.5)
    AND (EstimatedInertia <= LREAL#2.0)
    AND (EstimatedGravityCoefficient >= LREAL#0.0)
    AND (EstimatedGravityCoefficient <= LREAL#20.0)
    AND (EstimatedViscousCoefficient >= LREAL#0.0)
    AND (EstimatedViscousCoefficient <= LREAL#2.0);

FallbackActive := (ControllerMode = INT#2) AND NOT ModelEligible;
IF ControllerMode = INT#0 THEN
    ActiveArchitecture := INT#0;
ELSIF ControllerMode = INT#1 THEN
    ActiveArchitecture := INT#1;
ELSIF ModelEligible THEN
    ActiveArchitecture := INT#2;
ELSE
    ActiveArchitecture := INT#1;
END_IF;

// The nominal model term is observable even when another architecture is active.
GravityCompensation := Lesson75GravityCompensation(
    EstimatedGravityCoefficient := EstimatedGravityCoefficient,
    JointAngle := JointAngle);
ModelCompensation := GravityCompensation + EstimatedViscousCoefficient * JointRate;

Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-5000.0;
Parameters.RateMaximum := LREAL#5000.0;
Parameters.TrackingTime := LREAL#0.1;
Parameters.AntiWindup := PIDAntiWindup#Combined;

IF ActiveArchitecture = INT#0 THEN
    ActiveProportionalGain := FixedProportionalGain;
    Parameters.ProportionalGain := ActiveProportionalGain;
    Parameters.IntegralGain := LREAL#12.0;
    Parameters.DerivativeGain := LREAL#7.8;
    Input.FeedForward := LREAL#0.0;
ELSIF ActiveArchitecture = INT#1 THEN
    ActiveProportionalGain := ScheduledProportionalGain;
    Parameters.ProportionalGain := ActiveProportionalGain;
    Parameters.IntegralGain := LREAL#12.0;
    Parameters.DerivativeGain := LREAL#7.8;
    Input.FeedForward := LREAL#0.0;
ELSE
    // Keep the nominal 1.2-radian step below the motor limit so model
    // compensation retains actuator authority instead of starting saturated.
    ActiveProportionalGain := LREAL#24.0 * EstimatedInertia;
    Parameters.ProportionalGain := ActiveProportionalGain;
    Parameters.IntegralGain := LREAL#0.0;
    Parameters.DerivativeGain := LREAL#8.0 * EstimatedInertia;
    Input.FeedForward := ModelCompensation;
END_IF;
Controller.Configure(Parameters := Parameters);

IF NOT ControllerInitialized THEN
    InitialState.Setpoint := JointSetpoint;
    InitialState.ProcessValue := JointAngle;
    InitialState.ProcessValueRate := JointRate;
    InitialState.FeedForward := Input.FeedForward;
    InitialState.NominalOutput := LREAL#0.0;
    InitialState.SaturatedOutput := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    ControllerInitialized := TRUE;
END_IF;

Input.Setpoint := JointSetpoint;
Input.ProcessValue := JointAngle;
Input.CycleTime := LREAL#0.02;
IF Enabled THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
END_IF;
TorqueRequested := Controller.Update(Input := Input);
TorqueApplied := LIMIT(LREAL#-30.0, TorqueRequested, LREAL#30.0);
TorqueSaturated := Enabled AND ABS(TorqueRequested - TorqueApplied) > LREAL#1.0E-12;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ControllerMode AT %MW0 : INT;
    P1.ActiveArchitecture AT %MW2 : INT;
    P1.Enabled AT %MX4.0 : BOOL;
    P1.ModelEligible AT %MX4.1 : BOOL;
    P1.FallbackActive AT %MX4.2 : BOOL;
    P1.TorqueSaturated AT %MX4.3 : BOOL;
    P1.ExperimentEnabled AT %MX4.4 : BOOL;
    P1.SafetyAbort AT %MX4.5 : BOOL;
    P1.InputsValid AT %MX4.6 : BOOL;
    P1.JointSetpoint AT %ML8 : LREAL;
    P1.FixedProportionalGain AT %ML16 : LREAL;
    P1.ScheduledProportionalGain AT %ML24 : LREAL;
    P1.ActiveProportionalGain AT %ML32 : LREAL;
    P1.EstimatedInertia AT %ML40 : LREAL;
    P1.EstimatedGravityCoefficient AT %ML48 : LREAL;
    P1.EstimatedViscousCoefficient AT %ML56 : LREAL;
    P1.GravityCompensation AT %ML64 : LREAL;
    P1.TorqueRequested AT %ML72 : LREAL;
    P1.SchedulingVariable AT %ML80 : LREAL;
    P1.ModelCompensation AT %ML88 : LREAL;
    P1.TorqueApplied AT %QL0 : LREAL;
    P1.JointAngle AT %IL0 : LREAL;
    P1.JointRate AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
