USING Control;

(* TankController combines the local level models, gain scheduler, and PI controller used across the gravity-drained tank operating range. It produces a requested pump command; Main owns enable and physical output handling. *)
FUNCTION_BLOCK TankController
VAR
    PIDController : Control.PID;
    GainScheduler : Control.PIDGainScheduler;
    PIDInput : Control.PIDInput;
    FixedParameters : Control.PIDParameters;
    ActiveParameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    ScheduleParameters : Control.PIDGainScheduleParameters;
    ScheduledParameters : Control.PIDGainScheduleOutput;
    ScheduledControlEnabled : BOOL := FALSE;
    LastAppliedPumpFlow : LREAL := 0.027386127875;
    Configured : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    LevelSetpoint : LREAL := 0.30;
    MeasuredLevel : LREAL := 0.30;
    RequestedPumpFlow : LREAL := 0.027386127875;
    AppliedPumpFlow : LREAL := 0.027386127875;
    FilteredSchedulingLevel : LREAL := 0.30;
    InterpolationFraction : LREAL := 0.0;
    ActiveKp : LREAL := 0.100;
    ActiveKi : LREAL := 0.00280;
    ScheduleValid : BOOL := TRUE;
    ScheduleClamped : BOOL := FALSE;
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL := FALSE;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    ScheduledControl : BOOL;
    InitialLevel : LREAL;
    InitialPumpFlow : LREAL;
END_VAR

ScheduledControlEnabled := ScheduledControl;
FixedParameters.ProportionalGain := LREAL#0.100;
FixedParameters.IntegralGain := LREAL#0.00280;
FixedParameters.DerivativeGain := LREAL#0.0;
FixedParameters.ProportionalSetpointWeight := LREAL#1.0;
FixedParameters.DerivativeSetpointWeight := LREAL#0.0;
FixedParameters.Bias := LREAL#0.0;
FixedParameters.OutputMinimum := LREAL#0.0;
FixedParameters.OutputMaximum := LREAL#0.060;
FixedParameters.RateMinimum := LREAL#-0.005;
FixedParameters.RateMaximum := LREAL#0.005;
FixedParameters.TrackingTime := LREAL#0.5;
FixedParameters.AntiWindup := PIDAntiWindup#Combined;

ScheduleParameters.CommonParameters := FixedParameters;
ScheduleParameters.PointCount := DINT#3;
ScheduleParameters.FilterTimeConstant := LREAL#2.0;
ScheduleParameters.Points[0].OperatingPoint := LREAL#0.30;
ScheduleParameters.Points[0].Tuning.ProportionalGain := LREAL#0.100;
ScheduleParameters.Points[0].Tuning.IntegralGain := LREAL#0.00456;
ScheduleParameters.Points[0].Tuning.ProportionalSetpointWeight := LREAL#1.0;
ScheduleParameters.Points[0].Tuning.TrackingTime := LREAL#0.5;
ScheduleParameters.Points[1].OperatingPoint := LREAL#0.55;
ScheduleParameters.Points[1].Tuning.ProportionalGain := LREAL#0.100;
ScheduleParameters.Points[1].Tuning.IntegralGain := LREAL#0.00337;
ScheduleParameters.Points[1].Tuning.ProportionalSetpointWeight := LREAL#1.0;
ScheduleParameters.Points[1].Tuning.TrackingTime := LREAL#0.5;
ScheduleParameters.Points[2].OperatingPoint := LREAL#0.80;
ScheduleParameters.Points[2].Tuning.ProportionalGain := LREAL#0.100;
ScheduleParameters.Points[2].Tuning.IntegralGain := LREAL#0.00280;
ScheduleParameters.Points[2].Tuning.ProportionalSetpointWeight := LREAL#1.0;
ScheduleParameters.Points[2].Tuning.TrackingTime := LREAL#0.5;
GainScheduler.Configure(Parameters := ScheduleParameters);
GainScheduler.Initialize(OperatingPoint := InitialLevel);
ScheduledParameters := GainScheduler.Update(
    OperatingPoint := InitialLevel,
    CycleTime := LREAL#0.1
);

IF ScheduledControlEnabled THEN
    ActiveParameters := ScheduledParameters.Parameters;
ELSE
    ActiveParameters := FixedParameters;
END_IF;
PIDController.Configure(Parameters := ActiveParameters);
InitialState.Setpoint := InitialLevel;
InitialState.ProcessValue := InitialLevel;
InitialState.NominalOutput := InitialPumpFlow;
InitialState.SaturatedOutput := InitialPumpFlow;
PIDController.Initialize(State := InitialState);

LevelSetpoint := InitialLevel;
MeasuredLevel := InitialLevel;
RequestedPumpFlow := InitialPumpFlow;
AppliedPumpFlow := InitialPumpFlow;
LastAppliedPumpFlow := InitialPumpFlow;
ActiveKp := ActiveParameters.ProportionalGain;
ActiveKi := ActiveParameters.IntegralGain;
Configured := TRUE;
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    OperatingMode : DINT;
    RequestedLevel : LREAL;
    LevelMeasurement : LREAL;
    LevelQualityGood : BOOL;
    SchedulingAvailable : BOOL;
    PumpAvailable : BOOL;
    ManualPumpFlow : LREAL;
    CycleTime : LREAL;
END_VAR
VAR
    CandidatePumpFlow : LREAL;
    RawSetpointInEnvelope : BOOL;
END_VAR

IF NOT Configured THEN
    Update := FALSE;
ELSE
    LevelSetpoint := RequestedLevel;
    MeasuredLevel := LevelMeasurement;
    ScheduledParameters := GainScheduler.Update(
        OperatingPoint := RequestedLevel,
        CycleTime := CycleTime
    );
    FilteredSchedulingLevel := ScheduledParameters.FilteredOperatingPoint;
    InterpolationFraction := ScheduledParameters.Fraction;
    RawSetpointInEnvelope := (RequestedLevel >= LREAL#0.30)
        AND (RequestedLevel <= LREAL#0.80);
    ScheduleClamped := ScheduledParameters.Clamped OR (NOT RawSetpointInEnvelope);
    ScheduleValid := SchedulingAvailable AND ScheduledParameters.Valid
        AND (NOT ScheduleClamped);

    (* Apply one complete scheduler record between PID calls. No scan can use
       a new Ki with an old limit or tracking value. *)
    IF ScheduledControlEnabled AND ScheduleValid THEN
        ActiveParameters := ScheduledParameters.Parameters;
    ELSE
        ActiveParameters := FixedParameters;
    END_IF;
    PIDController.Configure(Parameters := ActiveParameters);
    ActiveKp := ActiveParameters.ProportionalGain;
    ActiveKi := ActiveParameters.IntegralGain;

    FaultActive := (NOT LevelQualityGood) OR (NOT PumpAvailable)
        OR (NOT RawSetpointInEnvelope) OR (CycleTime <= LREAL#0.0)
        OR (ScheduledControlEnabled AND (NOT ScheduleValid));
    PIDInput.Setpoint := RequestedLevel;
    PIDInput.ProcessValue := LevelMeasurement;
    PIDInput.FeedForward := LREAL#0.0;
    PIDInput.ManualOutput := LastAppliedPumpFlow;
    PIDInput.TrackingOutput := LastAppliedPumpFlow;
    PIDInput.CycleTime := CycleTime;
    TrackingActive := FALSE;

    IF FaultActive OR (OperatingMode = DINT#0) THEN
        (* Invalid authority closes the one-sided pump and tracks the actual
           zero flow so restoration cannot release hidden integral state. *)
        PIDInput.Setpoint := LevelMeasurement;
        PIDInput.Mode := PIDMode#Track;
        PIDInput.TrackingOutput := LREAL#0.0;
        PIDController.Update(Input := PIDInput);
        RequestedPumpFlow := LREAL#0.0;
        AppliedPumpFlow := LREAL#0.0;
        TrackingActive := TRUE;
    ELSIF OperatingMode = DINT#2 THEN
        PIDInput.Setpoint := LevelMeasurement;
        PIDInput.Mode := PIDMode#Track;
        PIDInput.TrackingOutput := LastAppliedPumpFlow;
        PIDController.Update(Input := PIDInput);
        RequestedPumpFlow := LIMIT(LREAL#0.0, ManualPumpFlow, LREAL#0.060);
        AppliedPumpFlow := LIMIT(
            LastAppliedPumpFlow - LREAL#0.0005,
            RequestedPumpFlow,
            LastAppliedPumpFlow + LREAL#0.0005
        );
        TrackingActive := TRUE;
    ELSIF OperatingMode = DINT#3 THEN
        PIDInput.Setpoint := LevelMeasurement;
        PIDInput.Mode := PIDMode#Track;
        PIDInput.TrackingOutput := LastAppliedPumpFlow;
        PIDController.Update(Input := PIDInput);
        RequestedPumpFlow := LREAL#0.0;
        AppliedPumpFlow := MAX(LREAL#0.0, LastAppliedPumpFlow - LREAL#0.0005);
        TrackingActive := TRUE;
    ELSE
        PIDInput.Mode := PIDMode#Auto;
        CandidatePumpFlow := PIDController.Update(Input := PIDInput);
        RequestedPumpFlow := LIMIT(LREAL#0.0, CandidatePumpFlow, LREAL#0.060);
        AppliedPumpFlow := LIMIT(
            LastAppliedPumpFlow - LREAL#0.0005,
            RequestedPumpFlow,
            LastAppliedPumpFlow + LREAL#0.0005
        );
    END_IF;

    LastAppliedPumpFlow := AppliedPumpFlow;
    Update := NOT FaultActive;
END_IF;
END_METHOD
END_FUNCTION_BLOCK
