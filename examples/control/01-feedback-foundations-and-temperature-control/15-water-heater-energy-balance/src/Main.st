PROGRAM Main
VAR
    (* These physical parameters are deliberately visible so the learner can
       derive the local FOPDT gain and time constant from equipment data. *)
    ThermalCapacitance : LREAL := 50.0;  (* kJ/degC *)
    EffectiveHeatLoss : LREAL := 10.0;   (* kW/degC *)
    FullScaleHeaterPower : LREAL := 200.0; (* kW at 100% command *)
    AmbientTemperature : LREAL := 20.0;
    RequestedHeaterPower : LREAL := 0.0;
    ProcessValue : LREAL := 20.0;
    AppliedHeaterPower : LREAL := 0.0;
    StaticGain : LREAL := 0.0;
    TimeConstant : LREAL := 0.0;
    PredictedEquilibrium : LREAL := 20.0;
    ModelValid : BOOL := FALSE;
    InputsValid : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ManualMode : BOOL := TRUE;
END_VAR

(* C*dT/dt = Qin - H*(T-Tambient). Therefore tau=C/H and the
   temperature gain per percent command is (P100/100)/H. *)
ModelValid := ThermalCapacitance > LREAL#0.0
    AND ThermalCapacitance <= LREAL#1.0E6
    AND EffectiveHeatLoss > LREAL#0.0
    AND EffectiveHeatLoss <= LREAL#1.0E6
    AND FullScaleHeaterPower >= LREAL#0.0
    AND FullScaleHeaterPower <= LREAL#1.0E6
    AND AmbientTemperature >= LREAL#0.0
    AND AmbientTemperature <= LREAL#80.0;

(* Control Studio changes only this Boolean authority. The project owns the
   reviewed 20-percent request and restores the request to zero with authority. *)
IF ExperimentEnabled THEN
    RequestedHeaterPower := LREAL#20.0;
ELSE
    RequestedHeaterPower := LREAL#0.0;
END_IF;
ManualMode := NOT ExperimentEnabled;

InputsValid := ProcessValue >= LREAL#0.0
    AND ProcessValue <= LREAL#80.0
    AND RequestedHeaterPower >= LREAL#-1000.0
    AND RequestedHeaterPower <= LREAL#1000.0;

IF ModelValid THEN
    TimeConstant := ThermalCapacitance / EffectiveHeatLoss;
    StaticGain := (FullScaleHeaterPower / LREAL#100.0) / EffectiveHeatLoss;
ELSE
    StaticGain := LREAL#0.0;
    TimeConstant := LREAL#0.0;
END_IF;

IF ModelValid AND InputsValid AND ExperimentEnabled THEN
    (* Preserve the request for diagnosis, but derive the physical prediction
       from the bounded power that the one-sided heater can actually apply. *)
    IF RequestedHeaterPower < LREAL#0.0 THEN
        AppliedHeaterPower := LREAL#0.0;
    ELSIF RequestedHeaterPower > LREAL#100.0 THEN
        AppliedHeaterPower := LREAL#100.0;
    ELSE
        AppliedHeaterPower := RequestedHeaterPower;
    END_IF;
    PredictedEquilibrium := AmbientTemperature + StaticGain * AppliedHeaterPower;
ELSE
    (* Removing Test authority is the executable Manual restoration: the model
       remains inspectable, but the plant command returns to safe zero. *)
    AppliedHeaterPower := LREAL#0.0;
    IF ModelValid THEN
        PredictedEquilibrium := AmbientTemperature;
    ELSE
        PredictedEquilibrium := LREAL#0.0;
    END_IF;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.RequestedHeaterPower AT %ML32 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.AppliedHeaterPower AT %QL0 : LREAL;
    P1.StaticGain AT %ML0 : LREAL;
    P1.TimeConstant AT %ML8 : LREAL;
    P1.PredictedEquilibrium AT %ML16 : LREAL;
    P1.ModelValid AT %MX40.0 : BOOL;
    P1.InputsValid AT %MX40.1 : BOOL;
    P1.ExperimentEnabled AT %MX40.2 : BOOL;
    P1.ManualMode AT %MX40.3 : BOOL;
END_VAR
END_CONFIGURATION
