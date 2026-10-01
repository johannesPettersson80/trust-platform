USING Control;

(* Jacketed-vessel temperature cascade: the outer PID turns the vessel error into a jacket-temperature
   setpoint in °C (20–50 °C), the inner PID turns the jacket error into heater power in % (0–100 %).
   Each controller has its own Auto/Manual switch and manual output, so it can be identified, tuned and
   applied on its own: tune the inner loop with the outer in Manual, then the outer with the inner in Auto. *)
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
    (* TRUE selects Auto; both controllers start in Manual at the resting point. *)
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
    P1.VesselSetpointC AT %ML0 : LREAL;
    P1.JacketSetpointC AT %ML8 : LREAL;
    P1.OuterManualC AT %ML16 : LREAL;
    P1.InnerManualPct AT %ML24 : LREAL;
END_VAR
END_CONFIGURATION
