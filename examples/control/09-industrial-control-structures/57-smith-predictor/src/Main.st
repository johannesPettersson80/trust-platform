USING Control;

(* The controller of the delayed heater is fed y_pred = y_m,0 + (y − y_m,L) = y + c, where c = y_m,0 − y_m,L is the
   delay-free model's temperature minus the delayed model's. *)
FUNCTION PredictedTemperatureC : LREAL
VAR_INPUT
    MeasuredC : LREAL;
    CorrectionC : LREAL;
END_VAR
PredictedTemperatureC := MeasuredC + CorrectionC;
END_FUNCTION

(* Both models are the heater K/(T·s + 1), one driven by the applied command u and one by the command θ earlier, so their
   difference c obeys T·dc/dt = K·(u − u_θ) − c. One update of length h, exact while both commands are held. *)
FUNCTION NextCorrectionC : LREAL
VAR_INPUT
    CorrectionC : LREAL;
    CommandPct : LREAL;
    EarlierPct : LREAL;
    ModelGain : LREAL;
    ModelTimeConstantS : LREAL;
    UpdateS : LREAL;
END_VAR
VAR
    Decay : LREAL;
END_VAR
IF ModelTimeConstantS > LREAL#0.0 THEN
    Decay := EXP(-UpdateS / ModelTimeConstantS);
ELSE
    Decay := LREAL#0.0;
END_IF;
NextCorrectionC := CorrectionC * Decay + ModelGain * (CommandPct - EarlierPct) * (LREAL#1.0 - Decay);
END_FUNCTION

(* The heater of Lessons 12–30 with a 1 s delivery delay, held by Control.PID through a Smith predictor. Control Studio's
   Smith setup binds the controller (Parameters, Auto, ManualPct, SetpointC) and the model it identifies: ModelGain K,
   ModelTimeConstantS T, ModelDelayS θ and the correction CorrectionC. Until a design is applied the controller is in
   Manual at 75 %, with the slow PI of Lesson 19 (Kp 5, Ki 1) and no predictor (K = 0). *)
PROGRAM Main
VAR
    Loop : Control.PID;
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
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    FirstScan : BOOL := TRUE;
    Auto : BOOL := FALSE;
    ManualPct : LREAL := 75.0;
    SetpointC : LREAL := 35.0;
    WaterC : LREAL := 35.0;
    HeaterPct : LREAL := 75.0;
    ModelGain : LREAL := 0.0;
    ModelTimeConstantS : LREAL := 5.0;
    ModelDelayS : LREAL := 0.0;
    CorrectionC : LREAL := 0.0;
    PredictedC : LREAL := 35.0;
    History : ARRAY[0..99] OF LREAL;
    Newest : INT := 0;
    DelaySamples : INT := 0;
    Index : INT;
    EarlierPct : LREAL := 75.0;
END_VAR

Loop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    (* Memory-mapped variables start from the %M image; set the resting values explicitly. *)
    SetpointC := LREAL#35.0;
    ManualPct := LREAL#75.0;
    State.Setpoint := SetpointC;
    State.ProcessValue := WaterC;
    State.NominalOutput := ManualPct;
    State.SaturatedOutput := ManualPct;
    Loop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
PredictedC := PredictedTemperatureC(MeasuredC := WaterC, CorrectionC := CorrectionC);
IF Auto THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
(* The command history over the delay is refilled with the applied command while not in Auto and on the first scan, so
   the correction starts from zero. *)
IF FirstScan OR NOT Auto THEN
    FOR Index := 0 TO 99 DO
        History[Index] := HeaterPct;
    END_FOR;
    FirstScan := FALSE;
END_IF;
Input.CycleTime := LREAL#0.1;
Input.Setpoint := SetpointC;
Input.ProcessValue := PredictedC;
Input.ManualOutput := ManualPct;
HeaterPct := Loop.Update(Input := Input);
DelaySamples := MIN(99, MAX(0, LREAL_TO_INT(ModelDelayS / LREAL#0.1)));
Newest := (Newest + 1) MOD 100;
History[Newest] := HeaterPct;
EarlierPct := History[(Newest - DelaySamples + 100) MOD 100];
CorrectionC := NextCorrectionC(CorrectionC := CorrectionC, CommandPct := HeaterPct, EarlierPct := EarlierPct,
    ModelGain := ModelGain, ModelTimeConstantS := ModelTimeConstantS, UpdateS := LREAL#0.1);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.WaterC AT %IL0 : LREAL;
    P1.HeaterPct AT %QL0 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.ManualPct AT %ML16 : LREAL;
END_VAR
END_CONFIGURATION
