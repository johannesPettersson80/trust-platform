USING Control;

FUNCTION SamePidParameters : BOOL
VAR_INPUT
    Left : Control.PIDParameters;
    Right : Control.PIDParameters;
END_VAR
SamePidParameters :=
    Left.ProportionalGain = Right.ProportionalGain
    AND Left.IntegralGain = Right.IntegralGain
    AND Left.DerivativeGain = Right.DerivativeGain
    AND Left.ProportionalSetpointWeight = Right.ProportionalSetpointWeight
    AND Left.DerivativeSetpointWeight = Right.DerivativeSetpointWeight
    AND Left.Bias = Right.Bias
    AND Left.OutputMinimum = Right.OutputMinimum
    AND Left.OutputMaximum = Right.OutputMaximum
    AND Left.RateMinimum = Right.RateMinimum
    AND Left.RateMaximum = Right.RateMaximum
    AND Left.TrackingTime = Right.TrackingTime
    AND Left.AntiWindup = Right.AntiWindup;
END_FUNCTION

(* P without a bias makes the inlet pump match the outlet only from an error: e = u_out / Kp. *)
FUNCTION ProportionalLevelOffset : LREAL
VAR_INPUT
    OutletCommandPct : LREAL;
    ProportionalGain : LREAL;
END_VAR
ProportionalLevelOffset := OutletCommandPct / ProportionalGain;
END_FUNCTION

(* SIMC for an integrating process K' e^(-Ls) / s: Kp = 1 / (K' (lambda + L)). *)
FUNCTION IntegratingProportionalGain : LREAL
VAR_INPUT
    RateGain : LREAL;
    ResponseTimeS : LREAL;
    DelayS : LREAL;
END_VAR
IntegratingProportionalGain := LREAL#1.0 / (RateGain * (ResponseTimeS + DelayS));
END_FUNCTION

(* SIMC's integral time for an integrating process, Ti = 4 (lambda + L): with Kp above, Kp K' Ti = 4. *)
FUNCTION IntegratingIntegralTime : LREAL
VAR_INPUT
    ResponseTimeS : LREAL;
    DelayS : LREAL;
END_VAR
IntegratingIntegralTime := LREAL#4.0 * (ResponseTimeS + DelayS);
END_FUNCTION

PROGRAM Main
VAR
    Controller : Control.PID;
    (* Control Studio's Tune replaces the gains. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#100.0,
        IntegralGain := LREAL#2.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#10.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualPumpPct : LREAL := 30.0;
    SafetyAbort : BOOL := FALSE;
    SetpointM : LREAL := 1.0;
    LevelM : LREAL := 1.0;
    InletPumpPct : LREAL := 30.0;
    (* The next process step draws from the tank with the outlet pump; the load trial changes it. *)
    OutletPumpPct : LREAL := 30.0;
    LevelLowM : LREAL := 0.2;
    LevelHighM : LREAL := 1.8;
END_VAR

(* A level outside 0.2–1.8 m stops both pumps until a full restart. *)
IF LevelM < LevelLowM OR LevelM > LevelHighM THEN
    SafetyAbort := TRUE;
END_IF;

(* Control Studio's Apply replaces this complete record at the task boundary. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);

IF NOT Initialized THEN
    (* Hot reset: the inlet pump matches the outlet at 30 % and the target is the present level. *)
    ManualPumpPct := LREAL#30.0;
    IF NOT AutoMode THEN
        SetpointM := LevelM;
    END_IF;
    InitialState.Setpoint := SetpointM;
    InitialState.ProcessValue := LevelM;
    InitialState.NominalOutput := ManualPumpPct;
    InitialState.SaturatedOutput := ManualPumpPct;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := SetpointM;
Input.ProcessValue := LevelM;
Input.CycleTime := LREAL#0.1;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualPumpPct;
IF SafetyAbort THEN
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    OutletPumpPct := LREAL#0.0;
ELSIF AutoMode AND Parameters.ProportionalGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualPumpPct;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualPumpPct;
END_IF;
InletPumpPct := Controller.Update(Input := Input);

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX16.0 : BOOL;
    P1.InletPumpPct AT %QL0 : LREAL;
    P1.OutletPumpPct AT %QL8 : LREAL;
    P1.LevelM AT %IL0 : LREAL;
    P1.SetpointM AT %ML0 : LREAL;
    P1.ManualPumpPct AT %ML8 : LREAL;
END_VAR
END_CONFIGURATION
