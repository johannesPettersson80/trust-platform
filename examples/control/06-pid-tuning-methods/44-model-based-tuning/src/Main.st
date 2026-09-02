USING Control;

PROGRAM Main
VAR
    HeaterController : Control.PID;
    TankController : Control.PID;
    DryerController : Control.PID;
    HeaterParameters : Control.PIDParameters;
    TankParameters : Control.PIDParameters;
    DryerParameters : Control.PIDParameters;
    HeaterState : Control.PIDInitialState;
    TankState : Control.PIDInitialState;
    DryerState : Control.PIDInitialState;
    HeaterInput : Control.PIDInput;
    TankInput : Control.PIDInput;
    DryerInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    HeaterKp : LREAL := 5.769230769;
    HeaterKi : LREAL := 1.153846154;
    TankKp : LREAL := 13.333333333;
    TankKi : LREAL := 2.222222222;
    DryerKp : LREAL := 2.083333333;
    DryerKi : LREAL := 0.104166667;
    HeaterSetpoint : LREAL := 20.0;
    TankSetpoint : LREAL := 50.0;
    DryerSetpoint : LREAL := 40.0;
    HeaterRequested : LREAL := 20.0;
    TankRequested : LREAL := 50.0;
    DryerRequested : LREAL := 30.0;
    HeaterApplied : LREAL := 20.0;
    TankApplied : LREAL := 50.0;
    DryerApplied : LREAL := 30.0;
    HeaterTemperature : LREAL := 20.0;
    TankLevel : LREAL := 50.0;
    DryerDryness : LREAL := 40.0;
    AutomaticMode : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    (* Control Studio supplies reviewed candidate gains. The PLC executes them;
       it does not identify a model, rank candidates, or apply a winner. *)
    HeaterParameters.ProportionalGain := HeaterKp;
    HeaterParameters.IntegralGain := HeaterKi;
    HeaterParameters.DerivativeGain := LREAL#0.0;
    HeaterParameters.ProportionalSetpointWeight := LREAL#1.0;
    HeaterParameters.DerivativeSetpointWeight := LREAL#0.0;
    HeaterParameters.Bias := LREAL#20.0;
    HeaterParameters.OutputMinimum := LREAL#0.0;
    HeaterParameters.OutputMaximum := LREAL#100.0;
    HeaterParameters.RateMinimum := LREAL#-1000.0;
    HeaterParameters.RateMaximum := LREAL#1000.0;
    HeaterParameters.TrackingTime := LREAL#1.0;
    HeaterParameters.AntiWindup := PIDAntiWindup#Combined;
    HeaterController.Configure(Parameters := HeaterParameters);

    TankParameters := HeaterParameters;
    TankParameters.ProportionalGain := TankKp;
    TankParameters.IntegralGain := TankKi;
    TankParameters.Bias := LREAL#50.0;
    TankController.Configure(Parameters := TankParameters);

    DryerParameters := HeaterParameters;
    DryerParameters.ProportionalGain := DryerKp;
    DryerParameters.IntegralGain := DryerKi;
    DryerParameters.Bias := LREAL#30.0;
    DryerController.Configure(Parameters := DryerParameters);

    (* Initialize at each physical equilibrium so startup is not mistaken for
       response-target performance. *)
    HeaterState.Setpoint := LREAL#20.0;
    HeaterState.ProcessValue := HeaterTemperature;
    HeaterState.NominalOutput := LREAL#20.0;
    HeaterState.SaturatedOutput := LREAL#20.0;
    HeaterController.Initialize(State := HeaterState);
    TankState.Setpoint := LREAL#50.0;
    TankState.ProcessValue := TankLevel;
    TankState.NominalOutput := LREAL#50.0;
    TankState.SaturatedOutput := LREAL#50.0;
    TankController.Initialize(State := TankState);
    DryerState.Setpoint := LREAL#40.0;
    DryerState.ProcessValue := DryerDryness;
    DryerState.NominalOutput := LREAL#30.0;
    DryerState.SaturatedOutput := LREAL#30.0;
    DryerController.Initialize(State := DryerState);
    Initialized := TRUE;
END_IF;

InputsValid := HeaterTemperature >= LREAL#0.0
    AND HeaterTemperature <= LREAL#60.0
    AND TankLevel >= LREAL#0.0 AND TankLevel <= LREAL#100.0
    AND DryerDryness >= LREAL#0.0 AND DryerDryness <= LREAL#100.0;
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    HeaterSetpoint := LREAL#25.0;
ELSE
    HeaterSetpoint := LREAL#20.0;
END_IF;

(* One reviewed heater-temperature step drives equivalent normalized tests on
   all three plants, so Control Studio owns one command and every comparison
   starts on the same scan. *)
TankSetpoint := LREAL#50.0 + LREAL#0.4 * (HeaterSetpoint - LREAL#20.0);
DryerSetpoint := LREAL#40.0 + (HeaterSetpoint - LREAL#20.0);

HeaterInput.Setpoint := HeaterSetpoint;
HeaterInput.ProcessValue := HeaterTemperature;
HeaterInput.Mode := PIDMode#Auto;
HeaterInput.CycleTime := LREAL#0.1;
HeaterRequested := HeaterController.Update(Input := HeaterInput);
HeaterApplied := MAX(MIN(HeaterRequested, LREAL#100.0), LREAL#0.0);

TankInput.Setpoint := TankSetpoint;
TankInput.ProcessValue := TankLevel;
TankInput.Mode := PIDMode#Auto;
TankInput.CycleTime := LREAL#0.1;
TankRequested := TankController.Update(Input := TankInput);
TankApplied := MAX(MIN(TankRequested, LREAL#100.0), LREAL#0.0);

DryerInput.Setpoint := DryerSetpoint;
DryerInput.ProcessValue := DryerDryness;
DryerInput.Mode := PIDMode#Auto;
DryerInput.CycleTime := LREAL#0.1;
DryerRequested := DryerController.Update(Input := DryerInput);
DryerApplied := MAX(MIN(DryerRequested, LREAL#100.0), LREAL#0.0);
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.1 : BOOL;
    P1.SafetyAbort AT %MX96.2 : BOOL;
    P1.InputsValid AT %MX96.3 : BOOL;
    P1.HeaterSetpoint AT %ML0 : LREAL;
    P1.TankSetpoint AT %ML8 : LREAL;
    P1.DryerSetpoint AT %ML16 : LREAL;
    P1.HeaterKp AT %ML24 : LREAL;
    P1.HeaterKi AT %ML32 : LREAL;
    P1.TankKp AT %ML40 : LREAL;
    P1.TankKi AT %ML48 : LREAL;
    P1.DryerKp AT %ML56 : LREAL;
    P1.DryerKi AT %ML64 : LREAL;
    P1.HeaterRequested AT %ML72 : LREAL;
    P1.TankRequested AT %ML80 : LREAL;
    P1.DryerRequested AT %ML88 : LREAL;
    P1.AutomaticMode AT %MX96.0 : BOOL;
    P1.HeaterApplied AT %QL0 : LREAL;
    P1.TankApplied AT %QL8 : LREAL;
    P1.DryerApplied AT %QL16 : LREAL;
    P1.HeaterTemperature AT %IL0 : LREAL;
    P1.TankLevel AT %IL8 : LREAL;
    P1.DryerDryness AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
