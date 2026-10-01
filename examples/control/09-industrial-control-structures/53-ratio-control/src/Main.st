USING Control;

(* The additive target follows the measured carrier flow in the required ratio R: q_a,sp = R·q_c, in L/min. *)
FUNCTION AdditiveSetpointLMin : LREAL
VAR_INPUT
    Ratio : LREAL;
    CarrierLMin : LREAL;
END_VAR
AdditiveSetpointLMin := Ratio * CarrierLMin;
END_FUNCTION

(* R litres of additive per litre of carrier make R/(1 + R) of the mixture, if the volumes add. *)
FUNCTION MixtureFraction : LREAL
VAR_INPUT
    Ratio : LREAL;
END_VAR
MixtureFraction := Ratio / (LREAL#1.0 + Ratio);
END_FUNCTION

(* After a zero command the pump flow decays as q0·e^(−t/τ): it still delivers ∫ q dt = q0·τ/60 litres, q0 in L/min. *)
FUNCTION TailVolumeL : LREAL
VAR_INPUT
    FlowLMin : LREAL;
    TimeConstantS : LREAL;
END_VAR
TailVolumeL := FlowLMin * TimeConstantS / LREAL#60.0;
END_FUNCTION

(* Ratio station: Control.PID as PI (Kp = 10 % per L/min, Ki = 10 % per L/min·s, tracking time 0.5 s) makes the additive
   pump follow R·q_c. Below 5 L/min of carrier the ratio is undefined: the command goes to 0 % in the same update through
   Manual mode, no ratio is evaluated, and the tail volume is summed as V = V + q·Δt/60. The stop holds until Restart. *)
FUNCTION_BLOCK RatioStation
VAR_INPUT
    CarrierLMin : LREAL;
    AdditiveLMin : LREAL;
    Restart : BOOL;
END_VAR
VAR_OUTPUT
    SetpointLMin : LREAL := 10.0;
    CommandPct : LREAL := 50.0;
    TailL : LREAL := 0.0;
    RatioEvaluated : BOOL := TRUE;
    Stopped : BOOL := FALSE;
END_VAR
VAR
    Ratio : LREAL := 0.1;
    MinimumCarrierLMin : LREAL := 5.0;
    Loop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#10.0,
        IntegralGain := LREAL#10.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#0.5,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    Input : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
END_VAR
Loop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := LREAL#10.0;
    State.ProcessValue := LREAL#10.0;
    State.NominalOutput := LREAL#50.0;
    State.SaturatedOutput := LREAL#50.0;
    Loop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
IF Restart THEN
    Stopped := FALSE;
    TailL := LREAL#0.0;
END_IF;
Stopped := Stopped OR CarrierLMin < MinimumCarrierLMin;
Input.CycleTime := LREAL#0.1;
Input.ProcessValue := AdditiveLMin;
IF Stopped THEN
    RatioEvaluated := FALSE;
    SetpointLMin := LREAL#0.0;
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    TailL := TailL + AdditiveLMin * LREAL#0.1 / LREAL#60.0;
ELSE
    RatioEvaluated := TRUE;
    SetpointLMin := AdditiveSetpointLMin(Ratio := Ratio, CarrierLMin := CarrierLMin);
    Input.Mode := PIDMode#Auto;
END_IF;
Input.Setpoint := SetpointLMin;
CommandPct := Loop.Update(Input := Input);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Two ratio stations with R = 0.1, each driving its own additive pump: one sees the carrier step from 100 to
       150 L/min, the other the carrier stop from 100 to 0 L/min. *)
    StepStation : RatioStation;
    StopStation : RatioStation;
    StepAdditiveLMin : LREAL := 10.0;
    StepPumpPct : LREAL := 50.0;
    StopAdditiveLMin : LREAL := 10.0;
    StopPumpPct : LREAL := 50.0;
    StepCarrierLMin : LREAL := 100.0;
    StopCarrierLMin : LREAL := 100.0;
    StepSetpointLMin : LREAL := 10.0;
    StopSetpointLMin : LREAL := 10.0;
    (* The additive-to-carrier ratio of the stepped station, evaluated only while its carrier flows. *)
    MixRatio : LREAL := 0.1;
    TailL : LREAL := 0.0;
    (* Control Studio's project experiment steps one carrier and stops the other. *)
    ExperimentEnabled : BOOL := FALSE;
    StopRatioEvaluated : BOOL := FALSE;
    StepRatioEvaluated : BOOL := FALSE;
END_VAR

IF ExperimentEnabled THEN
    StepCarrierLMin := LREAL#150.0;
    StopCarrierLMin := LREAL#0.0;
ELSE
    StepCarrierLMin := LREAL#100.0;
    StopCarrierLMin := LREAL#100.0;
END_IF;
StepStation(CarrierLMin := StepCarrierLMin, AdditiveLMin := StepAdditiveLMin, Restart := NOT ExperimentEnabled);
StepPumpPct := StepStation.CommandPct;
StepSetpointLMin := StepStation.SetpointLMin;
StepRatioEvaluated := StepStation.RatioEvaluated;
IF StepRatioEvaluated THEN
    MixRatio := StepAdditiveLMin / StepCarrierLMin;
END_IF;
StopStation(CarrierLMin := StopCarrierLMin, AdditiveLMin := StopAdditiveLMin, Restart := NOT ExperimentEnabled);
StopPumpPct := StopStation.CommandPct;
StopSetpointLMin := StopStation.SetpointLMin;
StopRatioEvaluated := StopStation.RatioEvaluated;
TailL := StopStation.TailL;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.StepPumpPct AT %QL0 : LREAL;
    P1.StepAdditiveLMin AT %IL0 : LREAL;
    P1.StopPumpPct AT %QL8 : LREAL;
    P1.StopAdditiveLMin AT %IL8 : LREAL;
    P1.StepCarrierLMin AT %ML0 : LREAL;
    P1.StopCarrierLMin AT %ML8 : LREAL;
    P1.StepSetpointLMin AT %ML16 : LREAL;
    P1.StopSetpointLMin AT %ML24 : LREAL;
    P1.MixRatio AT %ML32 : LREAL;
    P1.TailL AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX48.0 : BOOL;
    P1.StopRatioEvaluated AT %MX48.1 : BOOL;
    P1.StepRatioEvaluated AT %MX48.2 : BOOL;
END_VAR
END_CONFIGURATION
