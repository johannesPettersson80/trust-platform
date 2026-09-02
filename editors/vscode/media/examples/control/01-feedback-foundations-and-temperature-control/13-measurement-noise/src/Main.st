USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Filter : Control.LowPass;
    FilterParameters : Control.LowPassParameters;
    FilteredValue : LREAL;
    FilterOutput : LREAL;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    ExperimentCycle : ULINT := ULINT#0;
    AutoMode : BOOL := FALSE;
    ConfigurationValid : BOOL := FALSE;
    FilterSelection : INT := 1;
    InitializedFilterSelection : INT := 0;
    Setpoint : LREAL := 0.0;
    ProcessValue : LREAL := 0.0;
    SimulatorTruePressure : LREAL := 0.0;
    ManipulatedValue : LREAL := 0.0;
    ManualOutput : LREAL := 0.0;
    ProportionalGainUsed : LREAL := 0.667;
    IntegralGainUsed : LREAL := 0.667;
    DerivativeGainUsed : LREAL := 0.0;
    FilterTimeConstantUsed : LREAL := 0.1;
    InitializedFilterTimeConstant : LREAL := 0.1;
    ConfiguredCycleTime : LREAL := 0.1;
    InitializedCycleTime : LREAL := 0.1;
END_VAR

IF NOT Initialized THEN
    (* Preserve an externally selected initialization-only filter while making
       the shipped process state unambiguously safe. *)
    Setpoint := LREAL#0.0;
    ManualOutput := LREAL#0.0;
    AutoMode := FALSE;
    FilterTimeConstantUsed := LREAL#0.1;
    ConfiguredCycleTime := LREAL#0.1;
    ProportionalGainUsed := LREAL#0.667;
    IntegralGainUsed := LREAL#0.667;
    DerivativeGainUsed := LREAL#0.0;
    ManualOutput := LREAL#0.0;
    Parameters.ProportionalGain := ProportionalGainUsed;
    Parameters.IntegralGain := IntegralGainUsed;
    Parameters.DerivativeGain := DerivativeGainUsed;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-10;
    Parameters.OutputMaximum := LREAL#10;
    Parameters.RateMinimum := LREAL#-100;
    Parameters.RateMaximum := LREAL#100;
    Parameters.TrackingTime := LREAL#1;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    Controller.Initialize(State := InitialState);
    FilterParameters.TimeConstant := FilterTimeConstantUsed;
    InitializedFilterSelection := FilterSelection;
    InitializedFilterTimeConstant := FilterTimeConstantUsed;
    InitializedCycleTime := ConfiguredCycleTime;
    (* 0 bypasses filtering, 1 selects one pole, and 2 selects two cascaded
       poles. The selection is fixed for each fresh comparison run. *)
    IF FilterSelection = 1 THEN
        FilterParameters.Order := FilterOrder#FirstOrder;
    ELSIF FilterSelection = 2 THEN
        FilterParameters.Order := FilterOrder#SecondOrder;
    ELSE
        (* Bypass and invalid selections use a valid internal filter object;
           invalid selections are denied automatic authority below. *)
        FilterParameters.Order := FilterOrder#FirstOrder;
    END_IF;
    Filter.Configure(Parameters := FilterParameters);
    Filter.Initialize(Input := ProcessValue);
    Initialized := TRUE;
END_IF;

(* The learner-visible project runs the selected first-order filter. Native
   evidence chooses 0, 1, or 2 before initialization in three fresh runtimes,
   then drives this same timing and restoration sequence. *)
IF ExperimentEnabled THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentCycle := ULINT#0;
    END_IF;
    ExperimentCycle := ExperimentCycle + ULINT#1;
    IF ExperimentCycle <= ULINT#50 THEN
        Setpoint := LREAL#0.0;
        ManualOutput := LREAL#0.0;
        AutoMode := TRUE;
    ELSIF ExperimentCycle <= ULINT#400 THEN
        Setpoint := LREAL#1.0;
        ManualOutput := LREAL#0.0;
        AutoMode := TRUE;
    ELSE
        Setpoint := LREAL#0.0;
        ManualOutput := LREAL#0.0;
        AutoMode := FALSE;
    END_IF;
ELSIF ExperimentWasEnabled THEN
    ExperimentCycle := ULINT#0;
    Setpoint := LREAL#0.0;
    ManualOutput := LREAL#0.0;
    AutoMode := FALSE;
END_IF;

(* Ordered range checks also reject NaN. A changed initialization-only setting
   requires a restart instead of silently changing the running filter state. *)
ConfigurationValid := (FilterSelection >= 0) AND (FilterSelection <= 2)
    AND (FilterSelection = InitializedFilterSelection)
    AND (FilterTimeConstantUsed > LREAL#0.0)
    AND (FilterTimeConstantUsed <= LREAL#10.0)
    AND (FilterTimeConstantUsed = InitializedFilterTimeConstant)
    AND (ConfiguredCycleTime > LREAL#0.0)
    AND (ConfiguredCycleTime <= LREAL#1.0)
    AND (ConfiguredCycleTime = InitializedCycleTime)
    AND (Setpoint >= LREAL#-5.0) AND (Setpoint <= LREAL#5.0)
    AND (ProcessValue >= LREAL#-5.0) AND (ProcessValue <= LREAL#5.0)
    AND (ManualOutput >= LREAL#-10.0) AND (ManualOutput <= LREAL#10.0);

Input.Setpoint := Setpoint;
FilteredValue := ProcessValue;
IF ConfigurationValid THEN
    FilterOutput := Filter.Update(
        Input := ProcessValue,
        CycleTime := ConfiguredCycleTime
    );
    IF FilterSelection <> 0 THEN
        FilteredValue := FilterOutput;
    END_IF;
END_IF;
Input.ProcessValue := FilteredValue;
IF ConfigurationValid THEN
    Input.ManualOutput := ManualOutput;
ELSE
    (* Invalid filter configuration removes both automatic authority and any
       operator request so the declared restoration is deterministically zero. *)
    Input.ManualOutput := LREAL#0.0;
END_IF;
IF AutoMode AND ConfigurationValid THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.CycleTime := ConfiguredCycleTime;
ManipulatedValue := Controller.Update(Input := Input);
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.ConfigurationValid AT %MX16.1 : BOOL;
    P1.ExperimentEnabled AT %MX88.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.FilteredValue AT %ML8 : LREAL;
    P1.ManualOutput AT %ML72 : LREAL;
    P1.FilterTimeConstantUsed AT %ML24 : LREAL;
    P1.ConfiguredCycleTime AT %ML32 : LREAL;
    P1.FilterSelection AT %MW40 : INT;
    P1.ProportionalGainUsed AT %ML48 : LREAL;
    P1.IntegralGainUsed AT %ML56 : LREAL;
    P1.DerivativeGainUsed AT %ML64 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.SimulatorTruePressure AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
