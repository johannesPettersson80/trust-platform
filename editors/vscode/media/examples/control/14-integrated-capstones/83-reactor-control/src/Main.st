PROGRAM Main
VAR
    Controller : ReactorTrain;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ManualRequested : BOOL := FALSE;
    ControllerAuto : BOOL := FALSE;
    UseCascade : BOOL := TRUE;
    TrialKind : INT := 0; (* 0:target35→37; 1:jacket extraction30kW; 2:no event. *)
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    VesselC : LREAL := 35.0;
    JacketC : LREAL := 38.0;
    SetpointC : LREAL := 35.0;
    JacketSetpointC : LREAL := 38.0;
    AppliedPowerPct : LREAL := 15.0;
    NominalPowerPct : LREAL := 15.0;
    JacketExtractionKw : LREAL := 0.0;
    OuterIntegralC : LREAL := 38.0;
    HeaterIntegralPct : LREAL := 15.0;
    HeaterProportionalPct : LREAL := 0.0;
END_VAR
IF NOT Initialized THEN Controller.Configure(); Initialized := TRUE; END_IF;
InputsValid := VesselC >= LREAL#30.0 AND VesselC <= LREAL#40.0
    AND JacketC >= LREAL#20.0 AND JacketC <= LREAL#55.0;
IF NOT InputsValid THEN SafetyAbort := TRUE; END_IF;
SetpointC := LREAL#35.0;
JacketExtractionKw := LREAL#0.0;
ControllerAuto := ExperimentEnabled AND NOT ManualRequested AND NOT SafetyAbort;
IF ControllerAuto THEN
    CASE TrialKind OF
        0: SetpointC := LREAL#37.0;
        1: JacketExtractionKw := LREAL#30.0;
    END_CASE;
END_IF;
Controller.Update(SetpointC := SetpointC, VesselC := VesselC, JacketC := JacketC,
    UseCascade := UseCascade, Automatic := ControllerAuto, Faulted := SafetyAbort);
AppliedPowerPct := Controller.AppliedPowerPct;
NominalPowerPct := Controller.NominalPowerPct;
JacketSetpointC := Controller.SelectedJacketC;
OuterIntegralC := Controller.OuterIntegralC;
HeaterIntegralPct := Controller.HeaterIntegralPct;
HeaterProportionalPct := Controller.HeaterProportionalPct;
IF SafetyAbort THEN AppliedPowerPct := LREAL#0.0; JacketExtractionKw := LREAL#0.0; END_IF;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX240.5 : BOOL;
    P1.ManualRequested AT %MX240.4 : BOOL;
    P1.ControllerAuto AT %MX240.3 : BOOL;
    P1.SafetyAbort AT %MX240.6 : BOOL;
    P1.InputsValid AT %MX240.7 : BOOL;
    P1.TrialKind AT %MW192 : INT;
    P1.VesselC AT %IL0 : LREAL;
    P1.JacketC AT %IL8 : LREAL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.JacketExtractionKw AT %QL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.JacketSetpointC AT %ML8 : LREAL;
    P1.HeaterIntegralPct AT %ML16 : LREAL;
    P1.HeaterProportionalPct AT %ML24 : LREAL;
    P1.OuterIntegralC AT %ML32 : LREAL;
    P1.NominalPowerPct AT %ML40 : LREAL;
END_VAR
END_CONFIGURATION
