PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    TankAreaSquareMetres : LREAL := 0.5;
    TankInitialLevelCentimetres : LREAL := 50.0;
    TankInletFlow : LREAL := 10.0;
    TankOutletFlow : LREAL := 10.0;
    TankNetFlowLitresPerSecond : LREAL := 0.0;
    TankNetFlowCubicMetresPerSecond : LREAL := 0.0;
    TankPredictedRate : LREAL := 0.0;
    TankPredictedLevelAfter20Seconds : LREAL := 0.0;
    TankNetFlowCommand : LREAL := 0.0;
    TankLevel : LREAL := 50.0;

    HeaterThermalCapacitance : LREAL := 50.0;
    HeaterHeatLossCoefficient : LREAL := 10.0;
    HeaterFullScalePower : LREAL := 200.0;
    HeaterAmbientTemperature : LREAL := 20.0;
    HeaterCommandRequest : LREAL := 0.0;
    HeaterAppliedCommand : LREAL := 0.0;
    HeaterPowerKilowatts : LREAL := 0.0;
    HeaterStaticGain : LREAL := 0.0;
    HeaterTimeConstant : LREAL := 0.0;
    HeaterPredictedEquilibrium : LREAL := 0.0;
    HeaterPredictedAtDelayPlusTimeConstant : LREAL := 0.0;
    HeaterTemperature : LREAL := 20.0;
    ModelValid : BOOL := FALSE;
END_VAR

(* Control Studio writes only this authority bit. The project owns the physical
   experiment inputs and restores them when authority is removed. *)
IF ExperimentEnabled THEN
    TankInletFlow := LREAL#12.0;
    TankOutletFlow := LREAL#10.0;
    HeaterCommandRequest := LREAL#20.0;
ELSIF ExperimentWasEnabled THEN
    TankInletFlow := LREAL#10.0;
    TankOutletFlow := LREAL#10.0;
    HeaterCommandRequest := LREAL#0.0;
END_IF;

InputsValid := TankLevel >= LREAL#0.0 AND TankLevel <= LREAL#100.0
    AND HeaterTemperature >= LREAL#0.0 AND HeaterTemperature <= LREAL#100.0;
IF SafetyAbort OR NOT InputsValid THEN
    TankInletFlow := LREAL#10.0;
    TankOutletFlow := LREAL#10.0;
    HeaterCommandRequest := LREAL#0.0;
END_IF;

ModelValid := (TankAreaSquareMetres > LREAL#0.0)
    AND (HeaterThermalCapacitance > LREAL#0.0)
    AND (HeaterHeatLossCoefficient > LREAL#0.0)
    AND (HeaterFullScalePower > LREAL#0.0);

IF ModelValid THEN
    (* Convert litres to cubic metres before applying the tank volume balance. *)
    TankNetFlowLitresPerSecond := TankInletFlow - TankOutletFlow;
    TankNetFlowCubicMetresPerSecond := TankNetFlowLitresPerSecond * LREAL#0.001;
    TankPredictedRate :=
        (TankNetFlowCubicMetresPerSecond / TankAreaSquareMetres) * LREAL#100.0;
    TankPredictedLevelAfter20Seconds :=
        TankInitialLevelCentimetres + TankPredictedRate * LREAL#20.0;
    TankNetFlowCommand := TankNetFlowLitresPerSecond;

    IF HeaterCommandRequest < LREAL#0.0 THEN
        HeaterAppliedCommand := LREAL#0.0;
    ELSIF HeaterCommandRequest > LREAL#100.0 THEN
        HeaterAppliedCommand := LREAL#100.0;
    ELSE
        HeaterAppliedCommand := HeaterCommandRequest;
    END_IF;

    (* These quantities follow directly from the vessel energy balance. *)
    HeaterPowerKilowatts :=
        HeaterFullScalePower * HeaterAppliedCommand / LREAL#100.0;
    HeaterStaticGain :=
        (HeaterFullScalePower / LREAL#100.0) / HeaterHeatLossCoefficient;
    HeaterTimeConstant :=
        HeaterThermalCapacitance / HeaterHeatLossCoefficient;
    HeaterPredictedEquilibrium :=
        HeaterAmbientTemperature + HeaterPowerKilowatts / HeaterHeatLossCoefficient;
    HeaterPredictedAtDelayPlusTimeConstant :=
        HeaterAmbientTemperature
        + LREAL#0.6321205588
        * (HeaterPredictedEquilibrium - HeaterAmbientTemperature);
ELSE
    TankNetFlowCommand := LREAL#0.0;
    HeaterAppliedCommand := LREAL#0.0;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX80.1 : BOOL;
    P1.SafetyAbort AT %MX80.2 : BOOL;
    P1.InputsValid AT %MX80.3 : BOOL;
    P1.TankInletFlow AT %ML0 : LREAL;
    P1.TankOutletFlow AT %ML8 : LREAL;
    P1.HeaterCommandRequest AT %ML16 : LREAL;
    P1.TankNetFlowCommand AT %QL0 : LREAL;
    P1.HeaterAppliedCommand AT %QL8 : LREAL;
    P1.TankLevel AT %IL0 : LREAL;
    P1.HeaterTemperature AT %IL8 : LREAL;
    P1.TankPredictedRate AT %ML32 : LREAL;
    P1.TankPredictedLevelAfter20Seconds AT %ML40 : LREAL;
    P1.HeaterPredictedEquilibrium AT %ML48 : LREAL;
    P1.HeaterPredictedAtDelayPlusTimeConstant AT %ML56 : LREAL;
    P1.HeaterTimeConstant AT %ML64 : LREAL;
    P1.ModelValid AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
