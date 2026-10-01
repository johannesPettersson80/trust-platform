USING Control;

(* Model-based PI of Lesson 19 for a first-order model with gain K and time constant τ and the chosen response time λ:
   Kp = τ/(K·λ) and Ki = 1/(K·λ). *)
FUNCTION ModelBasedKp : LREAL
VAR_INPUT
    Gain : LREAL;
    TimeConstantS : LREAL;
    ResponseTimeS : LREAL;
END_VAR
ModelBasedKp := TimeConstantS / (Gain * ResponseTimeS);
END_FUNCTION

FUNCTION ModelBasedKi : LREAL
VAR_INPUT
    Gain : LREAL;
    ResponseTimeS : LREAL;
END_VAR
ModelBasedKi := LREAL#1.0 / (Gain * ResponseTimeS);
END_FUNCTION

(* At rest the vessel loses 2·(Tv − 20) kW, which the jacket passes on through 10·(Tj − Tv): Tj = Tv + 0.2·(Tv − 20).
   The heater replaces those losses and the jacket extraction Dj at 2 kW per point. *)
FUNCTION HoldingJacketC : LREAL
VAR_INPUT
    VesselC : LREAL;
END_VAR
HoldingJacketC := VesselC + LREAL#0.2 * (VesselC - LREAL#20.0);
END_FUNCTION

FUNCTION HoldingCommandPct : LREAL
VAR_INPUT
    VesselC : LREAL;
    JacketLoadKw : LREAL;
END_VAR
HoldingCommandPct := (LREAL#2.0 * (VesselC - LREAL#20.0) + JacketLoadKw) / LREAL#2.0;
END_FUNCTION

(* The single loop, tuned with Lesson 19's rule for the whole vessel: one PI from the vessel temperature straight to
   heater power, Kp = 2.5 % per °C, Ki = 0.1 % per °C·s, in Auto from 35 °C and 15 %. *)
FUNCTION_BLOCK VesselPi
VAR_INPUT
    VesselSetpointC : LREAL;
    VesselC : LREAL;
END_VAR
VAR_OUTPUT
    HeaterPct : LREAL := 15.0;
END_VAR
VAR
    Loop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
        IntegralGain := LREAL#0.1,
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
    Input : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
END_VAR
Loop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := LREAL#35.0;
    State.ProcessValue := LREAL#35.0;
    State.NominalOutput := LREAL#15.0;
    State.SaturatedOutput := LREAL#15.0;
    Loop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
Input.Setpoint := VesselSetpointC;
Input.ProcessValue := VesselC;
HeaterPct := Loop.Update(Input := Input);
END_FUNCTION_BLOCK

(* Two jacketed vessels. The first is held by a cascade of two Control.PID blocks that Control Studio commissions: the
   outer PID turns the vessel error into a jacket setpoint (20–50 °C), the inner PID turns the jacket error into heater
   power (0–100 %). Each has its own Auto/Manual switch and manual output, and both start in Manual at the resting point,
   so the inner loop can be tuned, validated and applied with the outer PID in Manual, and then the outer loop through
   the applied inner loop. The second vessel keeps the single loop. The project test takes 30 kW from both jackets. *)
PROGRAM Main
VAR
    Outer : Control.PID;
    Inner : Control.PID;
    OuterParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.5, IntegralGain := LREAL#0.12, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#20.0, OutputMaximum := LREAL#50.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    InnerParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0, IntegralGain := LREAL#10.0, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
    (* TRUE selects Auto. *)
    OuterAuto : BOOL := FALSE;
    InnerAuto : BOOL := FALSE;
    OuterManualC : LREAL := 38.0;
    InnerManualPct : LREAL := 15.0;
    VesselSetpointC : LREAL := 35.0;
    VesselC : LREAL := 35.0;
    JacketC : LREAL := 38.0;
    JacketSetpointC : LREAL := 38.0;
    HeaterPct : LREAL := 15.0;
    JacketLoadKw : LREAL := 0.0;
    Single : VesselPi;
    SingleVesselC : LREAL := 35.0;
    SingleJacketC : LREAL := 38.0;
    SinglePct : LREAL := 15.0;
    SingleLoadKw : LREAL := 0.0;
    (* Control Studio's project test takes 30 kW from both jackets. *)
    ExperimentEnabled : BOOL := FALSE;
END_VAR

Outer.Configure(Parameters := OuterParameters);
Inner.Configure(Parameters := InnerParameters);
IF NOT Initialized THEN
    (* Memory-mapped variables start from the %M image; set the resting values explicitly. *)
    VesselSetpointC := LREAL#35.0;
    JacketSetpointC := LREAL#38.0;
    OuterManualC := LREAL#38.0;
    InnerManualPct := LREAL#15.0;
    State.Setpoint := VesselSetpointC;
    State.ProcessValue := VesselC;
    State.NominalOutput := OuterManualC;
    State.SaturatedOutput := OuterManualC;
    Outer.Initialize(State := State);
    State.Setpoint := JacketSetpointC;
    State.ProcessValue := JacketC;
    State.NominalOutput := InnerManualPct;
    State.SaturatedOutput := InnerManualPct;
    Inner.Initialize(State := State);
    Initialized := TRUE;
END_IF;
IF ExperimentEnabled THEN
    JacketLoadKw := LREAL#30.0;
ELSE
    JacketLoadKw := LREAL#0.0;
END_IF;
SingleLoadKw := JacketLoadKw;

OuterInput.Setpoint := VesselSetpointC;
OuterInput.ProcessValue := VesselC;
OuterInput.ManualOutput := OuterManualC;
OuterInput.TrackingOutput := OuterManualC;
OuterInput.CycleTime := LREAL#0.1;
IF OuterAuto THEN
    OuterInput.Mode := PIDMode#Auto;
ELSE
    OuterInput.Mode := PIDMode#Manual;
END_IF;
JacketSetpointC := Outer.Update(Input := OuterInput);

InnerInput.Setpoint := JacketSetpointC;
InnerInput.ProcessValue := JacketC;
InnerInput.ManualOutput := InnerManualPct;
InnerInput.TrackingOutput := InnerManualPct;
InnerInput.CycleTime := LREAL#0.1;
IF InnerAuto THEN
    InnerInput.Mode := PIDMode#Auto;
ELSE
    InnerInput.Mode := PIDMode#Manual;
END_IF;
HeaterPct := Inner.Update(Input := InnerInput);

Single(VesselSetpointC := LREAL#35.0, VesselC := SingleVesselC);
SinglePct := Single.HeaterPct;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.HeaterPct AT %QL0 : LREAL;
    P1.JacketLoadKw AT %QL8 : LREAL;
    P1.JacketC AT %IL0 : LREAL;
    P1.VesselC AT %IL8 : LREAL;
    P1.SinglePct AT %QL16 : LREAL;
    P1.SingleLoadKw AT %QL24 : LREAL;
    P1.SingleJacketC AT %IL16 : LREAL;
    P1.SingleVesselC AT %IL24 : LREAL;
    P1.VesselSetpointC AT %ML0 : LREAL;
    P1.JacketSetpointC AT %ML8 : LREAL;
    P1.OuterManualC AT %ML16 : LREAL;
    P1.InnerManualPct AT %ML24 : LREAL;
    P1.ExperimentEnabled AT %MX32.0 : BOOL;
END_VAR
END_CONFIGURATION
