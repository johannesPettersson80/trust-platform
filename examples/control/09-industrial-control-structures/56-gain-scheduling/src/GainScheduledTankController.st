USING Control;

(* GainScheduledTankController compares fixed tuning, interpolated scheduling,
   and parallel local controllers for the nonlinear gravity-drained tank. It
   keeps transfer tracking inside the component so a schedule change cannot
   create an artificial pump-command bump. *)
FUNCTION_BLOCK GainScheduledTankController
VAR_INPUT
    SchedulingAvailable : BOOL := TRUE;
    LevelSetpoint : LREAL := 0.16;
    FixedLevel : LREAL := 0.16;
    InterpolatedLevel : LREAL := 0.25;
    ParallelLevel : LREAL := 0.36;
END_VAR
VAR_OUTPUT
    FixedPumpRequest : LREAL := 0.008;
    InterpolatedPumpRequest : LREAL := 0.010;
    ParallelPumpRequest : LREAL := 0.012;
    FilteredSchedulingLevel : LREAL := 0.16;
    InterpolationFraction : LREAL := 0.0;
    ActiveKp : LREAL := 0.05;
    ActiveKi : LREAL := 0.0025;
    ParallelRegionValue : LREAL := 0.0;
    ParallelTransitionCount : LREAL := 0.0;
END_VAR
VAR
    FixedController : Control.PID;
    InterpolatedController : Control.PID;
    ParallelLowController : Control.PID;
    ParallelMiddleController : Control.PID;
    ParallelHighController : Control.PID;
    GainScheduler : Control.PIDGainScheduler;
    FixedParameters : Control.PIDParameters;
    InterpolatedParameters : Control.PIDParameters;
    LowParameters : Control.PIDParameters;
    MiddleParameters : Control.PIDParameters;
    HighParameters : Control.PIDParameters;
    FixedState : Control.PIDInitialState;
    InterpolatedState : Control.PIDInitialState;
    ParallelState : Control.PIDInitialState;
    FixedInput : Control.PIDInput;
    InterpolatedInput : Control.PIDInput;
    LowInput : Control.PIDInput;
    MiddleInput : Control.PIDInput;
    HighInput : Control.PIDInput;
    Schedule : Control.PIDGainScheduleParameters;
    Scheduled : Control.PIDGainScheduleOutput;
    Initialized : BOOL := FALSE;
    LowRequest : LREAL := 0.008;
    MiddleRequest : LREAL := 0.010;
    HighRequest : LREAL := 0.012;
    ParallelRegionChanged : BOOL := FALSE;
    PreviousParallelSetpoint : LREAL := 0.16;
    PreviousParallelLevel : LREAL := 0.36;
    LowTrackingReference : LREAL := 0.008;
    MiddleTrackingReference : LREAL := 0.010;
    HighTrackingReference : LREAL := 0.012;
END_VAR

IF NOT Initialized THEN
    FixedParameters.ProportionalGain := LREAL#0.050;
    FixedParameters.IntegralGain := LREAL#0.00200;
    FixedParameters.DerivativeGain := LREAL#0.0;
    FixedParameters.ProportionalSetpointWeight := LREAL#1.0;
    FixedParameters.DerivativeSetpointWeight := LREAL#0.0;
    FixedParameters.Bias := LREAL#0.0;
    FixedParameters.OutputMinimum := LREAL#0.0;
    FixedParameters.OutputMaximum := LREAL#0.020;
    FixedParameters.RateMinimum := LREAL#-1.0;
    FixedParameters.RateMaximum := LREAL#1.0;
    FixedParameters.TrackingTime := LREAL#1.0;
    FixedParameters.AntiWindup := PIDAntiWindup#Combined;

    LowParameters := FixedParameters;
    LowParameters.IntegralGain := LREAL#0.00250;
    MiddleParameters := FixedParameters;
    MiddleParameters.IntegralGain := LREAL#0.00200;
    HighParameters := FixedParameters;
    HighParameters.IntegralGain := LREAL#0.0016666667;

    Schedule.PointCount := DINT#3;
    Schedule.Points[0].OperatingPoint := LREAL#0.16;
    Schedule.Points[0].Tuning.ProportionalGain := LREAL#0.050;
    Schedule.Points[0].Tuning.IntegralGain := LREAL#0.00250;
    Schedule.Points[0].Tuning.ProportionalSetpointWeight := LREAL#1.0;
    Schedule.Points[0].Tuning.TrackingTime := LREAL#1.0;
    Schedule.Points[1].OperatingPoint := LREAL#0.25;
    Schedule.Points[1].Tuning.ProportionalGain := LREAL#0.050;
    Schedule.Points[1].Tuning.IntegralGain := LREAL#0.00200;
    Schedule.Points[1].Tuning.ProportionalSetpointWeight := LREAL#1.0;
    Schedule.Points[1].Tuning.TrackingTime := LREAL#1.0;
    Schedule.Points[2].OperatingPoint := LREAL#0.36;
    Schedule.Points[2].Tuning.ProportionalGain := LREAL#0.050;
    Schedule.Points[2].Tuning.IntegralGain := LREAL#0.0016666667;
    Schedule.Points[2].Tuning.ProportionalSetpointWeight := LREAL#1.0;
    Schedule.Points[2].Tuning.TrackingTime := LREAL#1.0;
    Schedule.CommonParameters := FixedParameters;
    Schedule.FilterTimeConstant := LREAL#2.0;
    GainScheduler.Configure(Parameters := Schedule);
    GainScheduler.Initialize(OperatingPoint := LevelSetpoint);
    Scheduled := GainScheduler.Update(OperatingPoint := LevelSetpoint, CycleTime := LREAL#1.0);
    InterpolatedParameters := Scheduled.Parameters;

    FixedState.Setpoint := LevelSetpoint;
    FixedState.ProcessValue := FixedLevel;
    FixedState.NominalOutput := LREAL#0.008;
    FixedState.SaturatedOutput := LREAL#0.008;
    InterpolatedState.Setpoint := LevelSetpoint;
    InterpolatedState.ProcessValue := InterpolatedLevel;
    InterpolatedState.NominalOutput := LREAL#0.010;
    InterpolatedState.SaturatedOutput := LREAL#0.010;
    ParallelState.Setpoint := LevelSetpoint;
    ParallelState.ProcessValue := ParallelLevel;
    ParallelState.NominalOutput := LREAL#0.012;
    ParallelState.SaturatedOutput := LREAL#0.012;

    FixedController.Configure(Parameters := FixedParameters);
    InterpolatedController.Configure(Parameters := InterpolatedParameters);
    ParallelLowController.Configure(Parameters := LowParameters);
    ParallelMiddleController.Configure(Parameters := MiddleParameters);
    ParallelHighController.Configure(Parameters := HighParameters);
    FixedController.Initialize(State := FixedState);
    InterpolatedController.Initialize(State := InterpolatedState);
    ParallelLowController.Initialize(State := ParallelState);
    ParallelMiddleController.Initialize(State := ParallelState);
    ParallelHighController.Initialize(State := ParallelState);
    Initialized := TRUE;
END_IF;

(* The scheduler returns one complete parameter record. Configure applies that
   record between PID updates, so a scan never mixes old and new gains. *)
Scheduled := GainScheduler.Update(OperatingPoint := LevelSetpoint, CycleTime := LREAL#1.0);
FilteredSchedulingLevel := Scheduled.FilteredOperatingPoint;
InterpolationFraction := Scheduled.Fraction;
IF SchedulingAvailable AND Scheduled.Valid AND NOT Scheduled.Clamped THEN
    InterpolatedParameters := Scheduled.Parameters;
END_IF;
InterpolatedController.Configure(Parameters := InterpolatedParameters);
ActiveKp := InterpolatedParameters.ProportionalGain;
ActiveKi := InterpolatedParameters.IntegralGain;

(* Hysteresis makes noisy boundary measurements insufficient to change the
   selected local controller. *)
ParallelRegionChanged := FALSE;
IF ParallelRegionValue = LREAL#0.0 AND ParallelLevel > LREAL#0.435 THEN
    ParallelRegionValue := LREAL#1.0;
    ParallelTransitionCount := ParallelTransitionCount + LREAL#1.0;
    ParallelRegionChanged := TRUE;
ELSIF ParallelRegionValue = LREAL#1.0 THEN
    IF ParallelLevel > LREAL#0.685 THEN
        ParallelRegionValue := LREAL#2.0;
        ParallelTransitionCount := ParallelTransitionCount + LREAL#1.0;
        ParallelRegionChanged := TRUE;
    ELSIF ParallelLevel < LREAL#0.415 THEN
        ParallelRegionValue := LREAL#0.0;
        ParallelTransitionCount := ParallelTransitionCount + LREAL#1.0;
        ParallelRegionChanged := TRUE;
    END_IF;
ELSIF ParallelRegionValue = LREAL#2.0 AND ParallelLevel < LREAL#0.665 THEN
    ParallelRegionValue := LREAL#1.0;
    ParallelTransitionCount := ParallelTransitionCount + LREAL#1.0;
    ParallelRegionChanged := TRUE;
END_IF;

FixedInput.Setpoint := LevelSetpoint;
FixedInput.ProcessValue := FixedLevel;
FixedInput.CycleTime := LREAL#1.0;
InterpolatedInput.Setpoint := LevelSetpoint;
InterpolatedInput.ProcessValue := InterpolatedLevel;
InterpolatedInput.CycleTime := LREAL#1.0;
LowInput.Setpoint := LevelSetpoint;
LowInput.ProcessValue := ParallelLevel;
LowInput.CycleTime := LREAL#1.0;
MiddleInput := LowInput;
HighInput := LowInput;

(* Track mode still evaluates the current incremental P and I terms. Subtracting
   those known increments from the reference makes the evaluated output equal
   the applied flow while preserving live setpoint and measurement histories. *)
LowTrackingReference := ParallelPumpRequest
    - LowParameters.ProportionalGain
        * ((LevelSetpoint - PreviousParallelSetpoint)
            - (ParallelLevel - PreviousParallelLevel))
    - LowParameters.IntegralGain * LREAL#1.0
        * (LevelSetpoint - ParallelLevel);
MiddleTrackingReference := ParallelPumpRequest
    - MiddleParameters.ProportionalGain
        * ((LevelSetpoint - PreviousParallelSetpoint)
            - (ParallelLevel - PreviousParallelLevel))
    - MiddleParameters.IntegralGain * LREAL#1.0
        * (LevelSetpoint - ParallelLevel);
HighTrackingReference := ParallelPumpRequest
    - HighParameters.ProportionalGain
        * ((LevelSetpoint - PreviousParallelSetpoint)
            - (ParallelLevel - PreviousParallelLevel))
    - HighParameters.IntegralGain * LREAL#1.0
        * (LevelSetpoint - ParallelLevel);

IF SchedulingAvailable AND Scheduled.Valid AND NOT Scheduled.Clamped THEN
    FixedInput.Mode := PIDMode#Auto;
    InterpolatedInput.Mode := PIDMode#Auto;
    (* On the transfer scan, every local PI tracks the actual request. The newly
       selected PI enters Auto on the following scan from matched state. *)
    IF ParallelRegionChanged THEN
        LowInput.Mode := PIDMode#Track;
        LowInput.TrackingOutput := LowTrackingReference;
        MiddleInput.Mode := PIDMode#Track;
        MiddleInput.TrackingOutput := MiddleTrackingReference;
        HighInput.Mode := PIDMode#Track;
        HighInput.TrackingOutput := HighTrackingReference;
    ELSIF ParallelRegionValue = LREAL#0.0 THEN
        LowInput.Mode := PIDMode#Auto;
        MiddleInput.Mode := PIDMode#Track;
        MiddleInput.TrackingOutput := MiddleTrackingReference;
        HighInput.Mode := PIDMode#Track;
        HighInput.TrackingOutput := HighTrackingReference;
    ELSIF ParallelRegionValue = LREAL#1.0 THEN
        LowInput.Mode := PIDMode#Track;
        LowInput.TrackingOutput := LowTrackingReference;
        MiddleInput.Mode := PIDMode#Auto;
        HighInput.Mode := PIDMode#Track;
        HighInput.TrackingOutput := HighTrackingReference;
    ELSE
        LowInput.Mode := PIDMode#Track;
        LowInput.TrackingOutput := LowTrackingReference;
        MiddleInput.Mode := PIDMode#Track;
        MiddleInput.TrackingOutput := MiddleTrackingReference;
        HighInput.Mode := PIDMode#Auto;
    END_IF;
ELSE
    FixedInput.Setpoint := FixedLevel;
    FixedInput.Mode := PIDMode#Track;
    FixedInput.TrackingOutput := LREAL#0.0;
    InterpolatedInput.Setpoint := InterpolatedLevel;
    InterpolatedInput.Mode := PIDMode#Track;
    InterpolatedInput.TrackingOutput := LREAL#0.0;
    LowInput.Setpoint := ParallelLevel;
    LowInput.Mode := PIDMode#Track;
    LowInput.TrackingOutput := LREAL#0.0;
    MiddleInput := LowInput;
    HighInput := LowInput;
END_IF;

FixedPumpRequest := FixedController.Update(Input := FixedInput);
InterpolatedPumpRequest := InterpolatedController.Update(Input := InterpolatedInput);
LowRequest := ParallelLowController.Update(Input := LowInput);
MiddleRequest := ParallelMiddleController.Update(Input := MiddleInput);
HighRequest := ParallelHighController.Update(Input := HighInput);
IF SchedulingAvailable AND Scheduled.Valid AND NOT Scheduled.Clamped THEN
    IF ParallelRegionValue = LREAL#0.0 THEN
        ParallelPumpRequest := LowRequest;
    ELSIF ParallelRegionValue = LREAL#1.0 THEN
        ParallelPumpRequest := MiddleRequest;
    ELSE
        ParallelPumpRequest := HighRequest;
    END_IF;
ELSE
    FixedPumpRequest := LREAL#0.0;
    InterpolatedPumpRequest := LREAL#0.0;
    ParallelPumpRequest := LREAL#0.0;
END_IF;
PreviousParallelSetpoint := LevelSetpoint;
PreviousParallelLevel := ParallelLevel;
END_FUNCTION_BLOCK
