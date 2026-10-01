USING Control;

(* Feedforward from a measured heat extraction D_m: each command point delivers KwPerPoint of heat, so replacing D_m
   takes D_m/KwPerPoint points. *)
FUNCTION LoadFeedforwardPct : LREAL
VAR_INPUT
    MeasuredLoadKw : LREAL;
    KwPerPoint : LREAL;
END_VAR
LoadFeedforwardPct := MeasuredLoadKw / KwPerPoint;
END_FUNCTION

(* Until the requested heat arrives, the extraction cools the water as if the heater did nothing: after the delivery
   delay td the water has fallen by D/H·(1 − e^(−td/τ)), with the loss coefficient H in kW/°C and the time constant τ. *)
FUNCTION UnavoidableDropC : LREAL
VAR_INPUT
    LoadKw : LREAL;
    LossKwPerC : LREAL;
    TimeConstantS : LREAL;
    DelayS : LREAL;
END_VAR
UnavoidableDropC := LoadKw / LossKwPerC * (LREAL#1.0 - EXP(-DelayS / TimeConstantS));
END_FUNCTION

PROGRAM Main
VAR
    (* Three copies of the heater, each held at 35 °C by Control.PID as PI, Kp = 5 % per °C and Ki = 1 % per °C·s: the
       first alone, the second with feedforward from the measured load, the third with feedforward from an estimate
       20 % too high. *)
    PiOnlyLoop : Control.PID;
    FeedforwardLoop : Control.PID;
    BiasedLoop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0,
        IntegralGain := LREAL#1.0,
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
    State : Control.PIDInitialState;
    PiOnlyInput : Control.PIDInput;
    FeedforwardInput : Control.PIDInput;
    BiasedInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    PiOnlyC : LREAL := 35.0;
    PiOnlyPct : LREAL := 75.0;
    PiOnlyLoadKw : LREAL := 0.0;
    FeedforwardC : LREAL := 35.0;
    FeedforwardHeaterPct : LREAL := 75.0;
    FeedforwardLoadKw : LREAL := 0.0;
    BiasedC : LREAL := 35.0;
    BiasedHeaterPct : LREAL := 75.0;
    BiasedLoadKw : LREAL := 0.0;
    SetpointC : LREAL := 35.0;
    (* Control Studio's project experiment starts a 30 kW extraction on all three heaters. *)
    LoadKw : LREAL := 0.0;
    FeedforwardPct : LREAL := 0.0;
    BiasedFeedforwardPct : LREAL := 0.0;
    (* The feedback part of each feedforward heater's command: command − feedforward. *)
    FeedbackPct : LREAL := 75.0;
    BiasedFeedbackPct : LREAL := 75.0;
    ExperimentEnabled : BOOL := FALSE;
END_VAR

SetpointC := LREAL#35.0;
IF ExperimentEnabled THEN
    LoadKw := LREAL#30.0;
ELSE
    LoadKw := LREAL#0.0;
END_IF;
PiOnlyLoadKw := LoadKw;
FeedforwardLoadKw := LoadKw;
BiasedLoadKw := LoadKw;
FeedforwardPct := LoadFeedforwardPct(MeasuredLoadKw := LoadKw, KwPerPoint := LREAL#2.0);
BiasedFeedforwardPct := LoadFeedforwardPct(MeasuredLoadKw := LREAL#1.2 * LoadKw, KwPerPoint := LREAL#2.0);

PiOnlyLoop.Configure(Parameters := Parameters);
FeedforwardLoop.Configure(Parameters := Parameters);
BiasedLoop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointC;
    State.ProcessValue := SetpointC;
    State.NominalOutput := LREAL#75.0;
    State.SaturatedOutput := LREAL#75.0;
    PiOnlyLoop.Initialize(State := State);
    FeedforwardLoop.Initialize(State := State);
    BiasedLoop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
PiOnlyInput.Mode := PIDMode#Auto;
PiOnlyInput.CycleTime := LREAL#0.1;
PiOnlyInput.Setpoint := SetpointC;
PiOnlyInput.ProcessValue := PiOnlyC;
PiOnlyPct := PiOnlyLoop.Update(Input := PiOnlyInput);
FeedforwardInput.Mode := PIDMode#Auto;
FeedforwardInput.CycleTime := LREAL#0.1;
FeedforwardInput.Setpoint := SetpointC;
FeedforwardInput.ProcessValue := FeedforwardC;
FeedforwardInput.FeedForward := FeedforwardPct;
FeedforwardHeaterPct := FeedforwardLoop.Update(Input := FeedforwardInput);
BiasedInput.Mode := PIDMode#Auto;
BiasedInput.CycleTime := LREAL#0.1;
BiasedInput.Setpoint := SetpointC;
BiasedInput.ProcessValue := BiasedC;
BiasedInput.FeedForward := BiasedFeedforwardPct;
BiasedHeaterPct := BiasedLoop.Update(Input := BiasedInput);
FeedbackPct := FeedforwardHeaterPct - FeedforwardPct;
BiasedFeedbackPct := BiasedHeaterPct - BiasedFeedforwardPct;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PiOnlyPct AT %QL0 : LREAL;
    P1.PiOnlyLoadKw AT %QL8 : LREAL;
    P1.PiOnlyC AT %IL0 : LREAL;
    P1.FeedforwardHeaterPct AT %QL16 : LREAL;
    P1.FeedforwardLoadKw AT %QL24 : LREAL;
    P1.FeedforwardC AT %IL8 : LREAL;
    P1.BiasedHeaterPct AT %QL32 : LREAL;
    P1.BiasedLoadKw AT %QL40 : LREAL;
    P1.BiasedC AT %IL16 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.LoadKw AT %ML8 : LREAL;
    P1.FeedforwardPct AT %ML16 : LREAL;
    P1.BiasedFeedforwardPct AT %ML24 : LREAL;
    P1.FeedbackPct AT %ML32 : LREAL;
    P1.BiasedFeedbackPct AT %ML40 : LREAL;
    P1.ExperimentEnabled AT %MX48.0 : BOOL;
END_VAR
END_CONFIGURATION
