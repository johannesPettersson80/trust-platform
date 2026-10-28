PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    HasStarted : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    Initialized : BOOL := FALSE;
    TemperatureC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 75.0;
    ModelTemperatureC : REAL := 35.0;
    ThermalCapacitance : REAL := 50.0;
    EffectiveHeatLoss : REAL := 10.0;
    HeaterPowerKw : REAL := 0.0;
    LossPowerKw : REAL := 150.0;
    NetPowerKw : REAL := 0.0;
    ModelRateCPerS : REAL := 0.0;
    DelayedCommandPct : REAL := 75.0;
    CommandHistory : ARRAY[0..9] OF REAL;
    HistoryIndex : INT := 0;
    InitializeIndex : INT := 0;
END_VAR
IF NOT Initialized THEN
    ModelTemperatureC := REAL#35.0;
    FOR InitializeIndex := 0 TO 9 DO CommandHistory[InitializeIndex] := REAL#75.0; END_FOR;
    Initialized := TRUE;
END_IF;
IF TemperatureC <> TemperatureC OR TemperatureC < LREAL#32.0
    OR TemperatureC > LREAL#39.0 THEN SafetyAbort := TRUE;
END_IF;
IF ExperimentEnabled AND NOT SafetyAbort THEN
    HasStarted := TRUE;
    AppliedPowerPct := LREAL#85.0;
ELSIF NOT HasStarted AND NOT SafetyAbort THEN AppliedPowerPct := LREAL#75.0;
ELSE AppliedPowerPct := LREAL#0.0;
END_IF;
(* Provided one-second command-history interface for the model calculation.
   The physical simulator retains its own independent delivery history. *)
DelayedCommandPct := CommandHistory[HistoryIndex];
CommandHistory[HistoryIndex] := LREAL_TO_REAL(AppliedPowerPct);
HistoryIndex := (HistoryIndex + 1) MOD 10;
(* Learner energy balance: model state is never written to the PV input. *)
HeaterPowerKw := REAL#2.0 * DelayedCommandPct;
LossPowerKw := EffectiveHeatLoss * (ModelTemperatureC - REAL#20.0);
NetPowerKw := HeaterPowerKw - LossPowerKw;
IF ThermalCapacitance > REAL#0.0 AND EffectiveHeatLoss > REAL#0.0 THEN
    ModelRateCPerS := NetPowerKw / ThermalCapacitance;
    ModelTemperatureC := ModelTemperatureC + REAL#0.1 * ModelRateCPerS;
ELSE SafetyAbort := TRUE; AppliedPowerPct := LREAL#0.0;
END_IF;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX40.2 : BOOL;
    P1.SafetyAbort AT %MX40.1 : BOOL;
    P1.ModelTemperatureC AT %MD0 : REAL;
    P1.HeaterPowerKw AT %MD4 : REAL;
    P1.LossPowerKw AT %MD8 : REAL;
    P1.ModelRateCPerS AT %MD12 : REAL;
    P1.DelayedCommandPct AT %MD16 : REAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
