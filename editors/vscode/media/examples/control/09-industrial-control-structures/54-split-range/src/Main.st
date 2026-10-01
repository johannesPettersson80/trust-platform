USING Control;

(* Split range for Q = 2·uh − uc: a positive request selects the heater at 2 kW per point, a negative one the cooler at
   1 kW per point; the other service stays at 0 %. There is no neutral band: only Q = 0 turns both off. *)
FUNCTION HeaterPct : LREAL
VAR_INPUT
    RequestKw : LREAL;
END_VAR
HeaterPct := LIMIT(LREAL#0.0, MAX(RequestKw, LREAL#0.0) / LREAL#2.0, LREAL#100.0);
END_FUNCTION

FUNCTION CoolerPct : LREAL
VAR_INPUT
    RequestKw : LREAL;
END_VAR
CoolerPct := LIMIT(LREAL#0.0, MAX(-RequestKw, LREAL#0.0), LREAL#100.0);
END_FUNCTION

FUNCTION NetHeatKw : LREAL
VAR_INPUT
    HeaterPct : LREAL;
    CoolerPct : LREAL;
END_VAR
NetHeatKw := LREAL#2.0 * HeaterPct - CoolerPct;
END_FUNCTION

(* The cascade of Lesson 63 with a net-heat inner loop: the outer PI (0.5 °C/°C, 0.12 1/s) sets the jacket setpoint in °C,
   20–50 °C; the inner PI (10 kW/°C, 20 kW/(°C·s)) requests net heat in kW, limited to −100…200 kW, the capacities of the
   cooler and the heater. The request is split onto the two services, so the inner PI's limited output is the applied
   net heat that its back-calculation tracks. *)
FUNCTION_BLOCK SplitCascade
VAR_INPUT
    VesselSetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
END_VAR
VAR_OUTPUT
    JacketSetpointC : LREAL := 38.0;
    RequestKw : LREAL := 30.0;
    HeaterCommandPct : LREAL := 15.0;
    CoolerCommandPct : LREAL := 0.0;
    AppliedNetKw : LREAL := 30.0;
END_VAR
VAR
    Outer : Control.PID;
    Inner : Control.PID;
    OuterParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.5,
        IntegralGain := LREAL#0.12,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#20.0,
        OutputMaximum := LREAL#50.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InnerParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#10.0,
        IntegralGain := LREAL#20.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#-100.0,
        OutputMaximum := LREAL#200.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
END_VAR
Outer.Configure(Parameters := OuterParameters);
Inner.Configure(Parameters := InnerParameters);
IF NOT Initialized THEN
    State.Setpoint := LREAL#35.0;
    State.ProcessValue := LREAL#35.0;
    State.NominalOutput := LREAL#38.0;
    State.SaturatedOutput := LREAL#38.0;
    Outer.Initialize(State := State);
    State.Setpoint := LREAL#38.0;
    State.ProcessValue := LREAL#38.0;
    State.NominalOutput := LREAL#30.0;
    State.SaturatedOutput := LREAL#30.0;
    Inner.Initialize(State := State);
    Initialized := TRUE;
END_IF;
OuterInput.Mode := PIDMode#Auto;
OuterInput.CycleTime := LREAL#0.1;
OuterInput.Setpoint := VesselSetpointC;
OuterInput.ProcessValue := VesselC;
JacketSetpointC := Outer.Update(Input := OuterInput);
InnerInput.Mode := PIDMode#Auto;
InnerInput.CycleTime := LREAL#0.1;
InnerInput.Setpoint := JacketSetpointC;
InnerInput.ProcessValue := JacketC;
RequestKw := Inner.Update(Input := InnerInput);
HeaterCommandPct := HeaterPct(RequestKw := RequestKw);
CoolerCommandPct := CoolerPct(RequestKw := RequestKw);
AppliedNetKw := NetHeatKw(HeaterPct := HeaterCommandPct, CoolerPct := CoolerCommandPct);
END_FUNCTION_BLOCK

(* The heater-only cascade of Lesson 63: the inner PI (5 %/°C, 10 %/(°C·s)) drives the heater, 0–100 %. *)
FUNCTION_BLOCK HeaterCascade
VAR_INPUT
    VesselSetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
END_VAR
VAR_OUTPUT
    JacketSetpointC : LREAL := 38.0;
    HeaterCommandPct : LREAL := 15.0;
END_VAR
VAR
    Outer : Control.PID;
    Inner : Control.PID;
    OuterParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.5,
        IntegralGain := LREAL#0.12,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#20.0,
        OutputMaximum := LREAL#50.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    InnerParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0,
        IntegralGain := LREAL#10.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
END_VAR
Outer.Configure(Parameters := OuterParameters);
Inner.Configure(Parameters := InnerParameters);
IF NOT Initialized THEN
    State.Setpoint := LREAL#35.0;
    State.ProcessValue := LREAL#35.0;
    State.NominalOutput := LREAL#38.0;
    State.SaturatedOutput := LREAL#38.0;
    Outer.Initialize(State := State);
    State.Setpoint := LREAL#38.0;
    State.ProcessValue := LREAL#38.0;
    State.NominalOutput := LREAL#15.0;
    State.SaturatedOutput := LREAL#15.0;
    Inner.Initialize(State := State);
    Initialized := TRUE;
END_IF;
OuterInput.Mode := PIDMode#Auto;
OuterInput.CycleTime := LREAL#0.1;
OuterInput.Setpoint := VesselSetpointC;
OuterInput.ProcessValue := VesselC;
JacketSetpointC := Outer.Update(Input := OuterInput);
InnerInput.Mode := PIDMode#Auto;
InnerInput.CycleTime := LREAL#0.1;
InnerInput.Setpoint := JacketSetpointC;
InnerInput.ProcessValue := JacketC;
HeaterCommandPct := Inner.Update(Input := InnerInput);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Three copies of the jacketed vessel with a cooler: the heater-only cascade and the split-range cascade with 60 kW
       entering the jacket, and the split-range cascade with a 30 kW jacket loss. *)
    HeaterOnly : HeaterCascade;
    SplitInflow : SplitCascade;
    SplitLoss : SplitCascade;
    HeaterOnlyJacketC : LREAL := 38.0;
    HeaterOnlyVesselC : LREAL := 35.0;
    HeaterOnlyHeaterPct : LREAL := 15.0;
    HeaterOnlyCoolerPct : LREAL := 0.0;
    HeaterOnlyExtractionKw : LREAL := 0.0;
    InflowJacketC : LREAL := 38.0;
    InflowVesselC : LREAL := 35.0;
    InflowHeaterPct : LREAL := 15.0;
    InflowCoolerPct : LREAL := 0.0;
    InflowExtractionKw : LREAL := 0.0;
    LossJacketC : LREAL := 38.0;
    LossVesselC : LREAL := 35.0;
    LossHeaterPct : LREAL := 15.0;
    LossCoolerPct : LREAL := 0.0;
    LossExtractionKw : LREAL := 0.0;
    VesselSetpointC : LREAL := 35.0;
    (* Control Studio's project experiment lets 60 kW enter the first two jackets and removes 30 kW from the third. *)
    InflowKw : LREAL := 0.0;
    LossKw : LREAL := 0.0;
    InflowRequestKw : LREAL := 30.0;
    InflowNetKw : LREAL := 30.0;
    LossRequestKw : LREAL := 30.0;
    LossNetKw : LREAL := 30.0;
    HeaterOnlyJacketSetpointC : LREAL := 38.0;
    InflowJacketSetpointC : LREAL := 38.0;
    ExperimentEnabled : BOOL := FALSE;
END_VAR

VesselSetpointC := LREAL#35.0;
IF ExperimentEnabled THEN
    InflowKw := LREAL#60.0;
    LossKw := LREAL#30.0;
ELSE
    InflowKw := LREAL#0.0;
    LossKw := LREAL#0.0;
END_IF;
HeaterOnly(VesselSetpointC := VesselSetpointC, VesselC := HeaterOnlyVesselC, JacketC := HeaterOnlyJacketC);
HeaterOnlyHeaterPct := HeaterOnly.HeaterCommandPct;
HeaterOnlyCoolerPct := LREAL#0.0;
HeaterOnlyExtractionKw := -InflowKw;
HeaterOnlyJacketSetpointC := HeaterOnly.JacketSetpointC;
SplitInflow(VesselSetpointC := VesselSetpointC, VesselC := InflowVesselC, JacketC := InflowJacketC);
InflowHeaterPct := SplitInflow.HeaterCommandPct;
InflowCoolerPct := SplitInflow.CoolerCommandPct;
InflowExtractionKw := -InflowKw;
InflowRequestKw := SplitInflow.RequestKw;
InflowNetKw := SplitInflow.AppliedNetKw;
InflowJacketSetpointC := SplitInflow.JacketSetpointC;
SplitLoss(VesselSetpointC := VesselSetpointC, VesselC := LossVesselC, JacketC := LossJacketC);
LossHeaterPct := SplitLoss.HeaterCommandPct;
LossCoolerPct := SplitLoss.CoolerCommandPct;
LossExtractionKw := LossKw;
LossRequestKw := SplitLoss.RequestKw;
LossNetKw := SplitLoss.AppliedNetKw;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.HeaterOnlyHeaterPct AT %QL0 : LREAL;
    P1.HeaterOnlyCoolerPct AT %QL8 : LREAL;
    P1.HeaterOnlyExtractionKw AT %QL16 : LREAL;
    P1.HeaterOnlyJacketC AT %IL0 : LREAL;
    P1.HeaterOnlyVesselC AT %IL8 : LREAL;
    P1.InflowHeaterPct AT %QL24 : LREAL;
    P1.InflowCoolerPct AT %QL32 : LREAL;
    P1.InflowExtractionKw AT %QL40 : LREAL;
    P1.InflowJacketC AT %IL16 : LREAL;
    P1.InflowVesselC AT %IL24 : LREAL;
    P1.LossHeaterPct AT %QL48 : LREAL;
    P1.LossCoolerPct AT %QL56 : LREAL;
    P1.LossExtractionKw AT %QL64 : LREAL;
    P1.LossJacketC AT %IL32 : LREAL;
    P1.LossVesselC AT %IL40 : LREAL;
    P1.VesselSetpointC AT %ML0 : LREAL;
    P1.InflowKw AT %ML8 : LREAL;
    P1.LossKw AT %ML16 : LREAL;
    P1.InflowRequestKw AT %ML24 : LREAL;
    P1.InflowNetKw AT %ML32 : LREAL;
    P1.LossRequestKw AT %ML40 : LREAL;
    P1.LossNetKw AT %ML48 : LREAL;
    P1.HeaterOnlyJacketSetpointC AT %ML56 : LREAL;
    P1.InflowJacketSetpointC AT %ML64 : LREAL;
    P1.ExperimentEnabled AT %MX72.0 : BOOL;
END_VAR
END_CONFIGURATION
