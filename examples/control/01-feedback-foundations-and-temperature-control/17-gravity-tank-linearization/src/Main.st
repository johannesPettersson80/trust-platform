(* Slope of the gravity outflow q = c·√h at the operating level h0: g = c/(2·√h0), in m²/s. *)
FUNCTION TangentSlope : LREAL
VAR_INPUT
    Coefficient : LREAL;
    OperatingLevel : LREAL;
END_VAR
TangentSlope := Coefficient / (LREAL#2.0 * SQRT(OperatingLevel));
END_FUNCTION

(* One update of the tangent model A·dx/dt = v − g·x, for the level deviation x and the inflow deviation v. *)
FUNCTION TangentDeviationNext : LREAL
VAR_INPUT
    Deviation : LREAL;
    InflowDeviation : LREAL;
    Slope : LREAL;
    Area : LREAL;
    CycleTime : LREAL;
END_VAR
TangentDeviationNext := Deviation + (InflowDeviation - Slope * Deviation) / Area * CycleTime;
END_FUNCTION

PROGRAM Main
VAR
    (* Two identical tanks of Lesson 45 (A = 0.5 m², c = 0.020 m^(5/2)/s), both operating at h0 = 0.25 m with
       q0 = 0.010 m³/s. *)
    TankAreaM2 : LREAL := 0.5;
    OutletCoefficient : LREAL := 0.020;
    OperatingLevelM : LREAL := 0.25;
    OperatingFlowM3S : LREAL := 0.010;
    TangentSlopeM2S : LREAL := 0.0;
    SmallInletFlowM3S : LREAL := 0.010;
    LargeInletFlowM3S : LREAL := 0.010;
    (* The measured levels, in m. The program only reads them. *)
    SmallLevelM : LREAL := 0.0;
    LargeLevelM : LREAL := 0.0;
    (* The tangent models: deviations from h0, advanced every 1 s update with the inflow deviation applied during
       that second and started from the measured levels. *)
    SmallDeviationM : LREAL := 0.0;
    LargeDeviationM : LREAL := 0.0;
    SmallTangentLevelM : LREAL := 0.0;
    LargeTangentLevelM : LREAL := 0.0;
    AppliedSmallInletM3S : LREAL := 0.010;
    AppliedLargeInletM3S : LREAL := 0.010;
    ModelStarted : BOOL := FALSE;
    (* Control Studio's project experiment: 0.011 m³/s into one tank and 0.015 m³/s into the other while it runs. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
END_VAR

TangentSlopeM2S := TangentSlope(Coefficient := OutletCoefficient, OperatingLevel := OperatingLevelM);
IF NOT ModelStarted THEN
    SmallDeviationM := SmallLevelM - OperatingLevelM;
    LargeDeviationM := LargeLevelM - OperatingLevelM;
    ModelStarted := TRUE;
ELSE
    SmallDeviationM := TangentDeviationNext(Deviation := SmallDeviationM, InflowDeviation := AppliedSmallInletM3S - OperatingFlowM3S,
        Slope := TangentSlopeM2S, Area := TankAreaM2, CycleTime := LREAL#1.0);
    LargeDeviationM := TangentDeviationNext(Deviation := LargeDeviationM, InflowDeviation := AppliedLargeInletM3S - OperatingFlowM3S,
        Slope := TangentSlopeM2S, Area := TankAreaM2, CycleTime := LREAL#1.0);
END_IF;

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    (* Each experiment compares from the levels measured at its step. *)
    SmallDeviationM := SmallLevelM - OperatingLevelM;
    LargeDeviationM := LargeLevelM - OperatingLevelM;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
SmallTangentLevelM := OperatingLevelM + SmallDeviationM;
LargeTangentLevelM := OperatingLevelM + LargeDeviationM;

IF Active THEN
    SmallInletFlowM3S := LREAL#0.011;
    LargeInletFlowM3S := LREAL#0.015;
ELSE
    SmallInletFlowM3S := LREAL#0.010;
    LargeInletFlowM3S := LREAL#0.010;
END_IF;
AppliedSmallInletM3S := SmallInletFlowM3S;
AppliedLargeInletM3S := LargeInletFlowM3S;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SmallInletFlowM3S AT %QL0 : LREAL;
    P1.LargeInletFlowM3S AT %QL8 : LREAL;
    P1.SmallLevelM AT %IL0 : LREAL;
    P1.LargeLevelM AT %IL8 : LREAL;
    P1.SmallTangentLevelM AT %ML0 : LREAL;
    P1.LargeTangentLevelM AT %ML8 : LREAL;
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
    P1.Active AT %MX16.1 : BOOL;
END_VAR
END_CONFIGURATION
