(* Flow through the teaching valve: q = 2·z·sqrt(dp/100 kPa) kg/s with opening fraction z = x/100. *)
FUNCTION ValveFlow : LREAL
VAR_INPUT
    OpeningPct : LREAL;
    PressureDropKPa : LREAL;
END_VAR
ValveFlow := LREAL#2.0 * OpeningPct / LREAL#100.0 * SQRT(MAX(PressureDropKPa, LREAL#0.0) / LREAL#100.0);
END_FUNCTION

(* Installed flow with a fixed 128.125 kPa supply and pipe loss 12.5·q²: q = 2z·sqrt(1.28125/(1 + 0.5z²)). *)
FUNCTION FixedSupplyFlow : LREAL
VAR_INPUT
    OpeningPct : LREAL;
END_VAR
VAR
    Z : LREAL;
END_VAR
Z := OpeningPct / LREAL#100.0;
FixedSupplyFlow := LREAL#2.0 * Z * SQRT(LREAL#1.28125 / (LREAL#1.0 + LREAL#0.5 * Z * Z));
END_FUNCTION

(* Heat carried by the hot water: Q = cp·q·(Tin − T) with cp = 4 kJ/(kg·°C), in kW. *)
FUNCTION DeliveredHeatKW : LREAL
VAR_INPUT
    FlowKgS : LREAL;
    SupplyC : LREAL;
    WaterC : LREAL;
END_VAR
DeliveredHeatKW := LREAL#4.0 * FlowKgS * (SupplyC - WaterC);
END_FUNCTION

(* Local slope between two steady points, per percentage point of measured stem position. *)
FUNCTION LocalSlope : LREAL
VAR_INPUT
    FlowLow : LREAL;
    FlowHigh : LREAL;
    StemLow : LREAL;
    StemHigh : LREAL;
END_VAR
IF StemHigh <> StemLow THEN
    LocalSlope := (FlowHigh - FlowLow) / (StemHigh - StemLow);
ELSE
    LocalSlope := LREAL#0.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    (* One stem request drives both pressure arrangements; the copies feed the pressure measurements. *)
    ValveRequestPct : LREAL := 75.0;
    RegulatedPressureRequestPct : LREAL := 75.0;
    FixedSupplyRequestPct : LREAL := 75.0;
    FixedSupplyPressureRequestPct : LREAL := 75.0;
    StemPositionPct : LREAL := 75.0;
    FlowRegulatedKgS : LREAL := 1.5;
    FlowFixedSupplyKgS : LREAL := 1.5;
    PressureRegulatedKPa : LREAL := 100.0;
    PressureFixedSupplyKPa : LREAL := 100.0;
    (* Staircase: each target is held 30 s; the final 3 s count only if the stem has arrived within
       0.1 point and each flow varied by at most SteadyRangeKgS. *)
    Targets : ARRAY[0..4] OF LREAL := [70.0, 75.0, 80.0, 85.0, 75.0];
    SteadyRangeKgS : LREAL := 0.015;
    HoldTimer : TON;
    StepIndex : INT := 0;
    Samples : DINT := 0;
    StemSum : LREAL := 0.0;
    RegulatedSum : LREAL := 0.0;
    RegulatedMinimum : LREAL := 0.0;
    RegulatedMaximum : LREAL := 0.0;
    FixedSum : LREAL := 0.0;
    FixedMinimum : LREAL := 0.0;
    FixedMaximum : LREAL := 0.0;
    RegulatedPressureSum : LREAL := 0.0;
    FixedPressureSum : LREAL := 0.0;
    SteadyStemPct : ARRAY[0..4] OF LREAL;
    SteadyFlowRegulated : ARRAY[0..4] OF LREAL;
    SteadyFlowFixedSupply : ARRAY[0..4] OF LREAL;
    SteadyPressureRegulated : ARRAY[0..4] OF LREAL;
    SteadyPressureFixedSupply : ARRAY[0..4] OF LREAL;
    SteadyQualified : ARRAY[0..4] OF BOOL;
    (* Results at the stated temperatures: 60 °C supply, 35 °C water. *)
    SupplyC : LREAL := 60.0;
    WaterC : LREAL := 35.0;
    SlopeRegulatedKgSPerPct : LREAL := 0.0;
    SlopeFixedSupplyKgSPerPct : LREAL := 0.0;
    HeatAt75PctKW : LREAL := 0.0;
    HeatSlopeKWPerPct : LREAL := 0.0;
    Index : INT := 0;
END_VAR

IF ExperimentEnabled AND NOT WasEnabled THEN
    Active := TRUE;
    StepIndex := 0;
    Samples := 0;
    FOR Index := 0 TO 4 DO
        SteadyQualified[Index] := FALSE;
    END_FOR;
    HoldTimer(IN := FALSE, PT := T#30s);
END_IF;
IF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
WasEnabled := ExperimentEnabled;

IF Active THEN
    ValveRequestPct := Targets[StepIndex];
    HoldTimer(IN := TRUE, PT := T#30s);
    (* The final 3 s of each hold form the steady window. *)
    IF HoldTimer.ET >= T#27s THEN
        IF Samples = 0 THEN
            RegulatedMinimum := FlowRegulatedKgS; RegulatedMaximum := FlowRegulatedKgS;
            FixedMinimum := FlowFixedSupplyKgS; FixedMaximum := FlowFixedSupplyKgS;
            StemSum := 0.0; RegulatedSum := 0.0; FixedSum := 0.0;
            RegulatedPressureSum := 0.0; FixedPressureSum := 0.0;
        END_IF;
        Samples := Samples + 1;
        StemSum := StemSum + StemPositionPct;
        RegulatedSum := RegulatedSum + FlowRegulatedKgS;
        FixedSum := FixedSum + FlowFixedSupplyKgS;
        RegulatedPressureSum := RegulatedPressureSum + PressureRegulatedKPa;
        FixedPressureSum := FixedPressureSum + PressureFixedSupplyKPa;
        RegulatedMinimum := MIN(RegulatedMinimum, FlowRegulatedKgS);
        RegulatedMaximum := MAX(RegulatedMaximum, FlowRegulatedKgS);
        FixedMinimum := MIN(FixedMinimum, FlowFixedSupplyKgS);
        FixedMaximum := MAX(FixedMaximum, FlowFixedSupplyKgS);
    END_IF;
    IF HoldTimer.Q THEN
        SteadyStemPct[StepIndex] := StemSum / DINT_TO_LREAL(Samples);
        SteadyFlowRegulated[StepIndex] := RegulatedSum / DINT_TO_LREAL(Samples);
        SteadyFlowFixedSupply[StepIndex] := FixedSum / DINT_TO_LREAL(Samples);
        SteadyPressureRegulated[StepIndex] := RegulatedPressureSum / DINT_TO_LREAL(Samples);
        SteadyPressureFixedSupply[StepIndex] := FixedPressureSum / DINT_TO_LREAL(Samples);
        SteadyQualified[StepIndex] := ABS(SteadyStemPct[StepIndex] - Targets[StepIndex]) <= LREAL#0.1
            AND RegulatedMaximum - RegulatedMinimum <= SteadyRangeKgS
            AND FixedMaximum - FixedMinimum <= SteadyRangeKgS;
        Samples := 0;
        HoldTimer(IN := FALSE, PT := T#30s);
        IF StepIndex < 4 THEN
            StepIndex := StepIndex + 1;
        ELSE
            Active := FALSE;
            (* The 75 → 80 % step gives the local slopes and the heat at the stated temperatures. *)
            SlopeRegulatedKgSPerPct := LocalSlope(FlowLow := SteadyFlowRegulated[1], FlowHigh := SteadyFlowRegulated[2],
                StemLow := SteadyStemPct[1], StemHigh := SteadyStemPct[2]);
            SlopeFixedSupplyKgSPerPct := LocalSlope(FlowLow := SteadyFlowFixedSupply[1], FlowHigh := SteadyFlowFixedSupply[2],
                StemLow := SteadyStemPct[1], StemHigh := SteadyStemPct[2]);
            HeatAt75PctKW := DeliveredHeatKW(FlowKgS := SteadyFlowRegulated[1], SupplyC := SupplyC, WaterC := WaterC);
            HeatSlopeKWPerPct := LREAL#4.0 * (SupplyC - WaterC) * SlopeRegulatedKgSPerPct;
        END_IF;
    END_IF;
END_IF;
IF NOT Active THEN
    ValveRequestPct := LREAL#75.0;
    HoldTimer(IN := FALSE, PT := T#30s);
END_IF;
RegulatedPressureRequestPct := ValveRequestPct;
FixedSupplyRequestPct := ValveRequestPct;
FixedSupplyPressureRequestPct := ValveRequestPct;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
    P1.Active AT %MX64.1 : BOOL;
    P1.ValveRequestPct AT %QL0 : LREAL;
    P1.RegulatedPressureRequestPct AT %QL8 : LREAL;
    P1.FixedSupplyRequestPct AT %QL16 : LREAL;
    P1.FixedSupplyPressureRequestPct AT %QL24 : LREAL;
    P1.FlowRegulatedKgS AT %IL0 : LREAL;
    P1.FlowFixedSupplyKgS AT %IL8 : LREAL;
    P1.StemPositionPct AT %IL16 : LREAL;
    P1.PressureRegulatedKPa AT %IL24 : LREAL;
    P1.PressureFixedSupplyKPa AT %IL32 : LREAL;
END_VAR
END_CONFIGURATION
