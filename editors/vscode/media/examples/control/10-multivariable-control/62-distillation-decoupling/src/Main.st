USING Control;

PROGRAM Main
VAR
    OverheadController : Control.PID;
    BottomsController : Control.PID;
    StaticDecoupler : Control.StaticDecoupler2x2;
    DynamicFilter12 : Control.LowPass;
    DynamicFilter21 : Control.LowPass;
    OverheadParameters : Control.PIDParameters;
    BottomsParameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    OverheadInput : Control.PIDInput;
    BottomsInput : Control.PIDInput;
    DecouplerParameters : Control.StaticDecoupler2x2Parameters;
    Filter12Parameters : Control.LowPassParameters;
    Filter21Parameters : Control.LowPassParameters;
    VirtualCommand : Control.ControlVector2;
    StaticCommand : Control.ControlVector2;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
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
    FilteredBottomsCorrection : LREAL := 0.0;
    FilteredOverheadCorrection : LREAL := 0.0;
    RefluxRequested : LREAL := 0.0;
    ReboilerRequested : LREAL := 0.0;
    RefluxApplied : LREAL := 0.0;
    ReboilerApplied : LREAL := 0.0;
    FeedDisturbanceApplied : LREAL := 0.0;
    StaticQualified : BOOL := FALSE;
END_VAR

IF NOT Initialized THEN
    AutoMode1 := TRUE;
    AutoMode2 := TRUE;
    Architecture := 0;
    // The same diagonal PI settings remain active in all three architectures.
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

    // Row normalization preserves the two direct gains while the inverse cancels steady interaction.
    DecouplerParameters.GainMatrix.G11 := LREAL#1.0;
    DecouplerParameters.GainMatrix.G12 := LREAL#-0.5;
    DecouplerParameters.GainMatrix.G21 := LREAL#-0.428571428571429;
    DecouplerParameters.GainMatrix.G22 := LREAL#1.0;
    DecouplerParameters.ActuatorBias.First := LREAL#0.0;
    DecouplerParameters.ActuatorBias.Second := LREAL#0.0;
    DecouplerParameters.SingularityTolerance := LREAL#0.001;
    StaticQualified := StaticDecoupler.Configure(Parameters := DecouplerParameters);

    Filter12Parameters.TimeConstant := LREAL#35.0;
    Filter12Parameters.Order := FilterOrder#FirstOrder;
    DynamicFilter12.Configure(Parameters := Filter12Parameters);
    DynamicFilter12.Initialize(Input := LREAL#0.0);
    Filter21Parameters.TimeConstant := LREAL#40.0;
    Filter21Parameters.Order := FilterOrder#FirstOrder;
    DynamicFilter21.Configure(Parameters := Filter21Parameters);
    DynamicFilter21.Initialize(Input := LREAL#0.0);
    Initialized := TRUE;
END_IF;

(* The interactive observation is the qualified nominal dynamic case. Control
   Studio grants only authority; native tests own the restored architecture and
   mismatch comparison. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    Architecture := 2;
    Setpoint2 := LREAL#0.0;
    FeedDisturbanceRequested := LREAL#0.0;
    IF ExperimentElapsed < LREAL#20.0 THEN
        Setpoint1 := LREAL#0.0;
    ELSE
        Setpoint1 := LREAL#2.0;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    Architecture := 0;
    Setpoint1 := LREAL#0.0;
    Setpoint2 := LREAL#0.0;
    FeedDisturbanceRequested := LREAL#0.0;
END_IF;

OverheadInput.Setpoint := Setpoint1;
OverheadInput.ProcessValue := ProcessValue1;
OverheadInput.CycleTime := LREAL#0.1;
BottomsInput.Setpoint := Setpoint2;
BottomsInput.ProcessValue := ProcessValue2;
BottomsInput.CycleTime := LREAL#0.1;
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

// Filters update continuously so selecting the dynamic path never starts from stale state.
FilteredBottomsCorrection := DynamicFilter12.Update(
    Input := VirtualCommand.Second,
    CycleTime := LREAL#0.1
);
FilteredOverheadCorrection := DynamicFilter21.Update(
    Input := VirtualCommand.First,
    CycleTime := LREAL#0.1
);

IF (Architecture = 1) AND StaticQualified THEN
    StaticCommand := StaticDecoupler.ToActuators(VirtualCommand := VirtualCommand);
    RefluxRequested := StaticCommand.First;
    ReboilerRequested := StaticCommand.Second;
ELSIF (Architecture = 2) AND StaticQualified THEN
    // These proper lead-lag ratios cancel the nominal unequal cross-path dynamics.
    RefluxRequested := VirtualCommand.First
        + LREAL#0.285714285714286 * VirtualCommand.Second
        + LREAL#0.214285714285714 * FilteredBottomsCorrection;
    ReboilerRequested := VirtualCommand.Second
        + LREAL#0.321428571428571 * VirtualCommand.First
        + LREAL#0.107142857142857 * FilteredOverheadCorrection;
ELSE
    RefluxRequested := VirtualCommand.First;
    ReboilerRequested := VirtualCommand.Second;
END_IF;

RefluxApplied := LIMIT(LREAL#-10.0, RefluxRequested, LREAL#10.0);
ReboilerApplied := LIMIT(LREAL#-10.0, ReboilerRequested, LREAL#10.0);
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
    P1.StaticQualified AT %MX24.2 : BOOL;
    P1.ExperimentEnabled AT %MX24.3 : BOOL;
    P1.SafetyAbort AT %MX24.4 : BOOL;
    P1.InputsValid AT %MX24.5 : BOOL;
    P1.RefluxApplied AT %QL0 : LREAL;
    P1.ReboilerApplied AT %QL8 : LREAL;
    P1.FeedDisturbanceApplied AT %QL16 : LREAL;
    P1.ProcessValue1 AT %IL0 : LREAL;
    P1.ProcessValue2 AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
