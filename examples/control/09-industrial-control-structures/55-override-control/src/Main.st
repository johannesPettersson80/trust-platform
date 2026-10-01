USING Control;

(* At rest the vessel loses 2·(Tv − 20) kW through a jacket 10 kW/°C warmer per degree: Tj = Tv + 0.2·(Tv − 20). *)
FUNCTION HoldingJacketC : LREAL
VAR_INPUT
    VesselC : LREAL;
END_VAR
HoldingJacketC := VesselC + LREAL#0.2 * (VesselC - LREAL#20.0);
END_FUNCTION

(* The warmest vessel a jacket limit allows: Tj = 1.2·Tv − 4 solved for Tv. *)
FUNCTION MaximumVesselC : LREAL
VAR_INPUT
    JacketLimitC : LREAL;
END_VAR
MaximumVesselC := (JacketLimitC + LREAL#4.0) / LREAL#1.2;
END_FUNCTION

(* Low select for a heating constraint: the smaller heating request wins. *)
FUNCTION LowSelect : LREAL
VAR_INPUT
    NormalPct : LREAL;
    LimitPct : LREAL;
END_VAR
LowSelect := MIN(NormalPct, LimitPct);
END_FUNCTION

(* Override: the vessel PI (2.5 % per °C, 0.1 % per °C·s) and the jacket-limit PI (5 % per °C, 10 % per °C·s, setpoint
   JacketLimitSetpointC) both request heater power; the heater gets the smaller request. With Tracking, the controller that
   was not selected runs in Track mode from the applied command, so its request stays ready to take over without a jump. *)
FUNCTION_BLOCK OverrideLoop
VAR_INPUT
    VesselSetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
    JacketLimitSetpointC : LREAL := 41.8;
    Tracking : BOOL := TRUE;
END_VAR
VAR_OUTPUT
    HeaterPct : LREAL := 15.0;
    VesselRequestPct : LREAL := 15.0;
    JacketRequestPct : LREAL := 15.0;
    JacketSelected : BOOL := FALSE;
END_VAR
VAR
    VesselLoop : Control.PID;
    JacketLoop : Control.PID;
    VesselParameters : Control.PIDParameters := (
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
    JacketParameters : Control.PIDParameters := (
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
    VesselInput : Control.PIDInput;
    JacketInput : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
END_VAR
VesselLoop.Configure(Parameters := VesselParameters);
JacketLoop.Configure(Parameters := JacketParameters);
IF NOT Initialized THEN
    State.Setpoint := LREAL#35.0;
    State.ProcessValue := LREAL#35.0;
    State.NominalOutput := LREAL#15.0;
    State.SaturatedOutput := LREAL#15.0;
    VesselLoop.Initialize(State := State);
    State.Setpoint := JacketLimitSetpointC;
    State.ProcessValue := LREAL#38.0;
    JacketLoop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
VesselInput.Mode := PIDMode#Auto;
JacketInput.Mode := PIDMode#Auto;
IF Tracking THEN
    IF JacketSelected THEN
        VesselInput.Mode := PIDMode#Track;
    ELSE
        JacketInput.Mode := PIDMode#Track;
    END_IF;
END_IF;
VesselInput.TrackingOutput := HeaterPct;
VesselInput.CycleTime := LREAL#0.1;
VesselInput.Setpoint := VesselSetpointC;
VesselInput.ProcessValue := VesselC;
VesselRequestPct := VesselLoop.Update(Input := VesselInput);
JacketInput.TrackingOutput := HeaterPct;
JacketInput.CycleTime := LREAL#0.1;
JacketInput.Setpoint := JacketLimitSetpointC;
JacketInput.ProcessValue := JacketC;
JacketRequestPct := JacketLoop.Update(Input := JacketInput);
HeaterPct := LIMIT(LREAL#0.0, LowSelect(NormalPct := VesselRequestPct, LimitPct := JacketRequestPct), LREAL#100.0);
JacketSelected := JacketRequestPct < VesselRequestPct;
END_FUNCTION_BLOCK

(* The vessel PI alone, without a jacket limit. *)
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

PROGRAM Main
VAR
    (* Three copies of the heater-only jacketed vessel: the vessel PI alone, the override with tracking and the override
       without tracking. *)
    Alone : VesselPi;
    Tracked : OverrideLoop;
    Untracked : OverrideLoop;
    AloneJacketC : LREAL := 38.0;
    AloneVesselC : LREAL := 35.0;
    AloneHeaterPct : LREAL := 15.0;
    TrackedJacketC : LREAL := 38.0;
    TrackedVesselC : LREAL := 35.0;
    TrackedHeaterPct : LREAL := 15.0;
    UntrackedJacketC : LREAL := 38.0;
    UntrackedVesselC : LREAL := 35.0;
    UntrackedHeaterPct : LREAL := 15.0;
    (* Control Studio's project experiment asks for 40 °C, which the jacket limit makes impossible, and for 37 °C from
       40 s on. *)
    VesselSetpointC : LREAL := 35.0;
    JacketLimitSetpointC : LREAL := 41.8;
    ElapsedS : LREAL := 0.0;
    TrackedVesselRequestPct : LREAL := 15.0;
    TrackedJacketRequestPct : LREAL := 15.0;
    UntrackedVesselRequestPct : LREAL := 15.0;
    UntrackedJacketRequestPct : LREAL := 15.0;
    ExperimentEnabled : BOOL := FALSE;
    TrackedJacketSelected : BOOL := FALSE;
    UntrackedJacketSelected : BOOL := FALSE;
END_VAR

JacketLimitSetpointC := LREAL#41.8;
IF ExperimentEnabled THEN
    IF ElapsedS < LREAL#39.95 THEN
        VesselSetpointC := LREAL#40.0;
    ELSE
        VesselSetpointC := LREAL#37.0;
    END_IF;
    ElapsedS := ElapsedS + LREAL#0.1;
ELSE
    VesselSetpointC := LREAL#35.0;
    ElapsedS := LREAL#0.0;
END_IF;
Alone(VesselSetpointC := VesselSetpointC, VesselC := AloneVesselC);
AloneHeaterPct := Alone.HeaterPct;
Tracked(VesselSetpointC := VesselSetpointC, VesselC := TrackedVesselC, JacketC := TrackedJacketC,
    JacketLimitSetpointC := JacketLimitSetpointC, Tracking := TRUE);
TrackedHeaterPct := Tracked.HeaterPct;
TrackedVesselRequestPct := Tracked.VesselRequestPct;
TrackedJacketRequestPct := Tracked.JacketRequestPct;
TrackedJacketSelected := Tracked.JacketSelected;
Untracked(VesselSetpointC := VesselSetpointC, VesselC := UntrackedVesselC, JacketC := UntrackedJacketC,
    JacketLimitSetpointC := JacketLimitSetpointC, Tracking := FALSE);
UntrackedHeaterPct := Untracked.HeaterPct;
UntrackedVesselRequestPct := Untracked.VesselRequestPct;
UntrackedJacketRequestPct := Untracked.JacketRequestPct;
UntrackedJacketSelected := Untracked.JacketSelected;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AloneHeaterPct AT %QL0 : LREAL;
    P1.AloneJacketC AT %IL0 : LREAL;
    P1.AloneVesselC AT %IL8 : LREAL;
    P1.TrackedHeaterPct AT %QL8 : LREAL;
    P1.TrackedJacketC AT %IL16 : LREAL;
    P1.TrackedVesselC AT %IL24 : LREAL;
    P1.UntrackedHeaterPct AT %QL16 : LREAL;
    P1.UntrackedJacketC AT %IL32 : LREAL;
    P1.UntrackedVesselC AT %IL40 : LREAL;
    P1.VesselSetpointC AT %ML0 : LREAL;
    P1.JacketLimitSetpointC AT %ML8 : LREAL;
    P1.TrackedVesselRequestPct AT %ML16 : LREAL;
    P1.TrackedJacketRequestPct AT %ML24 : LREAL;
    P1.UntrackedVesselRequestPct AT %ML32 : LREAL;
    P1.UntrackedJacketRequestPct AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX48.0 : BOOL;
    P1.TrackedJacketSelected AT %MX48.1 : BOOL;
    P1.UntrackedJacketSelected AT %MX48.2 : BOOL;
END_VAR
END_CONFIGURATION
