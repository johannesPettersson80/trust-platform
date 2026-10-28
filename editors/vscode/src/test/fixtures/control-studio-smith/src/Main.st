USING Control;

(* A water heater with a 1 s delay held by Control.PID through a Smith predictor: the PID reads the measurement plus the
   correction c, the delay-free model's response to how much the output changed over the delay, T·dc/dt = K·(u − u_θ) − c.
   Control Studio sets K, T and θ from the identified model. *)
PROGRAM Main
VAR
    Loop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.0,
        IntegralGain := LREAL#0.2,
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
    ModelTimeConstantS : LREAL := 1.0;
    ModelDelayS : LREAL := 0.0;
    CorrectionC : LREAL := 0.0;
    PredictedC : LREAL := 35.0;
    History : ARRAY[0..99] OF LREAL;
    Newest : INT := 0;
    DelaySamples : INT := 0;
    Index : INT;
    EarlierPct : LREAL := 75.0;
END_VAR

PredictedC := WaterC + CorrectionC;
Loop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointC;
    State.ProcessValue := WaterC;
    State.NominalOutput := ManualPct;
    State.SaturatedOutput := ManualPct;
    Loop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
IF Auto THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
(* The output's history over the delay, refilled with the output being applied, before the controller's update, while not
   in Auto and on the first scan. *)
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
CorrectionC := CorrectionC + LREAL#0.1 / (ModelTimeConstantS + LREAL#0.1) * (ModelGain * (HeaterPct - EarlierPct) - CorrectionC);
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
