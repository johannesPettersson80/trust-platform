USING Control;

FUNCTION Lesson64ControlAuthority : BOOL
VAR_INPUT
    AutomaticMode : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
(* Preserve the current behavior for the red test: mapped Auto mode alone
   controls the PID. *)
Lesson64ControlAuthority := AutomaticMode AND InputsValid AND (NOT SafetyAbort);
END_FUNCTION

FUNCTION Lesson64ProportionalGain : LREAL
VAR_INPUT
    TimeConstantSeconds : LREAL;
    ProcessGain : LREAL;
    DesignTimeSeconds : LREAL;
END_VAR
Lesson64ProportionalGain := TimeConstantSeconds
    / (ProcessGain * DesignTimeSeconds);
END_FUNCTION

FUNCTION Lesson64IntegralGain : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    IntegralTimeSeconds : LREAL;
END_VAR
Lesson64IntegralGain := ProportionalGain / IntegralTimeSeconds;
END_FUNCTION

FUNCTION Lesson64ReviewedOverheadSetpoint : LREAL
VAR_INPUT
    ExperimentElapsedSeconds : LREAL;
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND (NOT SafetyAbort)
    AND (ExperimentElapsedSeconds >= LREAL#20.0) THEN
    Lesson64ReviewedOverheadSetpoint := LREAL#2.0;
ELSE
    Lesson64ReviewedOverheadSetpoint := LREAL#0.0;
END_IF;
END_FUNCTION

FUNCTION Lesson64DynamicRefluxRequest : LREAL
VAR_INPUT
    OverheadCorrection : LREAL;
    BottomsCorrection : LREAL;
    FilteredBottomsCorrection : LREAL;
END_VAR
Lesson64DynamicRefluxRequest := OverheadCorrection
    + LREAL#0.285714285714286 * BottomsCorrection
    + LREAL#0.214285714285714 * FilteredBottomsCorrection;
END_FUNCTION

FUNCTION Lesson64DynamicReboilerRequest : LREAL
VAR_INPUT
    BottomsCorrection : LREAL;
    OverheadCorrection : LREAL;
    FilteredOverheadCorrection : LREAL;
END_VAR
Lesson64DynamicReboilerRequest := BottomsCorrection
    + LREAL#0.321428571428571 * OverheadCorrection
    + LREAL#0.107142857142857 * FilteredOverheadCorrection;
END_FUNCTION

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
    OverheadParameters.ProportionalGain := Lesson64ProportionalGain(
        TimeConstantSeconds := LREAL#20.0,
        ProcessGain := LREAL#0.8,
        DesignTimeSeconds := LREAL#15.0);
    OverheadParameters.IntegralGain := Lesson64IntegralGain(
        ProportionalGain := OverheadParameters.ProportionalGain,
        IntegralTimeSeconds := LREAL#20.0);
    OverheadParameters.ProportionalSetpointWeight := LREAL#1.0;
    OverheadParameters.OutputMinimum := LREAL#-8.0;
    OverheadParameters.OutputMaximum := LREAL#8.0;
    OverheadParameters.RateMinimum := LREAL#-2.0;
    OverheadParameters.RateMaximum := LREAL#2.0;
    OverheadParameters.TrackingTime := LREAL#1.0;
    OverheadParameters.AntiWindup := PIDAntiWindup#Combined;
    BottomsParameters := OverheadParameters;
    BottomsParameters.ProportionalGain := Lesson64ProportionalGain(
        TimeConstantSeconds := LREAL#30.0,
        ProcessGain := LREAL#0.7,
        DesignTimeSeconds := LREAL#20.0);
    BottomsParameters.IntegralGain := Lesson64IntegralGain(
        ProportionalGain := BottomsParameters.ProportionalGain,
        IntegralTimeSeconds := LREAL#30.0);
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
   Studio grants authority for that run. The focused ST tests independently
   check the configured decoupler and the lesson-owned schedule. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    Architecture := 2;
    Setpoint2 := LREAL#0.0;
    FeedDisturbanceRequested := LREAL#0.0;
ELSE
    ExperimentElapsed := LREAL#0.0;
    Architecture := 0;
    Setpoint1 := LREAL#0.0;
    Setpoint2 := LREAL#0.0;
    FeedDisturbanceRequested := LREAL#0.0;
END_IF;
Setpoint1 := Lesson64ReviewedOverheadSetpoint(
    ExperimentElapsedSeconds := ExperimentElapsed,
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort);

OverheadInput.Setpoint := Setpoint1;
OverheadInput.ProcessValue := ProcessValue1;
OverheadInput.CycleTime := LREAL#0.1;
BottomsInput.Setpoint := Setpoint2;
BottomsInput.ProcessValue := ProcessValue2;
BottomsInput.CycleTime := LREAL#0.1;
IF Lesson64ControlAuthority(
    AutomaticMode := AutoMode1,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort) THEN
    OverheadInput.Mode := PIDMode#Auto;
ELSE
    OverheadInput.Mode := PIDMode#Manual;
    OverheadInput.ManualOutput := LREAL#0.0;
END_IF;
IF Lesson64ControlAuthority(
    AutomaticMode := AutoMode2,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort) THEN
    BottomsInput.Mode := PIDMode#Auto;
ELSE
    BottomsInput.Mode := PIDMode#Manual;
    BottomsInput.ManualOutput := LREAL#0.0;
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
    RefluxRequested := Lesson64DynamicRefluxRequest(
        OverheadCorrection := VirtualCommand.First,
        BottomsCorrection := VirtualCommand.Second,
        FilteredBottomsCorrection := FilteredBottomsCorrection);
    ReboilerRequested := Lesson64DynamicReboilerRequest(
        BottomsCorrection := VirtualCommand.Second,
        OverheadCorrection := VirtualCommand.First,
        FilteredOverheadCorrection := FilteredOverheadCorrection);
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
