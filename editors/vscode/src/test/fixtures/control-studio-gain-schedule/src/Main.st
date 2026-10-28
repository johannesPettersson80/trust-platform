USING Control;

(* A gravity tank's level held by Control.PID. The program interpolates the PI gains by the measured level between the
   gains of the low and the high operating point; Control Studio tunes each point's pair on the model identified there. *)
PROGRAM Main
VAR
    Loop : Control.PID;
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#0.05,
        IntegralGain := LREAL#0.002,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#0.03,
        RateMinimum := LREAL#-1.0,
        RateMaximum := LREAL#1.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    Auto : BOOL := FALSE;
    ManualM3PerS : LREAL := 0.006;
    SetpointM : LREAL := 0.09;
    LevelM : LREAL := 0.09;
    InflowM3PerS : LREAL := 0.006;
    KpLow : LREAL := 0.05;
    KiLow : LREAL := 0.002;
    KpHigh : LREAL := 0.05;
    KiHigh : LREAL := 0.002;
    LowLevelM : LREAL := 0.09;
    HighLevelM : LREAL := 0.49;
    Weight : LREAL := 0.0;
END_VAR

Weight := LIMIT(LREAL#0.0, (LevelM - LowLevelM) / (HighLevelM - LowLevelM), LREAL#1.0);
Parameters.ProportionalGain := KpLow + Weight * (KpHigh - KpLow);
Parameters.IntegralGain := KiLow + Weight * (KiHigh - KiLow);
Loop.Configure(Parameters := Parameters);
IF NOT Initialized THEN
    State.Setpoint := SetpointM;
    State.ProcessValue := LevelM;
    State.NominalOutput := ManualM3PerS;
    State.SaturatedOutput := ManualM3PerS;
    Loop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
IF Auto THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.CycleTime := LREAL#1.0;
Input.Setpoint := SetpointM;
Input.ProcessValue := LevelM;
Input.ManualOutput := ManualM3PerS;
InflowM3PerS := Loop.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.LevelM AT %IL0 : LREAL;
    P1.InflowM3PerS AT %QL0 : LREAL;
    P1.SetpointM AT %ML0 : LREAL;
    P1.ManualM3PerS AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
