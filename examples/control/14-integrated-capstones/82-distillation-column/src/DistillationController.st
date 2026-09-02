USING Control;

(* DistillationController composes two PI loops with optional static decoupling
   for the overhead and bottoms composition paths. It tracks the applied valve
   pair whenever automatic authority is absent and preserves the requested
   multivariable direction when a common physical limit is active. *)
FUNCTION_BLOCK DistillationController
VAR_INPUT
    OperatingMode : DINT;
    ArchitectureRequested : INT;
    QualityGood : BOOL;
    RefluxAvailable : BOOL;
    ReboilerAvailable : BOOL;
    OverheadSetpoint : LREAL;
    BottomsSetpoint : LREAL;
    FeedDisturbanceRequested : LREAL;
    ManualReflux : LREAL;
    ManualReboiler : LREAL;
    OverheadProcessValue : LREAL;
    BottomsProcessValue : LREAL;
END_VAR
VAR_OUTPUT
    ArchitectureActive : INT;
    ArchitectureQualified : BOOL := TRUE;
    VirtualOverheadCorrection : LREAL;
    VirtualBottomsCorrection : LREAL;
    RefluxRequested : LREAL;
    ReboilerRequested : LREAL;
    CommonScale : LREAL := 1.0;
    RefluxApplied : LREAL;
    ReboilerApplied : LREAL;
    FeedDisturbanceApplied : LREAL;
    StaticQualified : BOOL;
    Saturated : BOOL;
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL;
    ControllerRevision : DINT := DINT#82;
END_VAR
VAR
    OverheadController : Control.PID;
    BottomsController : Control.PID;
    StaticDecoupler : Control.StaticDecoupler2x2;
    OverheadParameters : Control.PIDParameters;
    BottomsParameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    OverheadInput : Control.PIDInput;
    BottomsInput : Control.PIDInput;
    DecouplerParameters : Control.StaticDecoupler2x2Parameters;
    VirtualCommand : Control.ControlVector2;
    CoupledCommand : Control.ControlVector2;
    AppliedCommand : Control.ControlVector2;
    TrackedVirtualCommand : Control.ControlVector2;
    Initialized : BOOL;
    RequestedPeak : LREAL;
END_VAR

ControllerRevision := DINT#82;
IF NOT Initialized THEN
    // Identical PI settings keep architecture, rather than retuning, as the experiment variable.
    OverheadParameters.ProportionalGain := LREAL#1.666666666666667;
    OverheadParameters.IntegralGain := LREAL#0.0833333333333333;
    OverheadParameters.ProportionalSetpointWeight := LREAL#1.0;
    OverheadParameters.OutputMinimum := LREAL#-8.0;
    OverheadParameters.OutputMaximum := LREAL#8.0;
    OverheadParameters.RateMinimum := LREAL#-2.0;
    OverheadParameters.RateMaximum := LREAL#2.0;
    OverheadParameters.TrackingTime := LREAL#1.0;
    OverheadParameters.AntiWindup := PIDAntiWindup#Combined;
    BottomsParameters := OverheadParameters;
    BottomsParameters.ProportionalGain := LREAL#2.142857142857143;
    BottomsParameters.IntegralGain := LREAL#0.0714285714285714;
    InitialState.Setpoint := LREAL#0.0;
    InitialState.ProcessValue := LREAL#0.0;
    InitialState.NominalOutput := LREAL#0.0;
    InitialState.SaturatedOutput := LREAL#0.0;
    OverheadController.Configure(Parameters := OverheadParameters);
    OverheadController.Initialize(State := InitialState);
    BottomsController.Configure(Parameters := BottomsParameters);
    BottomsController.Initialize(State := InitialState);

    DecouplerParameters.GainMatrix.G11 := LREAL#1.0;
    DecouplerParameters.GainMatrix.G12 := LREAL#-0.5;
    DecouplerParameters.GainMatrix.G21 := LREAL#-0.428571428571429;
    DecouplerParameters.GainMatrix.G22 := LREAL#1.0;
    DecouplerParameters.ActuatorBias.First := LREAL#0.0;
    DecouplerParameters.ActuatorBias.Second := LREAL#0.0;
    DecouplerParameters.SingularityTolerance := LREAL#0.001;
    StaticQualified := StaticDecoupler.Configure(Parameters := DecouplerParameters);
    Initialized := TRUE;
END_IF;

IF ArchitectureRequested = INT#0 THEN
    ArchitectureActive := INT#0;
    ArchitectureQualified := TRUE;
ELSIF ((ArchitectureRequested = INT#1) OR (ArchitectureRequested = INT#2))
    AND StaticQualified
THEN
    ArchitectureActive := ArchitectureRequested;
    ArchitectureQualified := TRUE;
ELSE
    ArchitectureActive := INT#0;
    ArchitectureQualified := FALSE;
END_IF;
FaultActive := (NOT QualityGood) OR (NOT RefluxAvailable)
    OR (NOT ReboilerAvailable);

AppliedCommand.First := RefluxApplied;
AppliedCommand.Second := ReboilerApplied;
IF ArchitectureActive >= INT#1 THEN
    TrackedVirtualCommand := StaticDecoupler.ToVirtual(
        ActuatorCommand := AppliedCommand
    );
ELSE
    TrackedVirtualCommand := AppliedCommand;
END_IF;

OverheadInput.Setpoint := OverheadSetpoint;
OverheadInput.ProcessValue := OverheadProcessValue;
OverheadInput.CycleTime := LREAL#0.1;
OverheadInput.FeedForward := LREAL#0.0;
OverheadInput.ManualOutput := TrackedVirtualCommand.First;
OverheadInput.TrackingOutput := TrackedVirtualCommand.First;
BottomsInput.Setpoint := BottomsSetpoint;
BottomsInput.ProcessValue := BottomsProcessValue;
BottomsInput.CycleTime := LREAL#0.1;
BottomsInput.FeedForward := LREAL#0.0;
BottomsInput.ManualOutput := TrackedVirtualCommand.Second;
BottomsInput.TrackingOutput := TrackedVirtualCommand.Second;
TrackingActive := FaultActive OR (OperatingMode <> DINT#1);
IF TrackingActive THEN
    (* Tracking the applied physical valves through the selected mapping keeps
       both PI histories aligned before authority returns to Automatic. *)
    OverheadInput.Setpoint := OverheadProcessValue;
    BottomsInput.Setpoint := BottomsProcessValue;
    OverheadInput.Mode := PIDMode#Manual;
    BottomsInput.Mode := PIDMode#Manual;
ELSE
    OverheadInput.Mode := PIDMode#Auto;
    BottomsInput.Mode := PIDMode#Auto;
END_IF;
VirtualCommand.First := OverheadController.Update(Input := OverheadInput);
VirtualCommand.Second := BottomsController.Update(Input := BottomsInput);
VirtualOverheadCorrection := VirtualCommand.First;
VirtualBottomsCorrection := VirtualCommand.Second;

IF ArchitectureActive >= INT#1 THEN
    CoupledCommand := StaticDecoupler.ToActuators(VirtualCommand := VirtualCommand);
    RefluxRequested := CoupledCommand.First;
    ReboilerRequested := CoupledCommand.Second;
ELSE
    RefluxRequested := VirtualCommand.First;
    ReboilerRequested := VirtualCommand.Second;
END_IF;

IF FaultActive OR (OperatingMode = DINT#0) THEN
    RefluxRequested := LREAL#0.0;
    ReboilerRequested := LREAL#0.0;
    RefluxApplied := LREAL#0.0;
    ReboilerApplied := LREAL#0.0;
    CommonScale := LREAL#1.0;
ELSIF OperatingMode = DINT#2 THEN
    RefluxRequested := LIMIT(LREAL#-10.0, ManualReflux, LREAL#10.0);
    ReboilerRequested := LIMIT(LREAL#-10.0, ManualReboiler, LREAL#10.0);
    RefluxApplied := LIMIT(RefluxApplied - LREAL#0.2, RefluxRequested, RefluxApplied + LREAL#0.2);
    ReboilerApplied := LIMIT(ReboilerApplied - LREAL#0.2, ReboilerRequested, ReboilerApplied + LREAL#0.2);
    CommonScale := LREAL#1.0;
ELSIF OperatingMode = DINT#3 THEN
    RefluxRequested := LREAL#0.0;
    ReboilerRequested := LREAL#0.0;
    RefluxApplied := LIMIT(RefluxApplied - LREAL#0.2, LREAL#0.0, RefluxApplied + LREAL#0.2);
    ReboilerApplied := LIMIT(ReboilerApplied - LREAL#0.2, LREAL#0.0, ReboilerApplied + LREAL#0.2);
    CommonScale := LREAL#1.0;
ELSE
    RequestedPeak := MAX(ABS(RefluxRequested), ABS(ReboilerRequested));
    IF ArchitectureActive = INT#2
        AND (RequestedPeak > LREAL#0.0)
    THEN
        (* One common scale preserves the requested two-valve direction when
           the physical magnitude must be reduced. *)
        CommonScale := MIN(LREAL#1.0, LREAL#10.0 / RequestedPeak);
        RefluxApplied := CommonScale * RefluxRequested;
        ReboilerApplied := CommonScale * ReboilerRequested;
    ELSE
        CommonScale := LREAL#1.0;
        RefluxApplied := LIMIT(LREAL#-10.0, RefluxRequested, LREAL#10.0);
        ReboilerApplied := LIMIT(LREAL#-10.0, ReboilerRequested, LREAL#10.0);
    END_IF;
END_IF;
Saturated := (CommonScale < LREAL#0.999999)
    OR (ABS(RefluxRequested - RefluxApplied) > LREAL#0.000001)
    OR (ABS(ReboilerRequested - ReboilerApplied) > LREAL#0.000001);
FeedDisturbanceApplied := LIMIT(
    LREAL#-3.0,
    FeedDisturbanceRequested,
    LREAL#3.0
);
END_FUNCTION_BLOCK
