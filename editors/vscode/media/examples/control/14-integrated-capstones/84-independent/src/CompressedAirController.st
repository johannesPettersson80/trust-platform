USING Control;

(* CompressedAirController compares pressure-only PI with qualified measured-
   demand feedforward for the factory receiver. It owns controller state and
   bumpless tracking, while its explicit mode, quality, timing, and availability
   inputs determine whether either requested compressor command has authority. *)
FUNCTION_BLOCK CompressedAirController
VAR_INPUT
    Enabled : BOOL;
    OperatingMode : DINT;
    PressureQualityGood : BOOL;
    DemandQualityGood : BOOL;
    DeliveryQualityGood : BOOL;
    CompressorAvailable : BOOL;
    PressureSetpoint : LREAL;
    MeasuredDemand : LREAL;
    ManualFeedbackRequest : LREAL;
    ManualIntegratedRequest : LREAL;
    CycleTime : LREAL;
    FeedbackPressure : LREAL;
    IntegratedPressure : LREAL;
    FeedbackActualDelivery : LREAL;
    IntegratedActualDelivery : LREAL;
END_VAR
VAR_OUTPUT
    Initialized : BOOL;
    FeedforwardQualified : BOOL := TRUE;
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL;
    TimingValid : BOOL := TRUE;
    FeedbackRequested : LREAL := 50.0;
    IntegratedFeedback : LREAL := 50.0;
    IntegratedFeedforward : LREAL;
    IntegratedRequested : LREAL := 50.0;
    FeedbackApplied : LREAL;
    IntegratedApplied : LREAL;
    PreviousFeedbackApplied : LREAL := 50.0;
    PreviousIntegratedApplied : LREAL := 50.0;
    FeedbackPlantDemand : LREAL := 50.0;
    IntegratedPlantDemand : LREAL := 50.0;
    ControllerRevision : DINT := DINT#84;
END_VAR
VAR
    FeedbackController : Control.PID;
    IntegratedController : Control.PID;
    Parameters : Control.PIDParameters;
    FeedbackInitialState : Control.PIDInitialState;
    IntegratedInitialState : Control.PIDInitialState;
    FeedbackInput : Control.PIDInput;
    IntegratedInput : Control.PIDInput;
END_VAR

ControllerRevision := DINT#84;
IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#40.0;
    Parameters.IntegralGain := LREAL#1.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#50.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-10.0;
    Parameters.RateMaximum := LREAL#10.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;

    FeedbackInitialState.Setpoint := PressureSetpoint;
    FeedbackInitialState.ProcessValue := FeedbackPressure;
    FeedbackInitialState.FeedForward := LREAL#0.0;
    FeedbackInitialState.NominalOutput := LREAL#50.0;
    FeedbackInitialState.SaturatedOutput := LREAL#50.0;
    IntegratedInitialState := FeedbackInitialState;

    FeedbackController.Configure(Parameters := Parameters);
    IntegratedController.Configure(Parameters := Parameters);
    FeedbackController.Initialize(State := FeedbackInitialState);
    IntegratedController.Initialize(State := IntegratedInitialState);
    PreviousFeedbackApplied := LREAL#50.0;
    PreviousIntegratedApplied := LREAL#50.0;
    Initialized := TRUE;
END_IF;

TimingValid := (CycleTime >= LREAL#0.8) AND (CycleTime <= LREAL#1.2);
FaultActive := (NOT PressureQualityGood) OR (NOT DeliveryQualityGood)
    OR (NOT TimingValid) OR (NOT CompressorAvailable);
FeedforwardQualified := DemandQualityGood
    AND (MeasuredDemand >= LREAL#0.0) AND (MeasuredDemand <= LREAL#100.0);
IF FeedforwardQualified THEN
    IntegratedFeedforward := MeasuredDemand - LREAL#50.0;
ELSE
    (* Demand is an enhancement signal: invalid demand removes feedforward but
       does not remove qualified pressure feedback authority. *)
    IntegratedFeedforward := LREAL#0.0;
END_IF;

TrackingActive := FaultActive OR (NOT Enabled) OR (OperatingMode <> DINT#1);
FeedbackInput.Setpoint := PressureSetpoint;
FeedbackInput.ProcessValue := FeedbackPressure;
FeedbackInput.FeedForward := LREAL#0.0;
FeedbackInput.CycleTime := CycleTime;
IntegratedInput.Setpoint := PressureSetpoint;
IntegratedInput.ProcessValue := IntegratedPressure;
IntegratedInput.FeedForward := IntegratedFeedforward;
IntegratedInput.CycleTime := CycleTime;

IF TrackingActive THEN
    FeedbackInput.Mode := PIDMode#Track;
    IntegratedInput.Mode := PIDMode#Track;
    IF DeliveryQualityGood THEN
        (* The installed drive owns its inner delivery loop. Track measured
           delivery during external authority so return to Auto is bumpless. *)
        FeedbackInput.TrackingOutput := FeedbackActualDelivery;
        IntegratedInput.TrackingOutput := IntegratedActualDelivery;
    ELSE
        FeedbackInput.TrackingOutput := LREAL#0.0;
        IntegratedInput.TrackingOutput := LREAL#0.0;
    END_IF;
ELSE
    FeedbackInput.Mode := PIDMode#Auto;
    IntegratedInput.Mode := PIDMode#Auto;
END_IF;

FeedbackRequested := FeedbackController.Update(Input := FeedbackInput);
IntegratedRequested := IntegratedController.Update(Input := IntegratedInput);
IntegratedFeedback := IntegratedRequested - IntegratedFeedforward;

IF FaultActive OR (NOT Enabled) OR (OperatingMode = DINT#0)
    OR (OperatingMode < DINT#0) OR (OperatingMode > DINT#3)
THEN
    FeedbackRequested := LREAL#0.0;
    IntegratedRequested := LREAL#0.0;
    FeedbackApplied := LREAL#0.0;
    IntegratedApplied := LREAL#0.0;
ELSIF OperatingMode = DINT#2 THEN
    FeedbackRequested := LIMIT(LREAL#0.0, ManualFeedbackRequest, LREAL#100.0);
    IntegratedRequested := LIMIT(LREAL#0.0, ManualIntegratedRequest, LREAL#100.0);
    FeedbackApplied := LIMIT(
        MAX(LREAL#0.0, PreviousFeedbackApplied - LREAL#10.0 * CycleTime),
        FeedbackRequested,
        MIN(LREAL#100.0, PreviousFeedbackApplied + LREAL#10.0 * CycleTime)
    );
    IntegratedApplied := LIMIT(
        MAX(LREAL#0.0, PreviousIntegratedApplied - LREAL#10.0 * CycleTime),
        IntegratedRequested,
        MIN(LREAL#100.0, PreviousIntegratedApplied + LREAL#10.0 * CycleTime)
    );
ELSIF OperatingMode = DINT#3 THEN
    FeedbackRequested := LREAL#0.0;
    IntegratedRequested := LREAL#0.0;
    FeedbackApplied := MAX(
        LREAL#0.0,
        PreviousFeedbackApplied - LREAL#10.0 * CycleTime
    );
    IntegratedApplied := MAX(
        LREAL#0.0,
        PreviousIntegratedApplied - LREAL#10.0 * CycleTime
    );
ELSE
    FeedbackApplied := LIMIT(
        MAX(LREAL#0.0, PreviousFeedbackApplied - LREAL#10.0 * CycleTime),
        FeedbackRequested,
        MIN(LREAL#100.0, PreviousFeedbackApplied + LREAL#10.0 * CycleTime)
    );
    IntegratedApplied := LIMIT(
        MAX(LREAL#0.0, PreviousIntegratedApplied - LREAL#10.0 * CycleTime),
        IntegratedRequested,
        MIN(LREAL#100.0, PreviousIntegratedApplied + LREAL#10.0 * CycleTime)
    );
END_IF;

FeedbackPlantDemand := LIMIT(LREAL#0.0, MeasuredDemand, LREAL#100.0);
IntegratedPlantDemand := FeedbackPlantDemand;
PreviousFeedbackApplied := FeedbackApplied;
PreviousIntegratedApplied := IntegratedApplied;
END_FUNCTION_BLOCK
