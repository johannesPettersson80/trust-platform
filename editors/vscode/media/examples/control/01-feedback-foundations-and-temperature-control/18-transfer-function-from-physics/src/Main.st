USING Control;

(* Closed loop of the heater G(s) = 0.2/(5s + 1) under P control: H(s) = 0.2·Kp/(5s + 1 + 0.2·Kp), a first-order
   response with the gain 0.2·Kp/(1 + 0.2·Kp) and the time constant 5/(1 + 0.2·Kp). *)
FUNCTION ClosedLoopGain : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
END_VAR
ClosedLoopGain := LREAL#0.2 * ProportionalGain / (LREAL#1.0 + LREAL#0.2 * ProportionalGain);
END_FUNCTION

FUNCTION ClosedLoopTimeS : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
END_VAR
ClosedLoopTimeS := LREAL#5.0 / (LREAL#1.0 + LREAL#0.2 * ProportionalGain);
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    (* P control about the 35 °C operating point: u = 75 % + Kp·(SP − T), Kp = 5 % per °C. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0,
        IntegralGain := LREAL#0.0,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#75.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    SetpointC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    HeaterPowerPct : LREAL := 75.0;
    ExtractionKw : LREAL := 0.0;
    (* Predictions of the step response, in °C: the transfer function H(s) without the delivery delay, and the
       same loop recomputed with the 1 s delay: 5·dx/dt + x = 0.2·v(t − 1) with v = Kp·(r − x), advanced every
       0.1 s update with the exact held-input step. *)
    NoDelayPredictionC : LREAL := 0.0;
    DelayedModelC : LREAL := 0.0;
    ModelDeviationC : LREAL := 0.0;
    ModelCommandDelay : ARRAY[0..9] OF LREAL;
    DelayIndex : INT := 0;
    ElapsedS : LREAL := 0.0;
    (* Control Studio's project experiment steps the setpoint to 36 °C while it runs. *)
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    Active : BOOL := FALSE;
    StepC : LREAL := 0.0;
    Decay : LREAL := 0.0;
    DelayedCommand : LREAL := 0.0;
    Count : INT := 0;
END_VAR

IF ExperimentEnabled AND NOT ExperimentWasEnabled THEN
    Active := TRUE;
    ElapsedS := LREAL#0.0;
    ModelDeviationC := LREAL#0.0;
    DelayedCommand := LREAL#0.0;
    FOR Count := 0 TO 9 DO
        ModelCommandDelay[Count] := LREAL#0.0;
    END_FOR;
ELSIF NOT ExperimentEnabled THEN
    Active := FALSE;
END_IF;
ExperimentWasEnabled := ExperimentEnabled;
IF Active THEN
    StepC := LREAL#1.0;
ELSE
    StepC := LREAL#0.0;
END_IF;
SetpointC := LREAL#35.0 + StepC;

Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointC;
    State.ProcessValue := TemperatureC;
    State.NominalOutput := LREAL#75.0;
    State.SaturatedOutput := LREAL#75.0;
    Controller.Initialize(State := State);
    Initialized := TRUE;
END_IF;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
Input.Setpoint := SetpointC;
Input.ProcessValue := TemperatureC;
HeaterPowerPct := Controller.Update(Input := Input);

(* Delay-free prediction from the transfer function: x(t) = Kcl·(1 − e^(−t/τcl)). *)
NoDelayPredictionC := LREAL#35.0 + StepC * ClosedLoopGain(ProportionalGain := Parameters.ProportionalGain)
    * (LREAL#1.0 - EXP(-ElapsedS / ClosedLoopTimeS(ProportionalGain := Parameters.ProportionalGain)));
(* The same loop with the delay. The heat that entered the water during the last 0.1 s was commanded 1.1 s
   ago: 1 s of delivery delay after the update that held it. *)
IF Active THEN
    Decay := EXP(LREAL#-0.1 / LREAL#5.0);
    ModelDeviationC := Decay * ModelDeviationC + (LREAL#1.0 - Decay) * LREAL#0.2 * DelayedCommand;
    DelayedCommand := ModelCommandDelay[DelayIndex];
    ModelCommandDelay[DelayIndex] := Parameters.ProportionalGain * (StepC - ModelDeviationC);
    DelayIndex := (DelayIndex + 1) MOD 10;
    ElapsedS := ElapsedS + LREAL#0.1;
END_IF;
DelayedModelC := LREAL#35.0 + ModelDeviationC;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.HeaterPowerPct AT %QL0 : LREAL;
    P1.ExtractionKw AT %QL8 : LREAL;
    P1.TemperatureC AT %IL0 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.NoDelayPredictionC AT %ML8 : LREAL;
    P1.DelayedModelC AT %ML16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.Active AT %MX24.1 : BOOL;
END_VAR
END_CONFIGURATION
