USING Control;

(* A water heater held at 35 °C by Control.PID. The program adds a feedforward from the measured heat extraction
   LoadKw through a lead/lag: FeedForward = K_ff·(z + T_lead/T_lag·(d − z)), with z the extraction through the lag. *)
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
    Auto : BOOL := FALSE;
    ManualPct : LREAL := 80.0;
    SetpointC : LREAL := 35.0;
    WaterC : LREAL := 35.0;
    HeaterPct : LREAL := 80.0;
    LoadKw : LREAL := 10.0;
    PlantLoadKw : LREAL := 10.0;
    FeedforwardGain : LREAL := 0.0;
    LeadS : LREAL := 1.0;
    LagS : LREAL := 1.0;
    FilteredLoadKw : LREAL := 10.0;
    FeedforwardPct : LREAL := 0.0;
END_VAR

PlantLoadKw := LoadKw;
FilteredLoadKw := FilteredLoadKw + LREAL#0.1 / (LagS + LREAL#0.1) * (LoadKw - FilteredLoadKw);
FeedforwardPct := FeedforwardGain * (FilteredLoadKw + LeadS / LagS * (LoadKw - FilteredLoadKw));
Loop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointC;
    State.ProcessValue := WaterC;
    State.FeedForward := FeedforwardPct;
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
Input.CycleTime := LREAL#0.1;
Input.Setpoint := SetpointC;
Input.ProcessValue := WaterC;
Input.FeedForward := FeedforwardPct;
Input.ManualOutput := ManualPct;
HeaterPct := Loop.Update(Input := Input);
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
END_VAR
END_CONFIGURATION
