USING Control;

(* The command that holds a heater's temperature: in the steady state the heater's power a·u = 2·u kW equals the heat loss
   H·(T − 20) plus the extraction D, so u = [H·(T − 20) + D]/2 points. *)
FUNCTION HoldingCommandPct : LREAL
VAR_INPUT
    LossKwPerC : LREAL;
    TemperatureC : LREAL;
    ExtractionKw : LREAL;
END_VAR
HoldingCommandPct := (LossKwPerC * (TemperatureC - LREAL#20.0) + ExtractionKw) / LREAL#2.0;
END_FUNCTION

(* Lesson 62's feedforward: a measured extraction D needs D/a points, with a = 2 kW per point. *)
FUNCTION FeedforwardPct : LREAL
VAR_INPUT
    ExtractionKw : LREAL;
END_VAR
FeedforwardPct := ExtractionKw / LREAL#2.0;
END_FUNCTION

(* The fall during the 1 s delivery delay, before the feedforward heat arrives: with 50·dx/dt = −H·x − D for 1 s from x = 0,
   x(1) = −(D/H)(1 − e^(−H/50)). *)
FUNCTION DelayDipC : LREAL
VAR_INPUT
    LossKwPerC : LREAL;
    ExtractionKw : LREAL;
END_VAR
DelayDipC := ExtractionKw / LossKwPerC * (LREAL#1.0 - EXP(-LossKwPerC / LREAL#50.0));
END_FUNCTION

(* The declared decision rule: the simplest candidate that passes every case. 1 = A (PI), 2 = B (PI with feedforward, which
   needs the extraction measured), 0 = neither qualifies: redesign. *)
FUNCTION Recommendation : LREAL
VAR_INPUT
    PassedA : LREAL;
    PassedB : LREAL;
    Cases : LREAL;
END_VAR
IF PassedA >= Cases THEN
    Recommendation := LREAL#1.0;
ELSIF PassedB >= Cases THEN
    Recommendation := LREAL#2.0;
ELSE
    Recommendation := LREAL#0.0;
END_IF;
END_FUNCTION

PROGRAM Main
VAR
    (* The production line's heater: 10 kW/°C with 1 s delivery delay, held at 35 °C; a draw-off takes 10 kW. Control Studio
       takes its model, tunes the gains and the feedforward settings, validates them on heaters of 8 and 12 kW/°C and
       applies them. The defaults are candidate A: Kp = 2.5 %/°C and Ki = 0.5 %/(°C·s) with no feedforward. The measured draw-off LoadKw is fed forward
       through a lead/lag, FeedForward = K_ff·(z + T_lead/T_lag·(d − z)) with z the draw-off through the lag; with equal lead
       and lag it is K_ff·d. A temperature outside 0–80 °C latches SafetyAbort, which removes heat and the draw-off until
       a restart. *)
    Controller : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
        IntegralGain := LREAL#0.5,
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
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    Auto : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    ManualPct : LREAL := 75.0;
    SetpointC : LREAL := 35.0;
    WaterC : LREAL := 35.0;
    HeaterPct : LREAL := 75.0;
    LoadKw : LREAL := 0.0;
    PlantLoadKw : LREAL := 0.0;
    FeedforwardGain : LREAL := 0.0;
    LeadS : LREAL := 1.0;
    LagS : LREAL := 1.0;
    FilteredLoadKw : LREAL := 0.0;
    FeedforwardCommandPct : LREAL := 0.0;
END_VAR

IF WaterC < LREAL#0.0 OR WaterC > LREAL#80.0 THEN
    SafetyAbort := TRUE;
END_IF;
IF SafetyAbort THEN
    PlantLoadKw := LREAL#0.0;
ELSE
    PlantLoadKw := LoadKw;
END_IF;
FilteredLoadKw := FilteredLoadKw + LREAL#0.1 / (LagS + LREAL#0.1) * (LoadKw - FilteredLoadKw);
FeedforwardCommandPct := FeedforwardGain * (FilteredLoadKw + LeadS / LagS * (LoadKw - FilteredLoadKw));
Controller.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointC;
    State.ProcessValue := WaterC;
    State.FeedForward := FeedforwardCommandPct;
    State.NominalOutput := ManualPct;
    State.SaturatedOutput := ManualPct;
    Controller.Initialize(State := State);
    Initialized := TRUE;
END_IF;
Input.CycleTime := LREAL#0.1;
Input.Setpoint := SetpointC;
Input.ProcessValue := WaterC;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    Input.FeedForward := LREAL#0.0;
ELSIF Auto THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualPct;
    Input.FeedForward := FeedforwardCommandPct;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualPct;
    Input.FeedForward := FeedforwardCommandPct;
END_IF;
HeaterPct := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.WaterC AT %IL0 : LREAL;
    P1.HeaterPct AT %QL0 : LREAL;
    P1.PlantLoadKw AT %QL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.LoadKw AT %ML8 : LREAL;
    P1.ManualPct AT %ML16 : LREAL;
    P1.FeedforwardCommandPct AT %ML24 : LREAL;
    P1.SafetyAbort AT %MX32.0 : BOOL;
END_VAR
END_CONFIGURATION
