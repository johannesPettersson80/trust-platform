PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    UseMpc : BOOL := TRUE;
    ConfigurationValid : BOOL := TRUE;
    SolverComplete : BOOL := FALSE;
    SolverFeasible : BOOL := FALSE;
    Infeasible : BOOL := FALSE;
    FallbackActive : BOOL := FALSE;
    ConstraintActive : BOOL := FALSE;
    CandidateCount : DINT := 0;
    FeasibleCandidateCount : DINT := 0;
    BestObjective : LREAL := 0.0;
    Zone1Setpoint : LREAL := 21.0;
    Zone2Setpoint : LREAL := 21.0;
    ComfortMinimum : LREAL := 20.0;
    ComfortMaximum : LREAL := 22.5;
    TemperatureWeight : LREAL := 100.0;
    EnergyWeight : LREAL := 0.01;
    MoveWeight : LREAL := 0.0025;
    Zone1HeatRequested : LREAL := 80.0;
    Zone2HeatRequested : LREAL := 80.0;
    PreviousZone1Heat : LREAL := 80.0;
    PreviousZone2Heat : LREAL := 80.0;
    OutsideCurrent : LREAL := 5.0;
    Occupancy1Current : LREAL := 0.0;
    Occupancy2Current : LREAL := 0.0;
    OutsideForecast1 : LREAL := 5.0;
    OutsideForecast2 : LREAL := 5.0;
    OutsideForecast3 : LREAL := 5.0;
    Occupancy1Forecast1 : LREAL := 0.0;
    Occupancy1Forecast2 : LREAL := 0.0;
    Occupancy1Forecast3 : LREAL := 0.0;
    Occupancy2Forecast1 : LREAL := 0.0;
    Occupancy2Forecast2 : LREAL := 0.0;
    Occupancy2Forecast3 : LREAL := 0.0;
    TariffForecast1 : LREAL := 0.1;
    TariffForecast2 : LREAL := 0.1;
    TariffForecast3 : LREAL := 0.1;
    Zone1HeatApplied : LREAL := 0.0;
    Zone2HeatApplied : LREAL := 0.0;
    OutsideApplied : LREAL := 5.0;
    Occupancy1Applied : LREAL := 0.0;
    Occupancy2Applied : LREAL := 0.0;
    Zone1Temperature : LREAL := 21.0;
    Zone2Temperature : LREAL := 21.0;
    Mpc : FiniteGridMpcController;
    MpcRequest : BuildingMpcRequest;
    MpcResult : BuildingMpcResult;
    Zone1Error : LREAL := 0.0;
    Zone2Error : LREAL := 0.0;
    PreviousZone1Error : LREAL := 0.0;
    PreviousZone2Error : LREAL := 0.0;
    FallbackHeat1 : LREAL := 0.0;
    FallbackHeat2 : LREAL := 0.0;
    SharedScale : LREAL := 1.0;
    Initialized : BOOL := FALSE;
END_VAR

// Start from the occupied comfort condition used by the lesson experiment.
IF NOT Initialized THEN
    Enabled := TRUE;
    Zone1Setpoint := LREAL#21.0;
    Zone2Setpoint := LREAL#21.0;
    OutsideCurrent := LREAL#5.0;
    Initialized := TRUE;
END_IF;

MpcRequest.Enable := UseMpc;
MpcRequest.UpstreamValid := TRUE;
MpcRequest.Zone1Temperature := Zone1Temperature;
MpcRequest.Zone2Temperature := Zone2Temperature;
MpcRequest.Zone1Setpoint := Zone1Setpoint;
MpcRequest.Zone2Setpoint := Zone2Setpoint;
MpcRequest.ComfortMinimum := ComfortMinimum;
MpcRequest.ComfortMaximum := ComfortMaximum;
MpcRequest.TemperatureWeight := TemperatureWeight;
MpcRequest.EnergyWeight := EnergyWeight;
MpcRequest.MoveWeight := MoveWeight;
MpcRequest.PreviousZone1Heat := PreviousZone1Heat;
MpcRequest.PreviousZone2Heat := PreviousZone2Heat;
MpcRequest.OutsideCurrent := OutsideCurrent;
MpcRequest.Occupancy1Current := Occupancy1Current;
MpcRequest.Occupancy2Current := Occupancy2Current;
MpcRequest.EstimatedBias1 := LREAL#0.0;
MpcRequest.EstimatedBias2 := LREAL#0.0;
MpcRequest.Forecast.OutsideTemperature[0] := OutsideForecast1;
MpcRequest.Forecast.OutsideTemperature[1] := OutsideForecast2;
MpcRequest.Forecast.OutsideTemperature[2] := OutsideForecast3;
MpcRequest.Forecast.Zone1OccupancyGain[0] := Occupancy1Forecast1;
MpcRequest.Forecast.Zone1OccupancyGain[1] := Occupancy1Forecast2;
MpcRequest.Forecast.Zone1OccupancyGain[2] := Occupancy1Forecast3;
MpcRequest.Forecast.Zone2OccupancyGain[0] := Occupancy2Forecast1;
MpcRequest.Forecast.Zone2OccupancyGain[1] := Occupancy2Forecast2;
MpcRequest.Forecast.Zone2OccupancyGain[2] := Occupancy2Forecast3;
MpcRequest.Forecast.Tariff[0] := TariffForecast1;
MpcRequest.Forecast.Tariff[1] := TariffForecast2;
MpcRequest.Forecast.Tariff[2] := TariffForecast3;
Mpc(Request := MpcRequest);
MpcResult := Mpc.Result;
ConfigurationValid := MpcResult.ConfigurationValid;
SolverComplete := MpcResult.SolverComplete;
SolverFeasible := MpcResult.SolverFeasible;
Infeasible := MpcResult.Infeasible;
CandidateCount := MpcResult.CandidateCount;
FeasibleCandidateCount := MpcResult.FeasibleCandidateCount;
BestObjective := MpcResult.BestObjective;
// The PI pair is calculated every scan so fallback uses fresh measurements.
Zone1Error := Zone1Setpoint - Zone1Temperature;
Zone2Error := Zone2Setpoint - Zone2Temperature;
FallbackHeat1 := PreviousZone1Heat
    + LREAL#20.0 * (Zone1Error - PreviousZone1Error)
    + LREAL#0.02 * LREAL#60.0 * Zone1Error;
FallbackHeat2 := PreviousZone2Heat
    + LREAL#20.0 * (Zone2Error - PreviousZone2Error)
    + LREAL#0.02 * LREAL#60.0 * Zone2Error;
FallbackHeat1 := LIMIT(
    MAX(LREAL#0.0, PreviousZone1Heat - LREAL#20.0),
    FallbackHeat1,
    MIN(LREAL#100.0, PreviousZone1Heat + LREAL#20.0)
);
FallbackHeat2 := LIMIT(
    MAX(LREAL#0.0, PreviousZone2Heat - LREAL#20.0),
    FallbackHeat2,
    MIN(LREAL#100.0, PreviousZone2Heat + LREAL#20.0)
);
IF FallbackHeat1 + FallbackHeat2 > LREAL#170.0 THEN
    SharedScale := LREAL#170.0 / (FallbackHeat1 + FallbackHeat2);
    FallbackHeat1 := SharedScale * FallbackHeat1;
    FallbackHeat2 := SharedScale * FallbackHeat2;
END_IF;


FallbackActive := UseMpc AND ((NOT ConfigurationValid) OR (NOT SolverFeasible));
IF UseMpc AND SolverFeasible THEN
    Zone1HeatRequested := MpcResult.FirstZone1Heat;
    Zone2HeatRequested := MpcResult.FirstZone2Heat;
ELSE
    Zone1HeatRequested := FallbackHeat1;
    Zone2HeatRequested := FallbackHeat2;
    IF NOT SolverFeasible THEN
        BestObjective := LREAL#0.0;
    END_IF;
END_IF;

ConstraintActive := (Zone1HeatRequested <= LREAL#0.0)
    OR (Zone1HeatRequested >= LREAL#100.0)
    OR (Zone2HeatRequested <= LREAL#0.0)
    OR (Zone2HeatRequested >= LREAL#100.0)
    OR (Zone1HeatRequested + Zone2HeatRequested >= LREAL#169.999999)
    OR (ABS(Zone1HeatRequested - PreviousZone1Heat) >= LREAL#19.999999)
    OR (ABS(Zone2HeatRequested - PreviousZone2Heat) >= LREAL#19.999999);

OutsideApplied := LIMIT(LREAL#-10.0, OutsideCurrent, LREAL#15.0);
Occupancy1Applied := LIMIT(LREAL#0.0, Occupancy1Current, LREAL#4.0);
Occupancy2Applied := LIMIT(LREAL#0.0, Occupancy2Current, LREAL#4.0);
IF Enabled THEN
    Zone1HeatApplied := LIMIT(LREAL#0.0, Zone1HeatRequested, LREAL#100.0);
    Zone2HeatApplied := LIMIT(LREAL#0.0, Zone2HeatRequested, LREAL#100.0);
ELSE
    Zone1HeatApplied := LREAL#0.0;
    Zone2HeatApplied := LREAL#0.0;
END_IF;

PreviousZone1Heat := Zone1HeatApplied;
PreviousZone2Heat := Zone2HeatApplied;
PreviousZone1Error := Zone1Error;
PreviousZone2Error := Zone2Error;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.CandidateCount AT %MD0 : DINT;
    P1.FeasibleCandidateCount AT %MD4 : DINT;
    P1.BestObjective AT %ML8 : LREAL;
    P1.Zone1Setpoint AT %ML16 : LREAL;
    P1.Zone2Setpoint AT %ML24 : LREAL;
    P1.ComfortMinimum AT %ML32 : LREAL;
    P1.ComfortMaximum AT %ML40 : LREAL;
    P1.TemperatureWeight AT %ML48 : LREAL;
    P1.EnergyWeight AT %ML56 : LREAL;
    P1.MoveWeight AT %ML64 : LREAL;
    P1.Zone1HeatRequested AT %ML72 : LREAL;
    P1.Zone2HeatRequested AT %ML80 : LREAL;
    P1.PreviousZone1Heat AT %ML88 : LREAL;
    P1.PreviousZone2Heat AT %ML96 : LREAL;
    P1.OutsideCurrent AT %ML104 : LREAL;
    P1.Occupancy1Current AT %ML112 : LREAL;
    P1.Occupancy2Current AT %ML120 : LREAL;
    P1.OutsideForecast1 AT %ML128 : LREAL;
    P1.OutsideForecast2 AT %ML136 : LREAL;
    P1.OutsideForecast3 AT %ML144 : LREAL;
    P1.Occupancy1Forecast1 AT %ML152 : LREAL;
    P1.Occupancy1Forecast2 AT %ML160 : LREAL;
    P1.Occupancy1Forecast3 AT %ML168 : LREAL;
    P1.Occupancy2Forecast1 AT %ML176 : LREAL;
    P1.Occupancy2Forecast2 AT %ML184 : LREAL;
    P1.Occupancy2Forecast3 AT %ML192 : LREAL;
    P1.TariffForecast1 AT %ML200 : LREAL;
    P1.TariffForecast2 AT %ML208 : LREAL;
    P1.TariffForecast3 AT %ML216 : LREAL;
    P1.Enabled AT %MX240.0 : BOOL;
    P1.UseMpc AT %MX240.1 : BOOL;
    P1.ConfigurationValid AT %MX240.2 : BOOL;
    P1.SolverComplete AT %MX240.3 : BOOL;
    P1.SolverFeasible AT %MX240.4 : BOOL;
    P1.Infeasible AT %MX240.5 : BOOL;
    P1.FallbackActive AT %MX240.6 : BOOL;
    P1.ConstraintActive AT %MX240.7 : BOOL;
    P1.Zone1HeatApplied AT %QL0 : LREAL;
    P1.Zone2HeatApplied AT %QL8 : LREAL;
    P1.OutsideApplied AT %QL16 : LREAL;
    P1.Occupancy1Applied AT %QL24 : LREAL;
    P1.Occupancy2Applied AT %QL32 : LREAL;
    P1.Zone1Temperature AT %IL0 : LREAL;
    P1.Zone2Temperature AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
