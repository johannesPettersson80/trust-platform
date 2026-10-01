USING Control;

(* Fraction of the raw-minus-filtered difference applied per update: alpha = dt/(Tf + dt). *)
FUNCTION FilterFraction : LREAL
VAR_INPUT
    CycleTime : LREAL;
    FilterTime : LREAL;
END_VAR
FilterFraction := CycleTime / (FilterTime + CycleTime);
END_FUNCTION

(* One first-order filter update: the filtered value moves alpha of the way toward the raw value. *)
FUNCTION FilterUpdate : LREAL
VAR_INPUT
    PreviousFilteredValue : LREAL;
    RawValue : LREAL;
    FilterFraction : LREAL;
END_VAR
FilterUpdate := PreviousFilteredValue + FilterFraction * (RawValue - PreviousFilteredValue);
END_FUNCTION

(* An angle reading is usable only with good encoder quality, inside the encoder's −π to π rad and with an age
   between 0 and MaximumAge. NaN fails every comparison. *)
FUNCTION ReadingUsable : BOOL
VAR_INPUT
    QualityGood : BOOL;
    Value : LREAL;
    Age : LREAL;
    MaximumAge : LREAL;
END_VAR
ReadingUsable := QualityGood AND Value >= LREAL#-3.14159 AND Value <= LREAL#3.14159
    AND Age >= LREAL#0.0 AND Age <= MaximumAge;
END_FUNCTION

(* Angle the load pulls the axis down from rest while the drive has no torque: J·dω/dt = −τL − B·ω gives
   θ(t) = −(τL/B)·(t − (J/B)·(1 − e^(−B·t/J))). *)
FUNCTION LoadDropRad : LREAL
VAR_INPUT
    LoadTorque : LREAL;
    Inertia : LREAL;
    Friction : LREAL;
    Seconds : LREAL;
END_VAR
LoadDropRad := -LoadTorque / Friction
    * (Seconds - Inertia / Friction * (LREAL#1.0 - EXP(-Friction * Seconds / Inertia)));
END_FUNCTION

(* A candidate set is usable only as a whole: non-negative gains, weights in 0–1, a minimum below the
   maximum, rate limits around zero, a positive tracking time and a non-negative filter time.
   Any NaN fails its comparison and rejects the set. *)
FUNCTION ValidPidParameters : BOOL
VAR_INPUT
    Candidate : Control.PIDParameters;
    FilterTime : LREAL;
END_VAR
ValidPidParameters :=
    Candidate.ProportionalGain >= LREAL#0.0 AND Candidate.IntegralGain >= LREAL#0.0
    AND Candidate.DerivativeGain >= LREAL#0.0
    AND Candidate.ProportionalSetpointWeight >= LREAL#0.0 AND Candidate.ProportionalSetpointWeight <= LREAL#1.0
    AND Candidate.DerivativeSetpointWeight >= LREAL#0.0 AND Candidate.DerivativeSetpointWeight <= LREAL#1.0
    AND Candidate.Bias = Candidate.Bias
    AND Candidate.OutputMinimum < Candidate.OutputMaximum
    AND Candidate.RateMinimum <= LREAL#0.0 AND Candidate.RateMaximum >= LREAL#0.0
    AND Candidate.TrackingTime > LREAL#0.0
    AND FilterTime >= LREAL#0.0;
END_FUNCTION

FUNCTION SamePidParameters : BOOL
VAR_INPUT
    Left : Control.PIDParameters;
    Right : Control.PIDParameters;
END_VAR
SamePidParameters :=
    Left.ProportionalGain = Right.ProportionalGain
    AND Left.IntegralGain = Right.IntegralGain
    AND Left.DerivativeGain = Right.DerivativeGain
    AND Left.ProportionalSetpointWeight = Right.ProportionalSetpointWeight
    AND Left.DerivativeSetpointWeight = Right.DerivativeSetpointWeight
    AND Left.Bias = Right.Bias
    AND Left.OutputMinimum = Right.OutputMinimum
    AND Left.OutputMaximum = Right.OutputMaximum
    AND Left.RateMinimum = Right.RateMinimum
    AND Left.RateMaximum = Right.RateMaximum
    AND Left.TrackingTime = Right.TrackingTime
    AND Left.AntiWindup = Right.AntiWindup;
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    (* The installed axis runs the Lesson 39 controller; commissioning applies the Lesson 37 candidate.
       Torque in N·m, limited to the drive's ±20 N·m with back-calculation anti-windup. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#16.0,
        IntegralGain := LREAL#4.0,
        DerivativeGain := LREAL#7.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#-20.0,
        OutputMaximum := LREAL#20.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    ConfiguredParameters : Control.PIDParameters;
    ConfiguredFilterTimeS : LREAL := 0.0;
    ParameterRevision : DINT := 0;
    ParametersRejected : BOOL := FALSE;
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    (* Ready: every retained value has been set from the actual condition of the axis. *)
    Initialized : BOOL := FALSE;
    InitializeRequest : BOOL := FALSE;
    InitializeRequestWas : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    AutoActive : BOOL := FALSE;
    (* TRUE at start: a program that starts in Auto from preset stored values continues without a
       transfer. A real start begins uninitialized in Manual, which clears it. *)
    AutoWasActive : BOOL := TRUE;
    ManualTorqueNm : LREAL := 0.0;
    SafetyAbort : BOOL := FALSE;
    (* Encoder interface: the simulated encoder delivers the angle with a quality flag every scan; these
       two inputs inject faults. *)
    EncoderQualityGood : BOOL := TRUE;
    EncoderFrozen : BOOL := FALSE;
    ReadingRad : LREAL := 0.0;
    AcquisitionTimer : TON;
    AcquisitionAgeS : LREAL := 0.0;
    NewAcquisition : BOOL := FALSE;
    (* Measurement validity: a reading older than two updates is stale; recovery needs three consecutive
       fresh valid readings after an acknowledgement. *)
    MaximumAgeS : LREAL := 0.04;
    FreshSamplesRequired : DINT := 3;
    MeasurementUsable : BOOL := FALSE;
    MeasurementFault : BOOL := FALSE;
    FaultAcknowledge : BOOL := FALSE;
    Acknowledged : BOOL := FALSE;
    RecoveryCount : DINT := 0;
    RecoveryComplete : BOOL := FALSE;
    SafeAngleRad : LREAL := 0.0;
    SetpointRad : LREAL := 0.0;
    AngleRad : LREAL := 0.0;
    VelocityRadS : LREAL := 0.0;
    TorqueNm : LREAL := 0.0;
    (* The request Control.PID keeps: above the limit while the drive is saturated. *)
    RequestedTorqueNm : LREAL := 0.0;
    (* Drum load of the simulated test rig, in N·m: a 2 kg weight at the 0.1 m drum radius, 1.962 N·m.
       The controller does not use it. *)
    LoadTorqueNm : LREAL := 0.0;
    (* Measurement filter on the angle (Lesson 37): Control.PID differentiates the filtered angle. *)
    FilterTimeS : LREAL := 0.1;
    FilterAlpha : LREAL := 1.0;
    FilteredAngleRad : LREAL := 0.0;
    (* Software travel and speed limits of the axis. *)
    AngleLimitRad : LREAL := 2.0;
    VelocityLimitRadS : LREAL := 10.0;
END_VAR

(* 1. Encoder and validity (Lesson 29). Every scan the encoder delivers a new acquisition unless it is
   frozen; the acquisition timer restarts with each one, so its elapsed time is the reading's age. *)
IF NOT EncoderFrozen THEN
    ReadingRad := AngleRad;
    AcquisitionTimer(IN := FALSE, PT := T#1h);
    NewAcquisition := TRUE;
ELSE
    NewAcquisition := FALSE;
END_IF;
AcquisitionTimer(IN := TRUE, PT := T#1h);
AcquisitionAgeS := UDINT_TO_LREAL(DWORD_TO_UDINT(TIME_TO_DWORD(AcquisitionTimer.ET))) / LREAL#1000.0;
MeasurementUsable := ReadingUsable(QualityGood := EncoderQualityGood, Value := ReadingRad,
    Age := AcquisitionAgeS, MaximumAge := MaximumAgeS);
IF MeasurementUsable THEN
    SafeAngleRad := ReadingRad;
END_IF;

(* A latched fault removes the torque, stops every controller and filter update, and clears the Auto
   request and the manual torque, so neither can restart the drive by itself. *)
IF NOT MeasurementUsable AND NOT MeasurementFault THEN
    MeasurementFault := TRUE;
    AutoMode := FALSE;
    ManualTorqueNm := LREAL#0.0;
    Acknowledged := FALSE;
    RecoveryCount := 0;
END_IF;
(* Recovery: after an acknowledgement, FreshSamplesRequired consecutive new and valid readings. A repeated
   old reading does not count; an invalid one restarts the count. *)
RecoveryComplete := FALSE;
IF MeasurementFault THEN
    IF FaultAcknowledge THEN
        Acknowledged := TRUE;
        FaultAcknowledge := FALSE;
        RecoveryCount := 0;
    END_IF;
    IF NOT MeasurementUsable THEN
        RecoveryCount := 0;
    ELSIF Acknowledged AND NewAcquisition THEN
        RecoveryCount := RecoveryCount + 1;
    END_IF;
    IF RecoveryCount >= FreshSamplesRequired THEN
        MeasurementFault := FALSE;
        Acknowledged := FALSE;
        RecoveryCount := 0;
        RecoveryComplete := TRUE;
    END_IF;
ELSE
    FaultAcknowledge := FALSE;
END_IF;

(* Leaving the travel or speed range removes the torque until a full restart. *)
IF ABS(SafeAngleRad) > AngleLimitRad OR ABS(VelocityRadS) > VelocityLimitRadS THEN
    SafetyAbort := TRUE;
END_IF;

(* 2. Parameter updates (Lesson 28): Control Studio's Apply writes the complete record and FilterTimeS at
   one task boundary. A changed set is adopted only as a whole and only if it is valid; an invalid set
   changes nothing. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters)
        OR FilterTimeS <> ConfiguredFilterTimeS THEN
    IF ValidPidParameters(Candidate := Parameters, FilterTime := FilterTimeS) THEN
        ConfiguredParameters := Parameters;
        ConfiguredFilterTimeS := FilterTimeS;
        ParameterRevision := ParameterRevision + 1;
        ParametersRejected := FALSE;
    ELSE
        ParametersRejected := TRUE;
    END_IF;
ELSE
    ParametersRejected := FALSE;
END_IF;
Controller.Configure(Parameters := ConfiguredParameters);

(* 3. One writer for the torque (Lesson 26): a fault or a safety stop gives zero torque; otherwise Manual
   commands the drive until automatic control is active. Automatic control needs integral action: only the
   integral holds the load on target, and only the incremental form transfers without a bump. *)
AutoActive := AutoMode AND Initialized AND NOT MeasurementFault AND NOT SafetyAbort
    AND ConfiguredParameters.ProportionalGain > LREAL#0.0 AND ConfiguredParameters.IntegralGain > LREAL#0.0;
IF MeasurementFault OR SafetyAbort THEN
    TorqueNm := LREAL#0.0;
ELSIF NOT AutoActive THEN
    TorqueNm := LIMIT(MN := ConfiguredParameters.OutputMinimum, IN := ManualTorqueNm, MX := ConfiguredParameters.OutputMaximum);
END_IF;

(* 4. Initialization (Lesson 26): every retained value from the actual condition, at startup, once per
   rising edge of InitializeRequest and after a recovery, only while Manual owns the torque. *)
IF (NOT Initialized OR (InitializeRequest AND NOT InitializeRequestWas) OR RecoveryComplete)
        AND NOT AutoActive AND NOT MeasurementFault AND MeasurementUsable THEN
    IF NOT Initialized AND NOT AutoMode THEN
        SetpointRad := SafeAngleRad;
    END_IF;
    FilteredAngleRad := SafeAngleRad;
    InitialState.Setpoint := SetpointRad;
    InitialState.ProcessValue := FilteredAngleRad;
    InitialState.NominalOutput := TorqueNm;
    InitialState.SaturatedOutput := TorqueNm;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    RequestedTorqueNm := TorqueNm;
    Initialized := TRUE;
END_IF;
InitializeRequestWas := InitializeRequest;

(* 5. Filter and PID update. No update uses an unusable reading. The filter runs in every mode, so the
   filtered velocity always matches the rates Control.PID stores; only initialization restarts it. In Manual
   and on the transfer scan the stored values follow the applied torque, the setpoint and the filtered angle
   (Lesson 27); normal updates resume on the next scan. *)
IF NOT MeasurementFault AND NOT SafetyAbort THEN
    FilterAlpha := FilterFraction(CycleTime := LREAL#0.02, FilterTime := ConfiguredFilterTimeS);
    FilteredAngleRad := FilterUpdate(PreviousFilteredValue := FilteredAngleRad, RawValue := SafeAngleRad, FilterFraction := FilterAlpha);
    IF AutoActive AND AutoWasActive THEN
        Input.Mode := PIDMode#Auto;
    ELSE
        Input.Mode := PIDMode#Manual;
    END_IF;
    Input.Setpoint := SetpointRad;
    Input.ProcessValue := FilteredAngleRad;
    Input.CycleTime := LREAL#0.02;
    Input.FeedForward := LREAL#0.0;
    Input.TrackingOutput := TorqueNm;
    Input.ManualOutput := TorqueNm;
    TorqueNm := Controller.Update(Input := Input);
    ObservedState := Controller.GetState();
    RequestedTorqueNm := ObservedState.NominalOutput;
    AutoWasActive := AutoActive;
ELSE
    AutoWasActive := FALSE;
END_IF;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX24.0 : BOOL;
    P1.MeasurementFault AT %MX24.1 : BOOL;
    P1.TorqueNm AT %QL0 : LREAL;
    P1.LoadTorqueNm AT %QL8 : LREAL;
    P1.AngleRad AT %IL0 : LREAL;
    P1.VelocityRadS AT %IL8 : LREAL;
    P1.SetpointRad AT %ML0 : LREAL;
    P1.ManualTorqueNm AT %ML8 : LREAL;
    P1.RequestedTorqueNm AT %ML32 : LREAL;
END_VAR
END_CONFIGURATION
