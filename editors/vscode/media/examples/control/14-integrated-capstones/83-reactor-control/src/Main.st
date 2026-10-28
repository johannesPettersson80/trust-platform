USING Control;

(* Heat balance at rest: the vessel loses 2·(Tv − 20) kW, supplied by the jacket at 10 kW per °C of difference. *)
FUNCTION HoldingJacketC : LREAL
VAR_INPUT
    VesselC : LREAL;
END_VAR
HoldingJacketC := VesselC + LREAL#0.2 * (VesselC - LREAL#20.0);
END_FUNCTION

(* The heater, 2 kW per point, supplies the vessel's loss and any jacket extraction. *)
FUNCTION HoldingCommandPct : LREAL
VAR_INPUT
    VesselC : LREAL;
    JacketLossKw : LREAL;
END_VAR
HoldingCommandPct := (LREAL#2.0 * (VesselC - LREAL#20.0) + JacketLossKw) / LREAL#2.0;
END_FUNCTION

(* A reading is usable only inside the vessel's 30–40 °C and the jacket's 20–55 °C. *)
FUNCTION ReadingsValid : BOOL
VAR_INPUT
    VesselC : LREAL;
    JacketC : LREAL;
END_VAR
ReadingsValid := VesselC >= LREAL#30.0 AND VesselC <= LREAL#40.0 AND JacketC >= LREAL#20.0 AND JacketC <= LREAL#55.0;
END_FUNCTION

(* One heater owner: the single-loop vessel PI or the cascade, selected by UseCascade. Every controller that does not
   command the heater runs in Manual and tracks: the vessel PI and the inner PI the applied command, the outer PI the
   jacket temperature. *)
FUNCTION_BLOCK ReactorTrain
VAR
    Single : Control.PID;
    Outer : Control.PID;
    Inner : Control.PID;
    SingleInput : Control.PIDInput;
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    PreviousApplied : LREAL := 15.0;
    AutomaticWas : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    AppliedPowerPct : LREAL := 15.0;
    SelectedJacketC : LREAL := 38.0;
    Faulted : BOOL := FALSE;
END_VAR
(* Gains, limits and initialization from the vessel at rest (Lesson 26): the vessel PI and the inner PI continue the
   applied 15 %, the outer PI the 38 °C jacket, each from its present setpoint and measurement. *)
METHOD PUBLIC Configure
VAR
    Parameters : Control.PIDParameters;
    Initial : Control.PIDInitialState;
END_VAR
Parameters.Bias := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.ProportionalGain := LREAL#2.5;
Parameters.IntegralGain := LREAL#0.1;
Single.Configure(Parameters := Parameters);
Initial.Setpoint := LREAL#35.0;
Initial.ProcessValue := LREAL#35.0;
Initial.NominalOutput := LREAL#15.0;
Initial.SaturatedOutput := LREAL#15.0;
Single.Initialize(State := Initial);
Parameters.ProportionalGain := LREAL#0.8717;
Parameters.IntegralGain := LREAL#0.21;
Parameters.OutputMinimum := LREAL#20.0;
Parameters.OutputMaximum := LREAL#50.0;
Outer.Configure(Parameters := Parameters);
Initial.NominalOutput := LREAL#38.0;
Initial.SaturatedOutput := LREAL#38.0;
Outer.Initialize(State := Initial);
Parameters.ProportionalGain := LREAL#25.25;
Parameters.IntegralGain := LREAL#13.59;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Inner.Configure(Parameters := Parameters);
Initial.Setpoint := LREAL#38.0;
Initial.ProcessValue := LREAL#38.0;
Initial.NominalOutput := LREAL#15.0;
Initial.SaturatedOutput := LREAL#15.0;
Inner.Initialize(State := Initial);
END_METHOD
(* A reading outside its range removes heat in the same update and latches (Lesson 29); no controller update uses it, and
   only a restart from the restored rest state clears the latch. Manual holds 15 %. The transfer update, the first with
   Automatic, is still a tracking update (Lesson 27); the selected arrangement runs in Auto from the next. *)
METHOD PUBLIC Update
VAR_INPUT
    SetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
    UseCascade : BOOL;
    Automatic : BOOL;
END_VAR
VAR
    Command : LREAL;
    Unused : LREAL;
END_VAR
IF NOT ReadingsValid(VesselC := VesselC, JacketC := JacketC) THEN
    Faulted := TRUE;
END_IF;
IF Faulted THEN
    AppliedPowerPct := LREAL#0.0;
ELSE
    IF Automatic THEN
        Command := PreviousApplied;
    ELSE
        Command := LREAL#15.0;
    END_IF;
    SingleInput.Mode := PIDMode#Manual;
    OuterInput.Mode := PIDMode#Manual;
    InnerInput.Mode := PIDMode#Manual;
    IF Automatic AND AutomaticWas THEN
        IF UseCascade THEN
            OuterInput.Mode := PIDMode#Auto;
            InnerInput.Mode := PIDMode#Auto;
        ELSE
            SingleInput.Mode := PIDMode#Auto;
        END_IF;
    END_IF;
    SingleInput.Setpoint := SetpointC;
    SingleInput.ProcessValue := VesselC;
    SingleInput.CycleTime := LREAL#0.1;
    SingleInput.ManualOutput := Command;
    OuterInput.Setpoint := SetpointC;
    OuterInput.ProcessValue := VesselC;
    OuterInput.CycleTime := LREAL#0.1;
    OuterInput.ManualOutput := JacketC;
    SelectedJacketC := Outer.Update(Input := OuterInput);
    InnerInput.Setpoint := SelectedJacketC;
    InnerInput.ProcessValue := JacketC;
    InnerInput.CycleTime := LREAL#0.1;
    InnerInput.ManualOutput := Command;
    IF UseCascade THEN
        AppliedPowerPct := Inner.Update(Input := InnerInput);
        SingleInput.ManualOutput := AppliedPowerPct;
        Unused := Single.Update(Input := SingleInput);
    ELSE
        AppliedPowerPct := Single.Update(Input := SingleInput);
        InnerInput.ManualOutput := AppliedPowerPct;
        Unused := Inner.Update(Input := InnerInput);
    END_IF;
    PreviousApplied := AppliedPowerPct;
END_IF;
AutomaticWas := Automatic AND NOT Faulted;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Five copies of the jacketed vessel: the single loop and the cascade on the 35 → 37 °C step, the single loop and the
       cascade on the 30 kW jacket loss, and the lifecycle case on the cascade. *)
    SingleStep : ReactorTrain;
    CascadeStep : ReactorTrain;
    SingleLoss : ReactorTrain;
    CascadeLoss : ReactorTrain;
    Lifecycle : ReactorTrain;
    Initialized : BOOL := FALSE;
    SingleStepJacketC : LREAL := 38.0;
    SingleStepVesselC : LREAL := 35.0;
    SingleStepPct : LREAL := 15.0;
    SingleStepLossKw : LREAL := 0.0;
    CascadeStepJacketC : LREAL := 38.0;
    CascadeStepVesselC : LREAL := 35.0;
    CascadeStepPct : LREAL := 15.0;
    CascadeStepLossKw : LREAL := 0.0;
    SingleLossJacketC : LREAL := 38.0;
    SingleLossVesselC : LREAL := 35.0;
    SingleLossPct : LREAL := 15.0;
    SingleLossKw : LREAL := 0.0;
    CascadeLossJacketC : LREAL := 38.0;
    CascadeLossVesselC : LREAL := 35.0;
    CascadeLossPct : LREAL := 15.0;
    CascadeLossKw : LREAL := 0.0;
    LifecycleJacketC : LREAL := 38.0;
    LifecycleVesselC : LREAL := 35.0;
    LifecyclePct : LREAL := 15.0;
    LifecycleLossKw : LREAL := 0.0;
    (* Control Studio's project experiment steps the setpoint to 37 °C and starts the 30 kW jacket loss. The lifecycle
       vessel stays in Manual until 10 s, and its vessel reading is 0 °C from 48 to 50 s. *)
    SetpointC : LREAL := 35.0;
    JacketLossKw : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    CascadeStepJacketSetpointC : LREAL := 38.0;
    CascadeLossJacketSetpointC : LREAL := 38.0;
    LifecycleJacketSetpointC : LREAL := 38.0;
    LifecycleReadingC : LREAL := 35.0;
    ExperimentEnabled : BOOL := FALSE;
    LifecycleAuto : BOOL := FALSE;
    LifecycleFault : BOOL := FALSE;
    LifecycleReadingValid : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    SingleStep.Configure();
    CascadeStep.Configure();
    SingleLoss.Configure();
    CascadeLoss.Configure();
    Lifecycle.Configure();
    Initialized := TRUE;
END_IF;
IF ExperimentEnabled THEN
    SetpointC := LREAL#37.0;
    JacketLossKw := LREAL#30.0;
ELSE
    SetpointC := LREAL#35.0;
    JacketLossKw := LREAL#0.0;
    ElapsedS := LREAL#0.0;
END_IF;
SingleStep.Update(SetpointC := SetpointC, VesselC := SingleStepVesselC, JacketC := SingleStepJacketC,
    UseCascade := FALSE, Automatic := TRUE);
SingleStepPct := SingleStep.AppliedPowerPct;
SingleStepLossKw := LREAL#0.0;
CascadeStep.Update(SetpointC := SetpointC, VesselC := CascadeStepVesselC, JacketC := CascadeStepJacketC,
    UseCascade := TRUE, Automatic := TRUE);
CascadeStepPct := CascadeStep.AppliedPowerPct;
CascadeStepJacketSetpointC := CascadeStep.SelectedJacketC;
CascadeStepLossKw := LREAL#0.0;
SingleLoss.Update(SetpointC := LREAL#35.0, VesselC := SingleLossVesselC, JacketC := SingleLossJacketC,
    UseCascade := FALSE, Automatic := TRUE);
SingleLossPct := SingleLoss.AppliedPowerPct;
SingleLossKw := JacketLossKw;
CascadeLoss.Update(SetpointC := LREAL#35.0, VesselC := CascadeLossVesselC, JacketC := CascadeLossJacketC,
    UseCascade := TRUE, Automatic := TRUE);
CascadeLossPct := CascadeLoss.AppliedPowerPct;
CascadeLossJacketSetpointC := CascadeLoss.SelectedJacketC;
CascadeLossKw := JacketLossKw;
LifecycleReadingC := LifecycleVesselC;
IF ExperimentEnabled AND ElapsedS >= LREAL#47.95 AND ElapsedS < LREAL#49.95 THEN
    LifecycleReadingC := LREAL#0.0;
END_IF;
LifecycleReadingValid := ReadingsValid(VesselC := LifecycleReadingC, JacketC := LifecycleJacketC);
Lifecycle.Update(SetpointC := SetpointC, VesselC := LifecycleReadingC, JacketC := LifecycleJacketC,
    UseCascade := TRUE, Automatic := ExperimentEnabled AND ElapsedS >= LREAL#9.95);
LifecyclePct := Lifecycle.AppliedPowerPct;
LifecycleJacketSetpointC := Lifecycle.SelectedJacketC;
LifecycleFault := Lifecycle.Faulted;
LifecycleAuto := ExperimentEnabled AND ElapsedS >= LREAL#9.95 AND NOT LifecycleFault;
LifecycleLossKw := LREAL#0.0;
IF ExperimentEnabled THEN
    ElapsedS := ElapsedS + LREAL#0.1;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SingleStepPct AT %QL0 : LREAL;
    P1.SingleStepLossKw AT %QL8 : LREAL;
    P1.SingleStepJacketC AT %IL0 : LREAL;
    P1.SingleStepVesselC AT %IL8 : LREAL;
    P1.CascadeStepPct AT %QL16 : LREAL;
    P1.CascadeStepLossKw AT %QL24 : LREAL;
    P1.CascadeStepJacketC AT %IL16 : LREAL;
    P1.CascadeStepVesselC AT %IL24 : LREAL;
    P1.SingleLossPct AT %QL32 : LREAL;
    P1.SingleLossKw AT %QL40 : LREAL;
    P1.SingleLossJacketC AT %IL32 : LREAL;
    P1.SingleLossVesselC AT %IL40 : LREAL;
    P1.CascadeLossPct AT %QL48 : LREAL;
    P1.CascadeLossKw AT %QL56 : LREAL;
    P1.CascadeLossJacketC AT %IL48 : LREAL;
    P1.CascadeLossVesselC AT %IL56 : LREAL;
    P1.LifecyclePct AT %QL64 : LREAL;
    P1.LifecycleLossKw AT %QL72 : LREAL;
    P1.LifecycleJacketC AT %IL64 : LREAL;
    P1.LifecycleVesselC AT %IL72 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.JacketLossKw AT %ML8 : LREAL;
    P1.CascadeStepJacketSetpointC AT %ML16 : LREAL;
    P1.CascadeLossJacketSetpointC AT %ML24 : LREAL;
    P1.LifecycleJacketSetpointC AT %ML32 : LREAL;
    P1.LifecycleReadingC AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX48.0 : BOOL;
    P1.LifecycleAuto AT %MX48.1 : BOOL;
    P1.LifecycleFault AT %MX48.2 : BOOL;
    P1.LifecycleReadingValid AT %MX48.3 : BOOL;
END_VAR
END_CONFIGURATION
