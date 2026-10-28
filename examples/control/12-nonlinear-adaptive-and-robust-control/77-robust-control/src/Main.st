USING Control;

(* The command that holds a heater's temperature: in the steady state the heater's power 2·u kW equals the heat loss
   H·(T − 20) plus the extraction D, so u = [H·(T − 20) + D]/2 points. *)
FUNCTION HoldingCommandPct : LREAL
VAR_INPUT
    LossKwPerC : LREAL;
    TemperatureC : LREAL;
    ExtractionKw : LREAL;
END_VAR
HoldingCommandPct := (LossKwPerC * (TemperatureC - LREAL#20.0) + ExtractionKw) / LREAL#2.0;
END_FUNCTION

(* W(ω), the bound on the relative model error |δ(jω)| of every heater of the grid against the reference heater with
   H = 10 kW/°C and L = 0.8 s: 2/√(64 + 2500·ω²) from the heat loss and min(0.4·ω, 2) from the extra delay. *)
FUNCTION ModelErrorBound : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
END_VAR
ModelErrorBound := LREAL#2.0 / SQRT(LREAL#64.0 + LREAL#2500.0 * OmegaRadS * OmegaRadS)
    + MIN(LREAL#0.4 * OmegaRadS, LREAL#2.0);
END_FUNCTION

(* |δ(jω)|, the relative model error of the heater with heat loss H and delay L against the reference heater:
   1 + δ = (50s + 10)/(50s + H)·e^(−(L − 0.8)s). *)
FUNCTION RelativeModelError : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
    LossKwPerC : LREAL;
    DelayS : LREAL;
END_VAR
VAR
    Denominator : LREAL;
    RatioReal : LREAL;
    RatioImag : LREAL;
    Shift : LREAL;
END_VAR
Denominator := LossKwPerC * LossKwPerC + LREAL#2500.0 * OmegaRadS * OmegaRadS;
RatioReal := (LREAL#10.0 * LossKwPerC + LREAL#2500.0 * OmegaRadS * OmegaRadS) / Denominator;
RatioImag := LREAL#50.0 * OmegaRadS * (LossKwPerC - LREAL#10.0) / Denominator;
Shift := (DelayS - LREAL#0.8) * OmegaRadS;
RelativeModelError := SQRT((RatioReal * COS(Shift) + RatioImag * SIN(Shift) - LREAL#1.0) ** LREAL#2.0
    + (RatioImag * COS(Shift) - RatioReal * SIN(Shift)) ** LREAL#2.0);
END_FUNCTION

(* |T0(jω)| = |L|/|1 + L| with L = C·G0, the PI C = Kp + Ki/s on the reference heater G0 = 0.2·e^(−0.8s)/(5s + 1). *)
FUNCTION ComplementaryMagnitude : LREAL
VAR_INPUT
    OmegaRadS : LREAL;
    Kp : LREAL;
    Ki : LREAL;
END_VAR
VAR
    Denominator : LREAL;
    PlantReal : LREAL;
    PlantImag : LREAL;
    LoopReal : LREAL;
    LoopImag : LREAL;
END_VAR
Denominator := LREAL#1.0 + LREAL#25.0 * OmegaRadS * OmegaRadS;
PlantReal := LREAL#0.2 * (COS(LREAL#0.8 * OmegaRadS) - LREAL#5.0 * OmegaRadS * SIN(LREAL#0.8 * OmegaRadS)) / Denominator;
PlantImag := LREAL#-0.2 * (SIN(LREAL#0.8 * OmegaRadS) + LREAL#5.0 * OmegaRadS * COS(LREAL#0.8 * OmegaRadS)) / Denominator;
(* C(jω) = Kp − j·Ki/ω. *)
LoopReal := Kp * PlantReal + Ki / OmegaRadS * PlantImag;
LoopImag := Kp * PlantImag - Ki / OmegaRadS * PlantReal;
ComplementaryMagnitude := SQRT(LoopReal * LoopReal + LoopImag * LoopImag)
    / SQRT((LREAL#1.0 + LoopReal) * (LREAL#1.0 + LoopReal) + LoopImag * LoopImag);
END_FUNCTION

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

PROGRAM Main
VAR
    Controller : Control.PID;
    (* The nominal heater of the range, 10 kW/°C with 1.0 s delivery delay. Control Studio validates the gains on the corner
       heaters of the range and applies them; the defaults are candidate A, Kp = 2.5 %/°C and Ki = 0.5 %/(°C·s). *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#2.5,
        IntegralGain := LREAL#0.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-10000.0,
        RateMaximum := LREAL#10000.0,
        TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation
    );
    ConfiguredParameters : Control.PIDParameters;
    ParameterRevision : DINT := 0;
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualOutput : LREAL := 75.0;
    SafetyAbort : BOOL := FALSE;
    DataUsable : BOOL := FALSE;
    SetpointC : LREAL := 35.0;
    DisturbanceKw : LREAL := 0.0;
    WaterInputC : LREAL := 35.0;
    WaterTemperatureC : LREAL := 35.0;
    TemperatureC : LREAL := 35.0;
    SafeProcessValueC : LREAL := 35.0;
    AppliedPowerPct : LREAL := 75.0;
    RequestedPowerPct : LREAL := 75.0;
    ErrorC : LREAL := 0.0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
END_VAR

(* Both feedback bindings observe the heated water. DisturbanceKw is a heat extraction in kW. *)
WaterTemperatureC := WaterInputC;
DataUsable := TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF DataUsable THEN
    SafeProcessValueC := TemperatureC;
ELSE
    SafetyAbort := TRUE;
END_IF;

(* Control Studio's Apply replaces this complete record at the task boundary; the
   program copies it into the actual controller and never overwrites it cyclically. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters) THEN
    ConfiguredParameters := Parameters;
    ParameterRevision := ParameterRevision + 1;
END_IF;
Controller.Configure(Parameters := Parameters);

IF NOT Initialized THEN
    (* Common hot reset: the heater already holds 35 °C with 75 % output, so the
       controller's retained output starts at the holding command. *)
    ManualOutput := LREAL#75.0;
    IF NOT AutoMode THEN
        SetpointC := SafeProcessValueC;
    END_IF;
    InitialState.Setpoint := SetpointC;
    InitialState.ProcessValue := SafeProcessValueC;
    InitialState.NominalOutput := ManualOutput;
    InitialState.SaturatedOutput := ManualOutput;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := SetpointC;
Input.ProcessValue := SafeProcessValueC;
Input.CycleTime := LREAL#0.1;
Input.FeedForward := LREAL#0.0;
Input.TrackingOutput := ManualOutput;
IF SafetyAbort THEN
    (* An abort or invalid measurement removes heat and the test load until a full restart. *)
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
ELSIF AutoMode AND Parameters.ProportionalGain > LREAL#0.0 AND Parameters.IntegralGain > LREAL#0.0 THEN
    Input.Mode := PIDMode#Auto;
    Input.ManualOutput := ManualOutput;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := ManualOutput;
END_IF;
ErrorC := SetpointC - SafeProcessValueC;
AppliedPowerPct := Controller.Update(Input := Input);

(* Diagnostics come from the actual controller state; the PI equations live in Control.PID. *)
ObservedState := Controller.GetState();
RequestedPowerPct := ObservedState.NominalOutput;
IF Input.Mode = PIDMode#Auto THEN
    ProportionalPct := Parameters.ProportionalGain * ErrorC;
ELSE
    ProportionalPct := LREAL#0.0;
END_IF;
IntegralPct := RequestedPowerPct - ProportionalPct;

END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SafetyAbort AT %MX96.3 : BOOL;
    P1.DataUsable AT %MX96.2 : BOOL;
    P1.AppliedPowerPct AT %QL0 : LREAL;
    P1.DisturbanceKw AT %QL8 : LREAL;
    P1.WaterInputC AT %IL0 : LREAL;
    P1.TemperatureC AT %IL8 : LREAL;
    P1.SetpointC AT %ML0 : LREAL;
    P1.IntegralPct AT %ML16 : LREAL;
    P1.RequestedPowerPct AT %ML32 : LREAL;
    P1.WaterTemperatureC AT %ML48 : LREAL;
    P1.ProportionalPct AT %ML56 : LREAL;
    P1.ManualOutput AT %ML72 : LREAL;
END_VAR
END_CONFIGURATION
