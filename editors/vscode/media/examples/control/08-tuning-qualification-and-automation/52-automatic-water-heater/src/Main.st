USING Control;

PROGRAM Main
VAR
    CurrentController : Control.PID;
    CandidateController : Control.PID;
    CurrentParameters : Control.PIDParameters;
    CandidateParameters : Control.PIDParameters;
    CurrentState : Control.PIDInitialState;
    CandidateState : Control.PIDInitialState;
    CurrentInput : Control.PIDInput;
    CandidateInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    TemperatureSetpoint : LREAL := 20.0;
    CurrentRequested : LREAL := 20.0;
    CandidateRequested : LREAL := 20.0;
    CurrentApplied : LREAL := 20.0;
    CandidateApplied : LREAL := 20.0;
    CurrentTemperature : LREAL := 20.0;
    CandidateTemperature : LREAL := 20.0;
END_VAR

IF NOT Initialized THEN
    (* The first instance is the confirmed Robust controller. Automatic Tune
       may validate the second copy, but it does not replace this instance. *)
    CurrentParameters.ProportionalGain := LREAL#3.260869565;
    CurrentParameters.IntegralGain := LREAL#0.652173913;
    CurrentParameters.DerivativeGain := LREAL#0.0;
    CurrentParameters.ProportionalSetpointWeight := LREAL#1.0;
    CurrentParameters.DerivativeSetpointWeight := LREAL#0.0;
    CurrentParameters.Bias := LREAL#20.0;
    CurrentParameters.OutputMinimum := LREAL#0.0;
    CurrentParameters.OutputMaximum := LREAL#100.0;
    CurrentParameters.RateMinimum := LREAL#-1000.0;
    CurrentParameters.RateMaximum := LREAL#1000.0;
    CurrentParameters.TrackingTime := LREAL#1.0;
    CurrentParameters.AntiWindup := PIDAntiWindup#Combined;
    CurrentController.Configure(Parameters := CurrentParameters);

    CandidateParameters := CurrentParameters;
    CandidateParameters.ProportionalGain := LREAL#5.769230769;
    CandidateParameters.IntegralGain := LREAL#1.153846154;
    CandidateController.Configure(Parameters := CandidateParameters);

    (* Both copies start at the same physical equilibrium so the validation
       measures controller behavior rather than an artificial startup step. *)
    CurrentState.Setpoint := LREAL#20.0;
    CurrentState.ProcessValue := CurrentTemperature;
    CurrentState.NominalOutput := LREAL#20.0;
    CurrentState.SaturatedOutput := LREAL#20.0;
    CurrentController.Initialize(State := CurrentState);
    CandidateState.Setpoint := LREAL#20.0;
    CandidateState.ProcessValue := CandidateTemperature;
    CandidateState.NominalOutput := LREAL#20.0;
    CandidateState.SaturatedOutput := LREAL#20.0;
    CandidateController.Initialize(State := CandidateState);
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled AND (NOT SafetyAbort) AND InputsValid THEN
    TemperatureSetpoint := LREAL#25.0;
ELSE
    TemperatureSetpoint := LREAL#20.0;
END_IF;

CurrentInput.Setpoint := TemperatureSetpoint;
CurrentInput.ProcessValue := CurrentTemperature;
CurrentInput.Mode := PIDMode#Auto;
CurrentInput.CycleTime := LREAL#0.1;
CurrentRequested := CurrentController.Update(Input := CurrentInput);
CurrentApplied := MAX(MIN(CurrentRequested, LREAL#100.0), LREAL#0.0);

CandidateInput.Setpoint := TemperatureSetpoint;
CandidateInput.ProcessValue := CandidateTemperature;
CandidateInput.Mode := PIDMode#Auto;
CandidateInput.CycleTime := LREAL#0.1;
CandidateRequested := CandidateController.Update(Input := CandidateInput);
CandidateApplied := MAX(MIN(CandidateRequested, LREAL#100.0), LREAL#0.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.InputsValid AT %MX96.2 : BOOL;
    P1.TemperatureSetpoint AT %ML0 : LREAL;
    P1.CurrentRequested AT %ML128 : LREAL;
    P1.CandidateRequested AT %ML136 : LREAL;
    P1.CurrentApplied AT %QL0 : LREAL;
    P1.CandidateApplied AT %QL8 : LREAL;
    P1.CurrentTemperature AT %IL0 : LREAL;
    P1.CandidateTemperature AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
