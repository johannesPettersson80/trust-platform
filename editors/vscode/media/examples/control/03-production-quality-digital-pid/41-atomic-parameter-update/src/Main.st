USING Control;

(* Fraction of the raw-minus-filtered difference applied per update: alpha = dt/(Tf + dt). *)
FUNCTION FilterFraction : LREAL
VAR_INPUT
    CycleTime : LREAL;
    FilterTime : LREAL;
END_VAR
FilterFraction := CycleTime / (FilterTime + CycleTime);
END_FUNCTION

(* First-order low-pass update: yf_new = yf_old + alpha·(yr − yf_old). *)
FUNCTION FilterUpdate : LREAL
VAR_INPUT
    PreviousFilteredValue : LREAL;
    RawValue : LREAL;
    FilterFraction : LREAL;
END_VAR
FilterUpdate := PreviousFilteredValue + FilterFraction * (RawValue - PreviousFilteredValue);
END_FUNCTION

(* An interval is usable only if time has advanced and the samples can still represent the
   loop's fastest oscillation. *)
FUNCTION IntervalUsable : BOOL
VAR_INPUT
    ElapsedTime : LREAL;
    MaximumInterval : LREAL;
END_VAR
IntervalUsable := ElapsedTime > LREAL#0.0 AND ElapsedTime <= MaximumInterval;
END_FUNCTION

(* Weighted proportional term about the operating point T0: P = Kp·(β·(SP − T0) − (PV − T0)). *)
FUNCTION WeightedProportional : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    SetpointWeight : LREAL;
    Setpoint : LREAL;
    ProcessValue : LREAL;
    OperatingPoint : LREAL;
END_VAR
WeightedProportional := ProportionalGain
    * (SetpointWeight * (Setpoint - OperatingPoint) - (ProcessValue - OperatingPoint));
END_FUNCTION

(* Integral state that makes P + I reproduce the applied command: I = u_applied − P. *)
FUNCTION InitialIntegral : LREAL
VAR_INPUT
    AppliedOutput : LREAL;
    Proportional : LREAL;
END_VAR
InitialIntegral := AppliedOutput - Proportional;
END_FUNCTION

(* A candidate set is usable only as a whole: non-negative gains, weights in 0–1, a minimum below the
   maximum, rate limits around zero, a positive tracking time and a non-negative filter time.
   Any NaN fails its comparison and rejects the set. *)
FUNCTION ValidPidParameters : BOOL
VAR_INPUT
    Candidate : Control.PIDParameters;
    FilterTime : LREAL;
END_VAR
ValidPidParameters :=
    Candidate.ProportionalGain >= LREAL#0.0 AND Candidate.IntegralGain >= LREAL#0.0
    AND Candidate.DerivativeGain >= LREAL#0.0
    AND Candidate.ProportionalSetpointWeight >= LREAL#0.0 AND Candidate.ProportionalSetpointWeight <= LREAL#1.0
    AND Candidate.DerivativeSetpointWeight >= LREAL#0.0 AND Candidate.DerivativeSetpointWeight <= LREAL#1.0
    AND Candidate.Bias = Candidate.Bias
    AND Candidate.OutputMinimum < Candidate.OutputMaximum
    AND Candidate.RateMinimum <= LREAL#0.0 AND Candidate.RateMaximum >= LREAL#0.0
    AND Candidate.TrackingTime > LREAL#0.0
    AND FilterTime >= LREAL#0.0;
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
    (* Tune replaces this record with each candidate; the defaults are the Lesson 24
       controller: relay gains, setpoint weight 0.3 and the 1 s measurement filter. *)
    Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#13.0,
        IntegralGain := LREAL#3.5,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#0.3,
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
    ConfiguredFilterTimeS : LREAL := 1.0;
    ParameterRevision : DINT := 0;
    ParametersRejected : BOOL := FALSE;
    InitialState : Control.PIDInitialState;
    ObservedState : Control.PIDInitialState;
    Input : Control.PIDInput;
    (* Ready: every retained value has been set from the actual operating condition. *)
    Initialized : BOOL := FALSE;
    InitializeRequest : BOOL := FALSE;
    InitializeRequestWas : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    AutoActive : BOOL := FALSE;
    (* TRUE at start: a program that starts in Auto from preset stored values continues without a
       transfer. A real start begins uninitialized in Manual, which clears it. *)
    AutoWasActive : BOOL := TRUE;
    (* FALSE runs the untracked counterexample: Manual leaves the stored values as initialized. *)
    TrackManual : BOOL := TRUE;
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
    (* Reference of the weighted proportional term: the 35 °C hot operating point. *)
    OperatingPointC : LREAL := 35.0;
    (* Measurement filter: Tf = 0 bypasses it; Control Studio applies Tf with the gains. *)
    FilterTimeS : LREAL := 1.0;
    FilterAlpha : LREAL := 1.0;
    FilteredC : LREAL := 35.0;
    FilterRestart : BOOL := TRUE;
    (* Timing: the controller updates every ControllerPeriodS of measured time. *)
    IntervalTimer : TON;
    SinceLastUpdateS : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    ControllerPeriodS : LREAL := 0.1;
    (* Longest usable interval: half the loop's 4.42 s ultimate period (Lesson 20). *)
    MaximumIntervalS : LREAL := 2.2;
    UpdateDue : BOOL := FALSE;
    TimingUsable : BOOL := FALSE;
    RejectedUpdates : DINT := 0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
END_VAR

(* The Lesson 21 heater: both feedback bindings observe the heated water, and the
   temperature measurement carries ±0.02 °C noise. *)
WaterTemperatureC := WaterInputC;
DataUsable := TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF DataUsable THEN
    SafeProcessValueC := TemperatureC;
ELSE
    SafetyAbort := TRUE;
END_IF;
(* Measured time since the previous update decision, read in the scan that acquires the measurement. *)
IntervalTimer(IN := TRUE, PT := T#1h);
SinceLastUpdateS := UDINT_TO_LREAL(DWORD_TO_UDINT(TIME_TO_DWORD(IntervalTimer.ET))) / LREAL#1000.0;

(* Parameter updates: Control Studio's Apply writes the complete record and FilterTimeS at one task
   boundary. The program adopts a changed set only as a whole and only if it is valid; an invalid set
   changes nothing, and the controller keeps running on the active set. *)
IF NOT Initialized OR NOT SamePidParameters(Left := Parameters, Right := ConfiguredParameters)
        OR FilterTimeS <> ConfiguredFilterTimeS THEN
    IF ValidPidParameters(Candidate := Parameters, FilterTime := FilterTimeS) THEN
        ConfiguredParameters := Parameters;
        ConfiguredFilterTimeS := FilterTimeS;
        ParameterRevision := ParameterRevision + 1;
        ParametersRejected := FALSE;
    ELSE
        ParametersRejected := TRUE;
    END_IF;
ELSE
    ParametersRejected := FALSE;
END_IF;
Controller.Configure(Parameters := ConfiguredParameters);

(* Hot reset: the program starts with the operator's manual command at the 75 % that holds the
   heater at 35 °C. *)
IF NOT Initialized THEN
    ManualOutput := LREAL#75.0;
END_IF;

(* One writer for the heater: a safety stop, else Manual until automatic control is active.
   Manual applies its own command and does not update the controller. *)
AutoActive := AutoMode AND Initialized AND NOT SafetyAbort
    AND ConfiguredParameters.ProportionalGain > LREAL#0.0 AND ConfiguredParameters.IntegralGain > LREAL#0.0;
IF SafetyAbort THEN
    (* An abort or invalid measurement removes heat and the test load until a full restart. *)
    AppliedPowerPct := LREAL#0.0;
    DisturbanceKw := LREAL#0.0;
ELSIF NOT AutoActive THEN
    AppliedPowerPct := LIMIT(MN := LREAL#0.0, IN := ManualOutput, MX := LREAL#100.0);
END_IF;

(* Initialization sets every retained value from the actual operating condition: at startup and
   once per rising edge of InitializeRequest, only while Manual owns the heater and the data is valid. *)
IF (NOT Initialized OR (InitializeRequest AND NOT InitializeRequestWas))
        AND NOT AutoActive AND NOT SafetyAbort AND DataUsable THEN
    IF NOT Initialized AND NOT AutoMode THEN
        SetpointC := SafeProcessValueC;
    END_IF;
    FilteredC := SafeProcessValueC;
    InitialState.Setpoint := SetpointC;
    InitialState.ProcessValue := FilteredC;
    InitialState.NominalOutput := AppliedPowerPct;
    InitialState.SaturatedOutput := AppliedPowerPct;
    InitialState.SetpointRate := LREAL#0.0;
    InitialState.ProcessValueRate := LREAL#0.0;
    InitialState.FeedForward := LREAL#0.0;
    Controller.Initialize(State := InitialState);
    ProportionalPct := WeightedProportional(ProportionalGain := ConfiguredParameters.ProportionalGain,
        SetpointWeight := ConfiguredParameters.ProportionalSetpointWeight, Setpoint := SetpointC,
        ProcessValue := FilteredC, OperatingPoint := OperatingPointC);
    IntegralPct := InitialIntegral(AppliedOutput := AppliedPowerPct, Proportional := ProportionalPct);
    RequestedPowerPct := AppliedPowerPct;
    Initialized := TRUE;
END_IF;
InitializeRequestWas := InitializeRequest;

(* Manual keeps the timing baseline current every scan, so the first automatic update integrates
   one scan, not the time spent in Manual. Auto updates every ControllerPeriodS. *)
UpdateDue := NOT AutoActive OR NOT AutoWasActive
    OR SinceLastUpdateS >= ControllerPeriodS - LREAL#0.001;
IF UpdateDue THEN
    ElapsedS := SinceLastUpdateS;
    TimingUsable := IntervalUsable(ElapsedTime := ElapsedS, MaximumInterval := MaximumIntervalS);
    IntervalTimer(IN := FALSE, PT := T#1h);
    IntervalTimer(IN := TRUE, PT := T#1h);
END_IF;
IF UpdateDue AND NOT TimingUsable THEN
    (* No state is updated over an unusable interval: the command is held, and the next
       accepted update restarts the filter from the fresh measurement. *)
    IF AutoActive THEN
        RejectedUpdates := RejectedUpdates + 1;
    END_IF;
    FilterRestart := TRUE;
ELSIF UpdateDue THEN
    (* The filter runs in Auto; in Manual, on the transfer scan and after a restart it takes the fresh measurement. *)
    IF AutoActive AND AutoWasActive AND NOT FilterRestart THEN
        FilterAlpha := FilterFraction(CycleTime := ElapsedS, FilterTime := ConfiguredFilterTimeS);
        FilteredC := FilterUpdate(PreviousFilteredValue := FilteredC, RawValue := SafeProcessValueC, FilterFraction := FilterAlpha);
    ELSE
        FilteredC := SafeProcessValueC;
    END_IF;
    FilterRestart := FALSE;
    Input.Setpoint := SetpointC;
    Input.ProcessValue := FilteredC;
    Input.CycleTime := ElapsedS;
    Input.FeedForward := LREAL#0.0;
    Input.TrackingOutput := AppliedPowerPct;
    Input.ManualOutput := AppliedPowerPct;
    ErrorC := SetpointC - FilteredC;
    IF AutoActive AND (AutoWasActive OR NOT TrackManual) THEN
        (* Ordinary PI update. *)
        Input.Mode := PIDMode#Auto;
        AppliedPowerPct := Controller.Update(Input := Input);
        AutoWasActive := TRUE;
    ELSIF TrackManual THEN
        (* Tracking: in Manual and on the transfer scan, the stored values follow the applied command,
           the setpoint and the measurement, so I = u_a − P. The command stays u_a: no integral step. *)
        Input.Mode := PIDMode#Manual;
        AppliedPowerPct := Controller.Update(Input := Input);
        AutoWasActive := AutoActive;
    END_IF;

    (* Diagnostics come from the actual controller state; the PI equations live in Control.PID. *)
    ObservedState := Controller.GetState();
    RequestedPowerPct := ObservedState.NominalOutput;
    ProportionalPct := WeightedProportional(ProportionalGain := ConfiguredParameters.ProportionalGain,
        SetpointWeight := ConfiguredParameters.ProportionalSetpointWeight, Setpoint := SetpointC,
        ProcessValue := FilteredC, OperatingPoint := OperatingPointC);
    IntegralPct := RequestedPowerPct - ProportionalPct;
END_IF;
IF NOT AutoActive THEN
    AutoWasActive := FALSE;
END_IF;
(* Command activity adds every applied-command movement, starting from the 75 % holding command. *)
CommandActivityPoints := CommandActivityPoints + ABS(AppliedPowerPct - PreviousAppliedPct);
PreviousAppliedPct := AppliedPowerPct;

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
    P1.CommandActivityPoints AT %ML64 : LREAL;
    P1.ManualOutput AT %ML72 : LREAL;
END_VAR
END_CONFIGURATION
