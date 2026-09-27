PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    TrialKind : INT := 0; (* 0:setpoint35→38; 1:30kW load at SP35. *)
    ZeroTimeS : LREAL := 3.0;
    PoleTimeS : LREAL := 2.0;
    ShapeStatePct : LREAL := 0.0;
    ShapeRatio : LREAL := 1.5;
    CommandDeviationPct : LREAL := 0.0;
    NoiseC : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    CandidateRevision : UDINT := 0; (* 0 is a nominal illustration, not a measured relay result. *)
    KpPctPerC : LREAL := 5.0;
    KiPctPerCs : LREAL := 1.0;
    DataUsable : BOOL := FALSE;
    AppliedPowerPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    DisturbanceKw : LREAL := 0.0;
    SetpointC : LREAL := 35.0;
    HeatingPathC : LREAL := 35.0;
    LoadPathC : LREAL := 0.0;
    WaterTemperatureC : LREAL := 35.0;
    ProbeStateC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    ErrorC : LREAL := 0.0;
    ProportionalPct : LREAL := 0.0;
    IntegralStatePct : LREAL := 75.0;
    IntegralPct : LREAL := 75.0;
    TrialIntegralPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
END_VAR
(* SIMULATION-ONLY superposition keeps the physical extraction outside the
   heater delivery delay. No sensor lag: the lesson explicitly bypasses it. *)
WaterTemperatureC := HeatingPathC + LoadPathC;
NoiseC := LREAL#0.0;
IF ExperimentEnabled THEN
    IF TrialKind = 2 THEN NoiseC := LREAL#0.05 * SIN(ElapsedS); END_IF;
ELSE ElapsedS := LREAL#0.0;
END_IF;
TemperatureC := WaterTemperatureC + NoiseC;
IF ZeroTimeS > LREAL#0.0 AND PoleTimeS > LREAL#0.0 THEN
    ShapeRatio := ZeroTimeS / PoleTimeS;
ELSE ShapeRatio := LREAL#1.0; END_IF;
DataUsable := KpPctPerC >= LREAL#0.0 AND KiPctPerCs >= LREAL#0.0
    AND ZeroTimeS > LREAL#0.0 AND PoleTimeS > LREAL#0.0
    AND TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
SetpointC := LREAL#35.0;
DisturbanceKw := LREAL#0.0;
IF ExperimentEnabled AND DataUsable AND NOT SafetyAbort THEN
    IF TrialKind = 0 THEN SetpointC := LREAL#38.0;
    ELSIF TrialKind = 1 THEN DisturbanceKw := LREAL#30.0;
    END_IF;
    ErrorC := SetpointC - TemperatureC;
    ProportionalPct := KpPctPerC * ErrorC;
    TrialIntegralPct := IntegralStatePct + KiPctPerCs * ErrorC * LREAL#0.1;
    CommandDeviationPct := ProportionalPct + TrialIntegralPct - LREAL#75.0;
    RequestedPowerPct := LREAL#75.0 + ShapeRatio * CommandDeviationPct
        + (LREAL#1.0 - ShapeRatio) * ShapeStatePct;
    (* Calculate the current request before advancing the shaping state. *)
    ShapeStatePct := ShapeStatePct + LREAL#0.1 / PoleTimeS
        * (CommandDeviationPct - ShapeStatePct);
    AppliedPowerPct := MAX(MIN(RequestedPowerPct, LREAL#100.0), LREAL#0.0);
    (* Same Lesson8 tracking order, Tt=1s: limit, then reconcile retained I. *)
    IntegralStatePct := TrialIntegralPct
        + LREAL#0.1 * (AppliedPowerPct - RequestedPowerPct) / ShapeRatio;
    CommandActivityPoints := CommandActivityPoints
        + ABS(AppliedPowerPct - PreviousAppliedPct);
    PreviousAppliedPct := AppliedPowerPct;
ELSE
    ShapeStatePct := LREAL#0.0;
    IntegralStatePct := LREAL#75.0;
    ProportionalPct := LREAL#0.0;
    RequestedPowerPct := LREAL#75.0;
    AppliedPowerPct := LREAL#75.0;
    PreviousAppliedPct := LREAL#75.0;
    (* Keep final activity visible after shutdown. Reset on the next enable. *)
END_IF;
IF NOT ExperimentWasEnabled AND ExperimentEnabled THEN
    CommandActivityPoints := ABS(AppliedPowerPct - LREAL#75.0);
END_IF;
IntegralPct := IntegralStatePct;
IF ExperimentEnabled THEN ElapsedS := ElapsedS + LREAL#0.1; END_IF;
ExperimentWasEnabled := ExperimentEnabled;
IF TemperatureC <> TemperatureC OR TemperatureC < LREAL#0.0
    OR TemperatureC > LREAL#80.0 THEN SafetyAbort := TRUE;
END_IF;
IF SafetyAbort THEN AppliedPowerPct := LREAL#0.0;
    NoiseC := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
END_IF;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.DataUsable AT %MX96.2 : BOOL;
    P1.TrialKind AT %MW80 : INT;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.HeatingPathC AT %IL0 : LREAL;
    P1.LoadPathC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.NoiseC AT %ML24 : LREAL;
    P1.ShapeStatePct AT %ML72 : LREAL;
    P1.IntegralPct AT %ML16 : LREAL;
    P1.RequestedPowerPct AT %ML32 : LREAL;
    P1.TemperatureC AT %ML40 : LREAL;
    P1.WaterTemperatureC AT %ML48 : LREAL;
    P1.ProportionalPct AT %ML56 : LREAL;
    P1.CommandActivityPoints AT %ML64 : LREAL;
END_VAR
END_CONFIGURATION
