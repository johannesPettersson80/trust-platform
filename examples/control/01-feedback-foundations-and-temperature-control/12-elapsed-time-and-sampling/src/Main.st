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

(* Integral change over the elapsed interval: dI = Ki·e·dt. *)
FUNCTION IntegralChange : LREAL
VAR_INPUT
    IntegralGain : LREAL;
    Error : LREAL;
    ElapsedTime : LREAL;
END_VAR
IntegralChange := IntegralGain * Error * ElapsedTime;
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

(* Value of a unit sine of the given frequency (Hz) at sample number k, taken every SampleInterval seconds. *)
FUNCTION SampledSine : LREAL
VAR_INPUT
    Frequency : LREAL;
    SampleInterval : LREAL;
    SampleNumber : DINT;
END_VAR
SampledSine := SIN(LREAL#6.283185307179586 * Frequency * SampleInterval * DINT_TO_LREAL(SampleNumber));
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
    (* Reference of the weighted proportional term: the 35 °C hot operating point. *)
    OperatingPointC : LREAL := 35.0;
    (* Measurement filter: Tf = 0 bypasses it; Control Studio applies Tf with the gains. *)
    FilterTimeS : LREAL := 1.0;
    FilterAlpha : LREAL := 1.0;
    FilteredC : LREAL := 35.0;
    FilterActive : BOOL := FALSE;
    FilterWasActive : BOOL := FALSE;
    (* Sampling and timing: the controller updates every ControllerPeriodS of measured time. *)
    IntervalTimer : TON;
    SinceLastUpdateS : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    ControllerPeriodS : LREAL := 0.1;
    DelayUpdateS : LREAL := 0.0;
    (* Longest usable interval: half the loop's 4.42 s ultimate period (Lesson 20). *)
    MaximumIntervalS : LREAL := 2.2;
    (* 0 uses the measured interval; a positive value assumes that fixed interval instead. *)
    AssumedCycleS : LREAL := 0.0;
    UpdateDue : BOOL := FALSE;
    TimingUsable : BOOL := FALSE;
    RejectedUpdates : DINT := 0;
    ProportionalPct : LREAL := 0.0;
    IntegralPct : LREAL := 75.0;
    PreviousAppliedPct : LREAL := 75.0;
    CommandActivityPoints : LREAL := 0.0;
END_VAR

(* The Lesson 21 heater: both feedback bindings observe the heated water, and the
   temperature measurement carries ±0.02 °C noise. The controller uses the filtered value
   and updates on the measured clock. *)
WaterTemperatureC := WaterInputC;
DataUsable := TemperatureC >= LREAL#0.0 AND TemperatureC <= LREAL#80.0;
IF DataUsable THEN
    SafeProcessValueC := TemperatureC;
ELSE
    SafetyAbort := TRUE;
END_IF;
(* Measured time since the previous update decision, read in the scan that acquires the
   measurement. The timer restarts at every decision, so it never runs into its PT limit. *)
IntervalTimer(IN := TRUE, PT := T#1h);
SinceLastUpdateS := UDINT_TO_LREAL(DWORD_TO_UDINT(TIME_TO_DWORD(IntervalTimer.ET))) / LREAL#1000.0;

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
    FilteredC := SafeProcessValueC;
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

(* Manual tracks every scan; Auto entry and a safety stop update at once; otherwise the
   controller waits until ControllerPeriodS (plus any one-off delay) of measured time has passed,
   less the 1 ms clock resolution. *)
FilterActive := AutoMode AND NOT SafetyAbort AND Parameters.ProportionalGain > LREAL#0.0
    AND Parameters.IntegralGain > LREAL#0.0;
UpdateDue := NOT FilterActive OR NOT FilterWasActive
    OR SinceLastUpdateS >= ControllerPeriodS + DelayUpdateS - LREAL#0.001;
IF UpdateDue THEN
    ElapsedS := SinceLastUpdateS;
    TimingUsable := IntervalUsable(ElapsedTime := ElapsedS, MaximumInterval := MaximumIntervalS);
    (* The next interval is measured from this acquisition, whether or not this one was usable. *)
    IntervalTimer(IN := FALSE, PT := T#1h);
    IntervalTimer(IN := TRUE, PT := T#1h);
    DelayUpdateS := LREAL#0.0;
END_IF;
IF UpdateDue AND NOT TimingUsable THEN
    (* No state is updated over an unusable interval: the command is held, and the next
       accepted update restarts the filter from the fresh measurement, as on entering Auto. *)
    RejectedUpdates := RejectedUpdates + 1;
    FilterWasActive := FALSE;
    IF SafetyAbort THEN
        AppliedPowerPct := LREAL#0.0;
        DisturbanceKw := LREAL#0.0;
    END_IF;
ELSIF UpdateDue THEN
    (* The filter runs in Auto with the measured interval. In Manual and in the Auto-entry update
       it restarts from the fresh measurement, so entering Auto is exactly bumpless. *)
    IF FilterActive AND FilterWasActive THEN
        FilterAlpha := FilterFraction(CycleTime := ElapsedS, FilterTime := MAX(FilterTimeS, LREAL#0.0));
        FilteredC := FilterUpdate(PreviousFilteredValue := FilteredC, RawValue := SafeProcessValueC, FilterFraction := FilterAlpha);
    ELSE
        FilteredC := SafeProcessValueC;
    END_IF;
    Input.Setpoint := SetpointC;
    Input.ProcessValue := FilteredC;
    (* The integral must use the measured interval; an assumed 0.1 s is the wrong arithmetic. *)
    Input.CycleTime := SEL(AssumedCycleS > LREAL#0.0, ElapsedS, AssumedCycleS);
    Input.FeedForward := LREAL#0.0;
    Input.TrackingOutput := ManualOutput;
    IF SafetyAbort THEN
        (* An abort or invalid measurement removes heat and the test load until a full restart. *)
        Input.Mode := PIDMode#Manual;
        Input.ManualOutput := LREAL#0.0;
        DisturbanceKw := LREAL#0.0;
    ELSIF FilterActive THEN
        Input.Mode := PIDMode#Auto;
        Input.ManualOutput := ManualOutput;
    ELSE
        Input.Mode := PIDMode#Manual;
        Input.ManualOutput := ManualOutput;
    END_IF;
    ErrorC := SetpointC - FilteredC;
    AppliedPowerPct := Controller.Update(Input := Input);
    FilterWasActive := FilterActive;

    (* Diagnostics come from the actual controller state; the PI equations live in Control.PID. *)
    ObservedState := Controller.GetState();
    RequestedPowerPct := ObservedState.NominalOutput;
    IF Input.Mode = PIDMode#Auto THEN
        (* Weighted P = Kp·(β·(SP − T0) − (PV − T0)); the integral uses the full error. *)
        ProportionalPct := Parameters.ProportionalGain * (Parameters.ProportionalSetpointWeight * (SetpointC - OperatingPointC)
            - (FilteredC - OperatingPointC));
    ELSE
        ProportionalPct := LREAL#0.0;
    END_IF;
    IntegralPct := RequestedPowerPct - ProportionalPct;
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
