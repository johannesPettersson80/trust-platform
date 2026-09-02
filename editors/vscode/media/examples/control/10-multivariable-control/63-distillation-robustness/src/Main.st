USING Control;

PROGRAM Main
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
    Initialized : BOOL := FALSE;
    AutoMode1 : BOOL := TRUE;
    AutoMode2 : BOOL := TRUE;
    Architecture : INT := 0;
    Setpoint1 : LREAL := 0.0;
    Setpoint2 : LREAL := 0.0;
    FeedDisturbanceRequested : LREAL := 0.0;
    ProcessValue1 : LREAL := 0.0;
    ProcessValue2 : LREAL := 0.0;
    VirtualOverheadCorrection : LREAL := 0.0;
    VirtualBottomsCorrection : LREAL := 0.0;
    RefluxRequested : LREAL := 0.0;
    ReboilerRequested : LREAL := 0.0;
    RequestedPeak : LREAL := 0.0;
    CommonScale : LREAL := 1.0;
    RefluxApplied : LREAL := 0.0;
    ReboilerApplied : LREAL := 0.0;
    FeedDisturbanceApplied : LREAL := 0.0;
    StaticQualified : BOOL := FALSE;
    Saturated : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    AutoMode1 := TRUE;
    AutoMode2 := TRUE;
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

OverheadInput.Setpoint := Setpoint1;
OverheadInput.ProcessValue := ProcessValue1;
OverheadInput.CycleTime := LREAL#0.1;
OverheadInput.ManualOutput := LREAL#0.0;
BottomsInput.Setpoint := Setpoint2;
BottomsInput.ProcessValue := ProcessValue2;
BottomsInput.CycleTime := LREAL#0.1;
BottomsInput.ManualOutput := LREAL#0.0;
IF AutoMode1 THEN
    OverheadInput.Mode := PIDMode#Auto;
ELSE
    OverheadInput.Mode := PIDMode#Manual;
END_IF;
IF AutoMode2 THEN
    BottomsInput.Mode := PIDMode#Auto;
ELSE
    BottomsInput.Mode := PIDMode#Manual;
END_IF;
VirtualCommand.First := OverheadController.Update(Input := OverheadInput);
VirtualCommand.Second := BottomsController.Update(Input := BottomsInput);
VirtualOverheadCorrection := VirtualCommand.First;
VirtualBottomsCorrection := VirtualCommand.Second;

IF (Architecture >= 1) AND StaticQualified THEN
    CoupledCommand := StaticDecoupler.ToActuators(VirtualCommand := VirtualCommand);
    RefluxRequested := CoupledCommand.First;
    ReboilerRequested := CoupledCommand.Second;
ELSE
    RefluxRequested := VirtualCommand.First;
    ReboilerRequested := VirtualCommand.Second;
END_IF;

RequestedPeak := MAX(ABS(RefluxRequested), ABS(ReboilerRequested));
IF (Architecture = 2) AND StaticQualified AND (RequestedPeak > LREAL#0.0) THEN
    // One scale preserves the requested input direction when a shared constraint is active.
    CommonScale := MIN(LREAL#1.0, LREAL#10.0 / RequestedPeak);
    RefluxApplied := CommonScale * RefluxRequested;
    ReboilerApplied := CommonScale * ReboilerRequested;
ELSE
    CommonScale := LREAL#1.0;
    RefluxApplied := LIMIT(LREAL#-10.0, RefluxRequested, LREAL#10.0);
    ReboilerApplied := LIMIT(LREAL#-10.0, ReboilerRequested, LREAL#10.0);
END_IF;
Saturated := (CommonScale < LREAL#0.999999)
    OR (ABS(RefluxRequested - RefluxApplied) > LREAL#0.000001)
    OR (ABS(ReboilerRequested - ReboilerApplied) > LREAL#0.000001);
FeedDisturbanceApplied := LIMIT(
    LREAL#-3.0,
    FeedDisturbanceRequested,
    LREAL#3.0
);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode1 AT %MX24.0 : BOOL;
    P1.AutoMode2 AT %MX24.1 : BOOL;
    P1.Architecture AT %MW16 : INT;
    P1.Setpoint1 AT %ML32 : LREAL;
    P1.Setpoint2 AT %ML40 : LREAL;
    P1.FeedDisturbanceRequested AT %ML48 : LREAL;
    P1.VirtualOverheadCorrection AT %ML56 : LREAL;
    P1.VirtualBottomsCorrection AT %ML64 : LREAL;
    P1.RefluxRequested AT %ML72 : LREAL;
    P1.ReboilerRequested AT %ML80 : LREAL;
    P1.CommonScale AT %ML88 : LREAL;
    P1.StaticQualified AT %MX24.2 : BOOL;
    P1.Saturated AT %MX24.3 : BOOL;
    P1.RefluxApplied AT %QL0 : LREAL;
    P1.ReboilerApplied AT %QL8 : LREAL;
    P1.FeedDisturbanceApplied AT %QL16 : LREAL;
    P1.ProcessValue1 AT %IL0 : LREAL;
    P1.ProcessValue2 AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
