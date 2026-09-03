PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    UseMpc : BOOL := TRUE;
    OperatingMode : DINT := DINT#1;
    QualityGood : BOOL := TRUE;
    Heater1Available : BOOL := TRUE;
    Heater2Available : BOOL := TRUE;
    ManualZone1Heat : LREAL := 0.0;
    ManualZone2Heat : LREAL := 0.0;
    EstimatorInitialized : BOOL := FALSE;
    EstimatorValid : BOOL := FALSE;
    PreviousPrediction1 : LREAL := 21.0;
    PreviousPrediction2 : LREAL := 21.0;
    Innovation1 : LREAL := 0.0;
    Innovation2 : LREAL := 0.0;
    EstimatedBias1 : LREAL := 0.0;
    EstimatedBias2 : LREAL := 0.0;
    EstimatorGain : LREAL := 0.4;
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
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL := FALSE;
    ControllerRevision : DINT := DINT#83;
    StartupInitialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : DINT := DINT#0;
    ExperimentElapsedSeconds : LREAL := 0.0;
END_VAR

// The simulated capstone starts from its reviewed automatic operating state.
IF NOT StartupInitialized THEN
    Enabled := FALSE;
    OperatingMode := DINT#1;
    Zone1Setpoint := LREAL#21.0;
    Zone2Setpoint := LREAL#21.0;
    StartupInitialized := TRUE;
END_IF;
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentScan := ExperimentScan + DINT#1;
    ExperimentElapsedSeconds := ExperimentElapsedSeconds + LREAL#60.0;
    Enabled := TRUE;
    UseMpc := TRUE;
    OperatingMode := DINT#1;
    QualityGood := TRUE;
    Heater1Available := TRUE;
    Heater2Available := TRUE;
    ComfortMinimum := LREAL#20.0;
    ComfortMaximum := LREAL#22.5;
    TemperatureWeight := LREAL#100.0;
    EnergyWeight := LREAL#0.01;
    MoveWeight := LREAL#0.0025;
    OutsideCurrent := LREAL#5.0;
    Occupancy1Current := LREAL#0.0;
    Occupancy2Current := LREAL#0.0;
    TariffForecast1 := LREAL#0.1;
    TariffForecast2 := LREAL#0.1;
    TariffForecast3 := LREAL#0.1;
    IF ExperimentScan >= DINT#6 THEN OutsideCurrent := LREAL#4.0; END_IF;
    IF ExperimentScan >= DINT#8 THEN
        Occupancy1Current := LREAL#2.0;
        TariffForecast1 := LREAL#0.6;
        TariffForecast2 := LREAL#0.6;
        TariffForecast3 := LREAL#0.6;
    END_IF;
    IF ExperimentScan >= DINT#10 THEN Occupancy2Current := LREAL#1.0; END_IF;
    IF ExperimentScan >= DINT#13 THEN
        TariffForecast1 := LREAL#0.15;
        TariffForecast2 := LREAL#0.15;
        TariffForecast3 := LREAL#0.15;
    END_IF;
    IF ExperimentScan = DINT#14 THEN
        ComfortMinimum := LREAL#22.0;
    ELSIF ExperimentScan = DINT#15 THEN
        TemperatureWeight := LREAL#-1.0;
    ELSIF ExperimentScan = DINT#16 THEN
        QualityGood := FALSE;
    ELSIF ExperimentScan = DINT#17 THEN
        Heater1Available := FALSE;
    ELSIF ExperimentScan = DINT#18 THEN
        OperatingMode := DINT#2;
        ManualZone1Heat := LREAL#60.0;
        ManualZone2Heat := LREAL#60.0;
    ELSIF ExperimentScan = DINT#19 THEN
        OperatingMode := DINT#3;
    END_IF;
    OutsideForecast1 := OutsideCurrent;
    OutsideForecast2 := OutsideCurrent;
    OutsideForecast3 := OutsideCurrent;
    Occupancy1Forecast1 := Occupancy1Current;
    Occupancy1Forecast2 := Occupancy1Current;
    Occupancy1Forecast3 := Occupancy1Current;
    Occupancy2Forecast1 := Occupancy2Current;
    Occupancy2Forecast2 := Occupancy2Current;
    Occupancy2Forecast3 := Occupancy2Current;
ELSE
    Enabled := FALSE;
    OperatingMode := DINT#0;
    QualityGood := TRUE;
    Heater1Available := TRUE;
    Heater2Available := TRUE;
    OutsideCurrent := LREAL#5.0;
    Occupancy1Current := LREAL#0.0;
    Occupancy2Current := LREAL#0.0;
    ManualZone1Heat := LREAL#0.0;
    ManualZone2Heat := LREAL#0.0;
    ExperimentScan := DINT#0;
    ExperimentElapsedSeconds := LREAL#0.0;
END_IF;
AutoMode := Enabled AND (OperatingMode = DINT#1);
ControllerRevision := DINT#83;
EstimatorGain := LREAL#0.4;
FaultActive := (NOT QualityGood) OR (NOT Heater1Available)
    OR (NOT Heater2Available);
IF NOT EstimatorInitialized THEN
    IF QualityGood
        AND (Zone1Temperature >= LREAL#15.0)
        AND (Zone1Temperature <= LREAL#27.0)
        AND (Zone2Temperature >= LREAL#15.0)
        AND (Zone2Temperature <= LREAL#27.0)
    THEN
        PreviousPrediction1 := Zone1Temperature;
        PreviousPrediction2 := Zone2Temperature;
        Innovation1 := LREAL#0.0;
        Innovation2 := LREAL#0.0;
        EstimatedBias1 := LREAL#0.0;
        EstimatedBias2 := LREAL#0.0;
        EstimatorInitialized := TRUE;
        EstimatorValid := TRUE;
    ELSE
        EstimatorValid := FALSE;
    END_IF;
ELSIF QualityGood THEN
    Innovation1 := Zone1Temperature - PreviousPrediction1;
    Innovation2 := Zone2Temperature - PreviousPrediction2;
    EstimatorValid := (ABS(Innovation1) <= LREAL#2.0)
        AND (ABS(Innovation2) <= LREAL#2.0);
    IF EstimatorValid THEN
        (* The retained correction represents unmeasured net heat per sample;
           every horizon state uses the same qualified local bias estimate. *)
        EstimatedBias1 := LIMIT(
            LREAL#-1.0,
            EstimatedBias1 + EstimatorGain * Innovation1,
            LREAL#1.0
        );
        EstimatedBias2 := LIMIT(
            LREAL#-1.0,
            EstimatedBias2 + EstimatorGain * Innovation2,
            LREAL#1.0
        );
    ELSE
        EstimatorInitialized := FALSE;
    END_IF;
ELSE
    EstimatorValid := FALSE;
    EstimatorInitialized := FALSE;
END_IF;

MpcRequest.Enable := UseMpc;
MpcRequest.UpstreamValid := EstimatorValid AND (NOT FaultActive);
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
MpcRequest.EstimatedBias1 := EstimatedBias1;
MpcRequest.EstimatedBias2 := EstimatedBias2;
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

TrackingActive := FaultActive OR (NOT Enabled) OR (OperatingMode <> DINT#1);
IF FaultActive OR (OperatingMode = DINT#0) OR (OperatingMode < DINT#0)
    OR (OperatingMode > DINT#3)
THEN
    Zone1HeatRequested := LREAL#0.0;
    Zone2HeatRequested := LREAL#0.0;
    Zone1HeatApplied := LREAL#0.0;
    Zone2HeatApplied := LREAL#0.0;
ELSIF OperatingMode = DINT#2 THEN
    Zone1HeatRequested := LIMIT(LREAL#0.0, ManualZone1Heat, LREAL#100.0);
    Zone2HeatRequested := LIMIT(LREAL#0.0, ManualZone2Heat, LREAL#100.0);
    Zone1HeatApplied := LIMIT(
        MAX(LREAL#0.0, PreviousZone1Heat - LREAL#20.0),
        Zone1HeatRequested,
        MIN(LREAL#100.0, PreviousZone1Heat + LREAL#20.0)
    );
    Zone2HeatApplied := LIMIT(
        MAX(LREAL#0.0, PreviousZone2Heat - LREAL#20.0),
        Zone2HeatRequested,
        MIN(LREAL#100.0, PreviousZone2Heat + LREAL#20.0)
    );
ELSIF OperatingMode = DINT#3 THEN
    Zone1HeatRequested := LREAL#0.0;
    Zone2HeatRequested := LREAL#0.0;
    Zone1HeatApplied := MAX(LREAL#0.0, PreviousZone1Heat - LREAL#20.0);
    Zone2HeatApplied := MAX(LREAL#0.0, PreviousZone2Heat - LREAL#20.0);
ELSIF Enabled THEN
    Zone1HeatApplied := LIMIT(
        MAX(LREAL#0.0, PreviousZone1Heat - LREAL#20.0),
        Zone1HeatRequested,
        MIN(LREAL#100.0, PreviousZone1Heat + LREAL#20.0)
    );
    Zone2HeatApplied := LIMIT(
        MAX(LREAL#0.0, PreviousZone2Heat - LREAL#20.0),
        Zone2HeatRequested,
        MIN(LREAL#100.0, PreviousZone2Heat + LREAL#20.0)
    );
ELSE
    Zone1HeatApplied := LREAL#0.0;
    Zone2HeatApplied := LREAL#0.0;
END_IF;

IF Zone1HeatApplied + Zone2HeatApplied > LREAL#170.0 THEN
    (* Common scaling preserves the requested heating direction while enforcing
       the shared hydronic capacity limit. *)
    SharedScale := LREAL#170.0 / (Zone1HeatApplied + Zone2HeatApplied);
    Zone1HeatApplied := SharedScale * Zone1HeatApplied;
    Zone2HeatApplied := SharedScale * Zone2HeatApplied;
ELSE
    SharedScale := LREAL#1.0;
END_IF;

(* Retain the prediction made from the command that the plant actually
   receives. The next innovation can then reveal unmeasured thermal load. *)
PreviousPrediction1 := LREAL#0.875659571463 * Zone1Temperature
    + LREAL#0.029177846573 * Zone2Temperature
    + LREAL#0.018727354703 * Zone1HeatApplied
    + LREAL#0.000305161690 * Zone2HeatApplied
    + LREAL#0.095162581964 * OutsideApplied
    + LREAL#0.093636773515 * Occupancy1Applied
    + LREAL#0.001525808449 * Occupancy2Applied
    + EstimatedBias1;
PreviousPrediction2 := LREAL#0.029177846573 * Zone1Temperature
    + LREAL#0.875659571463 * Zone2Temperature
    + LREAL#0.000305161690 * Zone1HeatApplied
    + LREAL#0.018727354703 * Zone2HeatApplied
    + LREAL#0.095162581964 * OutsideApplied
    + LREAL#0.001525808449 * Occupancy1Applied
    + LREAL#0.093636773515 * Occupancy2Applied
    + EstimatedBias2;

PreviousZone1Heat := Zone1HeatApplied;
PreviousZone2Heat := Zone2HeatApplied;
PreviousZone1Error := Zone1Error;
PreviousZone2Error := Zone2Error;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#60s, PRIORITY := 1);
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
    P1.OperatingMode AT %MD224 : DINT;
    P1.QualityGood AT %MX241.0 : BOOL;
    P1.Heater1Available AT %MX241.1 : BOOL;
    P1.Heater2Available AT %MX241.2 : BOOL;
    P1.EstimatorInitialized AT %MX241.3 : BOOL;
    P1.EstimatorValid AT %MX241.4 : BOOL;
    P1.TrackingActive AT %MX241.5 : BOOL;
    P1.FaultActive AT %MX241.6 : BOOL;
    P1.AutoMode AT %MX241.7 : BOOL;
    P1.ExperimentEnabled AT %MX242.0 : BOOL;
    P1.SafetyAbort AT %MX242.1 : BOOL;
    P1.InputsValid AT %MX242.2 : BOOL;
    P1.ExperimentElapsedSeconds AT %ML328 : LREAL;
    P1.ManualZone1Heat AT %ML248 : LREAL;
    P1.ManualZone2Heat AT %ML256 : LREAL;
    P1.PreviousPrediction1 AT %ML264 : LREAL;
    P1.PreviousPrediction2 AT %ML272 : LREAL;
    P1.Innovation1 AT %ML280 : LREAL;
    P1.Innovation2 AT %ML288 : LREAL;
    P1.EstimatedBias1 AT %ML296 : LREAL;
    P1.EstimatedBias2 AT %ML304 : LREAL;
    P1.EstimatorGain AT %ML312 : LREAL;
    P1.ControllerRevision AT %MD320 : DINT;
    P1.Zone1HeatApplied AT %QL0 : LREAL;
    P1.Zone2HeatApplied AT %QL8 : LREAL;
    P1.OutsideApplied AT %QL16 : LREAL;
    P1.Occupancy1Applied AT %QL24 : LREAL;
    P1.Occupancy2Applied AT %QL32 : LREAL;
    P1.Zone1Temperature AT %IL0 : LREAL;
    P1.Zone2Temperature AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
