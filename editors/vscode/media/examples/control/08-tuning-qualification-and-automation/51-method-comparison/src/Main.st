USING Control;

PROGRAM Main
VAR
    HeaterSimcController : Control.PID;
    HeaterAmigoController : Control.PID;
    TankSimcController : Control.PID;
    DryerSimcController : Control.PID;
    DryerAmigoController : Control.PID;
    HeaterSimcParameters : Control.PIDParameters;
    HeaterAmigoParameters : Control.PIDParameters;
    TankSimcParameters : Control.PIDParameters;
    DryerSimcParameters : Control.PIDParameters;
    DryerAmigoParameters : Control.PIDParameters;
    HeaterSimcState : Control.PIDInitialState;
    HeaterAmigoState : Control.PIDInitialState;
    TankSimcState : Control.PIDInitialState;
    DryerSimcState : Control.PIDInitialState;
    DryerAmigoState : Control.PIDInitialState;
    HeaterSimcInput : Control.PIDInput;
    HeaterAmigoInput : Control.PIDInput;
    TankSimcInput : Control.PIDInput;
    DryerSimcInput : Control.PIDInput;
    DryerAmigoInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    TankAmigoEligible : BOOL := FALSE;
    HeaterSetpoint : LREAL := 20.0;
    TankSetpoint : LREAL := 50.0;
    DryerSetpoint : LREAL := 40.0;
    HeaterSimcRequested : LREAL := 20.0;
    HeaterAmigoRequested : LREAL := 20.0;
    TankSimcRequested : LREAL := 50.0;
    DryerSimcRequested : LREAL := 30.0;
    DryerAmigoRequested : LREAL := 30.0;
    HeaterSimcApplied : LREAL := 20.0;
    HeaterAmigoApplied : LREAL := 20.0;
    TankSimcApplied : LREAL := 50.0;
    DryerSimcApplied : LREAL := 30.0;
    DryerAmigoApplied : LREAL := 30.0;
    HeaterSimcTemperature : LREAL := 20.0;
    HeaterAmigoTemperature : LREAL := 20.0;
    TankSimcLevel : LREAL := 50.0;
    DryerSimcDryness : LREAL := 40.0;
    DryerAmigoDryness : LREAL := 40.0;
END_VAR

IF NOT Initialized THEN
    (* Both methods use the same parallel PI form. Only their reviewed gains
       differ; method qualification and comparison stay in Control Studio. *)
    HeaterSimcParameters.ProportionalGain := LREAL#5.769230769;
    HeaterSimcParameters.IntegralGain := LREAL#1.153846154;
    HeaterSimcParameters.DerivativeGain := LREAL#0.0;
    HeaterSimcParameters.ProportionalSetpointWeight := LREAL#1.0;
    HeaterSimcParameters.DerivativeSetpointWeight := LREAL#0.0;
    HeaterSimcParameters.Bias := LREAL#20.0;
    HeaterSimcParameters.OutputMinimum := LREAL#0.0;
    HeaterSimcParameters.OutputMaximum := LREAL#100.0;
    HeaterSimcParameters.RateMinimum := LREAL#-1000.0;
    HeaterSimcParameters.RateMaximum := LREAL#1000.0;
    HeaterSimcParameters.TrackingTime := LREAL#1.0;
    HeaterSimcParameters.AntiWindup := PIDAntiWindup#Combined;
    HeaterSimcController.Configure(Parameters := HeaterSimcParameters);

    HeaterAmigoParameters := HeaterSimcParameters;
    HeaterAmigoParameters.ProportionalGain := LREAL#6.027777778;
    HeaterAmigoParameters.IntegralGain := LREAL#1.552507154;
    HeaterAmigoController.Configure(Parameters := HeaterAmigoParameters);

    TankSimcParameters := HeaterSimcParameters;
    TankSimcParameters.ProportionalGain := LREAL#13.333333333;
    TankSimcParameters.IntegralGain := LREAL#2.222222222;
    TankSimcParameters.Bias := LREAL#50.0;
    TankSimcController.Configure(Parameters := TankSimcParameters);

    DryerSimcParameters := HeaterSimcParameters;
    DryerSimcParameters.ProportionalGain := LREAL#2.083333333;
    DryerSimcParameters.IntegralGain := LREAL#0.104166667;
    DryerSimcParameters.Bias := LREAL#30.0;
    DryerSimcController.Configure(Parameters := DryerSimcParameters);

    DryerAmigoParameters := DryerSimcParameters;
    DryerAmigoParameters.ProportionalGain := LREAL#1.286989796;
    DryerAmigoParameters.IntegralGain := LREAL#0.072185590;
    DryerAmigoController.Configure(Parameters := DryerAmigoParameters);

    (* Initialize at each physical equilibrium so startup is not counted as
       tuning performance or actuator movement. *)
    HeaterSimcState.Setpoint := LREAL#20.0;
    HeaterSimcState.ProcessValue := HeaterSimcTemperature;
    HeaterSimcState.NominalOutput := LREAL#20.0;
    HeaterSimcState.SaturatedOutput := LREAL#20.0;
    HeaterSimcController.Initialize(State := HeaterSimcState);
    HeaterAmigoState.Setpoint := LREAL#20.0;
    HeaterAmigoState.ProcessValue := HeaterAmigoTemperature;
    HeaterAmigoState.NominalOutput := LREAL#20.0;
    HeaterAmigoState.SaturatedOutput := LREAL#20.0;
    HeaterAmigoController.Initialize(State := HeaterAmigoState);
    TankSimcState.Setpoint := LREAL#50.0;
    TankSimcState.ProcessValue := TankSimcLevel;
    TankSimcState.NominalOutput := LREAL#50.0;
    TankSimcState.SaturatedOutput := LREAL#50.0;
    TankSimcController.Initialize(State := TankSimcState);
    DryerSimcState.Setpoint := LREAL#40.0;
    DryerSimcState.ProcessValue := DryerSimcDryness;
    DryerSimcState.NominalOutput := LREAL#30.0;
    DryerSimcState.SaturatedOutput := LREAL#30.0;
    DryerSimcController.Initialize(State := DryerSimcState);
    DryerAmigoState.Setpoint := LREAL#40.0;
    DryerAmigoState.ProcessValue := DryerAmigoDryness;
    DryerAmigoState.NominalOutput := LREAL#30.0;
    DryerAmigoState.SaturatedOutput := LREAL#30.0;
    DryerAmigoController.Initialize(State := DryerAmigoState);
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled AND (NOT SafetyAbort) AND InputsValid THEN
    HeaterSetpoint := LREAL#25.0;
    TankSetpoint := LREAL#52.0;
    DryerSetpoint := LREAL#45.0;
ELSE
    HeaterSetpoint := LREAL#20.0;
    TankSetpoint := LREAL#50.0;
    DryerSetpoint := LREAL#40.0;
END_IF;

HeaterSimcInput.Setpoint := HeaterSetpoint;
HeaterSimcInput.ProcessValue := HeaterSimcTemperature;
HeaterSimcInput.Mode := PIDMode#Auto;
HeaterSimcInput.CycleTime := LREAL#0.1;
HeaterSimcRequested := HeaterSimcController.Update(Input := HeaterSimcInput);
HeaterSimcApplied := MAX(MIN(HeaterSimcRequested, LREAL#100.0), LREAL#0.0);

HeaterAmigoInput.Setpoint := HeaterSetpoint;
HeaterAmigoInput.ProcessValue := HeaterAmigoTemperature;
HeaterAmigoInput.Mode := PIDMode#Auto;
HeaterAmigoInput.CycleTime := LREAL#0.1;
HeaterAmigoRequested := HeaterAmigoController.Update(Input := HeaterAmigoInput);
HeaterAmigoApplied := MAX(MIN(HeaterAmigoRequested, LREAL#100.0), LREAL#0.0);

TankSimcInput.Setpoint := TankSetpoint;
TankSimcInput.ProcessValue := TankSimcLevel;
TankSimcInput.Mode := PIDMode#Auto;
TankSimcInput.CycleTime := LREAL#0.1;
TankSimcRequested := TankSimcController.Update(Input := TankSimcInput);
TankSimcApplied := MAX(MIN(TankSimcRequested, LREAL#100.0), LREAL#0.0);

DryerSimcInput.Setpoint := DryerSetpoint;
DryerSimcInput.ProcessValue := DryerSimcDryness;
DryerSimcInput.Mode := PIDMode#Auto;
DryerSimcInput.CycleTime := LREAL#0.1;
DryerSimcRequested := DryerSimcController.Update(Input := DryerSimcInput);
DryerSimcApplied := MAX(MIN(DryerSimcRequested, LREAL#100.0), LREAL#0.0);

DryerAmigoInput.Setpoint := DryerSetpoint;
DryerAmigoInput.ProcessValue := DryerAmigoDryness;
DryerAmigoInput.Mode := PIDMode#Auto;
DryerAmigoInput.CycleTime := LREAL#0.1;
DryerAmigoRequested := DryerAmigoController.Update(Input := DryerAmigoInput);
DryerAmigoApplied := MAX(MIN(DryerAmigoRequested, LREAL#100.0), LREAL#0.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.2 : BOOL;
    P1.InputsValid AT %MX96.3 : BOOL;
    P1.TankAmigoEligible AT %MX96.1 : BOOL;
    P1.HeaterSetpoint AT %ML0 : LREAL;
    P1.TankSetpoint AT %ML8 : LREAL;
    P1.DryerSetpoint AT %ML16 : LREAL;
    P1.HeaterSimcRequested AT %ML128 : LREAL;
    P1.HeaterAmigoRequested AT %ML136 : LREAL;
    P1.TankSimcRequested AT %ML144 : LREAL;
    P1.DryerSimcRequested AT %ML152 : LREAL;
    P1.DryerAmigoRequested AT %ML160 : LREAL;
    P1.HeaterSimcApplied AT %QL0 : LREAL;
    P1.HeaterAmigoApplied AT %QL8 : LREAL;
    P1.TankSimcApplied AT %QL16 : LREAL;
    P1.DryerSimcApplied AT %QL24 : LREAL;
    P1.DryerAmigoApplied AT %QL32 : LREAL;
    P1.HeaterSimcTemperature AT %IL0 : LREAL;
    P1.HeaterAmigoTemperature AT %IL8 : LREAL;
    P1.TankSimcLevel AT %IL16 : LREAL;
    P1.DryerSimcDryness AT %IL24 : LREAL;
    P1.DryerAmigoDryness AT %IL32 : LREAL;
END_VAR
END_CONFIGURATION
