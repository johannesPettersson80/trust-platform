USING Control;

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    ManualMode : BOOL := FALSE;
    TemperatureQualityGood : BOOL := TRUE;
    TemperatureSetpoint : LREAL := 35.0;
    ManualOutput : LREAL := 60.0;
    CycleTime : LREAL := 0.1;
    PTemperature : LREAL := 35.0;
    HistoricalTemperature : LREAL := 35.0;
    CurrentTemperature : LREAL := 35.0;
    CandidateTemperature : LREAL := 35.0;
    PRequested : LREAL := 75.0;
    HistoricalRequested : LREAL := 75.0;
    CurrentRequested : LREAL := 75.0;
    CandidateRequested : LREAL := 75.0;
    PApplied : LREAL := 75.0;
    HistoricalApplied : LREAL := 75.0;
    CurrentApplied : LREAL := 75.0;
    CandidateApplied : LREAL := 75.0;
    ControllerRevision : DINT := DINT#77;
    CandidateUnderReview : INT := INT#4;
    Initialized : BOOL := FALSE;
    PController : Control.PID;
    HistoricalController : Control.PID;
    CurrentController : Control.PID;
    CandidateController : Control.PID;
    PParameters : Control.PIDParameters;
    HistoricalParameters : Control.PIDParameters;
    CurrentParameters : Control.PIDParameters;
    CandidateParameters : Control.PIDParameters;
    PState : Control.PIDInitialState;
    HistoricalState : Control.PIDInitialState;
    CurrentState : Control.PIDInitialState;
    CandidateState : Control.PIDInitialState;
    PInput : Control.PIDInput;
    HistoricalInput : Control.PIDInput;
    CurrentInput : Control.PIDInput;
    CandidateInput : Control.PIDInput;
END_VAR

IF NOT Initialized THEN
    TemperatureSetpoint := LREAL#35.0;
    TemperatureQualityGood := TRUE;
    PParameters.ProportionalGain := LREAL#2.0;
    PParameters.IntegralGain := LREAL#0.0;
    PParameters.DerivativeGain := LREAL#0.0;
    PParameters.ProportionalSetpointWeight := LREAL#1.0;
    PParameters.DerivativeSetpointWeight := LREAL#0.0;
    PParameters.Bias := LREAL#75.0;
    PParameters.OutputMinimum := LREAL#0.0;
    PParameters.OutputMaximum := LREAL#100.0;
    PParameters.RateMinimum := LREAL#-1000.0;
    PParameters.RateMaximum := LREAL#1000.0;
    PParameters.TrackingTime := LREAL#1.0;
    PParameters.AntiWindup := PIDAntiWindup#Combined;

    HistoricalParameters := PParameters;
    HistoricalParameters.ProportionalGain := LREAL#4.0;
    HistoricalParameters.IntegralGain := LREAL#0.8;
    CurrentParameters := PParameters;
    CurrentParameters.ProportionalGain := LREAL#3.260869565;
    CurrentParameters.IntegralGain := LREAL#0.652173913;
    CandidateParameters := PParameters;
    CandidateParameters.ProportionalGain := LREAL#5.769230769;
    CandidateParameters.IntegralGain := LREAL#1.153846154;

    PController.Configure(Parameters := PParameters);
    HistoricalController.Configure(Parameters := HistoricalParameters);
    CurrentController.Configure(Parameters := CurrentParameters);
    CandidateController.Configure(Parameters := CandidateParameters);

    PState.Setpoint := LREAL#35.0;
    PState.ProcessValue := PTemperature;
    PState.NominalOutput := LREAL#75.0;
    PState.SaturatedOutput := LREAL#75.0;
    HistoricalState := PState;
    HistoricalState.ProcessValue := HistoricalTemperature;
    CurrentState := PState;
    CurrentState.ProcessValue := CurrentTemperature;
    CandidateState := PState;
    CandidateState.ProcessValue := CandidateTemperature;
    PController.Initialize(State := PState);
    HistoricalController.Initialize(State := HistoricalState);
    CurrentController.Initialize(State := CurrentState);
    CandidateController.Initialize(State := CandidateState);
    Initialized := TRUE;
END_IF;

(* The comparison exports candidates but never mutates the confirmed revision. *)
ControllerRevision := DINT#77;

PInput.Setpoint := TemperatureSetpoint;
PInput.ProcessValue := PTemperature;
PInput.CycleTime := CycleTime;
HistoricalInput.Setpoint := TemperatureSetpoint;
HistoricalInput.ProcessValue := HistoricalTemperature;
HistoricalInput.CycleTime := CycleTime;
CurrentInput.Setpoint := TemperatureSetpoint;
CurrentInput.ProcessValue := CurrentTemperature;
CurrentInput.CycleTime := CycleTime;
CandidateInput.Setpoint := TemperatureSetpoint;
CandidateInput.ProcessValue := CandidateTemperature;
CandidateInput.CycleTime := CycleTime;

IF ManualMode OR NOT TemperatureQualityGood THEN
    (* Manual and invalid-quality operation use the same tracked safe power, so
       automatic recovery starts from the applied operating point. *)
    PInput.Mode := PIDMode#Manual;
    HistoricalInput.Mode := PIDMode#Manual;
    CurrentInput.Mode := PIDMode#Manual;
    CandidateInput.Mode := PIDMode#Manual;
    PInput.ManualOutput := ManualOutput;
    HistoricalInput.ManualOutput := ManualOutput;
    CurrentInput.ManualOutput := ManualOutput;
    CandidateInput.ManualOutput := ManualOutput;
ELSE
    PInput.Mode := PIDMode#Auto;
    HistoricalInput.Mode := PIDMode#Auto;
    CurrentInput.Mode := PIDMode#Auto;
    CandidateInput.Mode := PIDMode#Auto;
END_IF;

PRequested := PController.Update(Input := PInput);
HistoricalRequested := HistoricalController.Update(Input := HistoricalInput);
CurrentRequested := CurrentController.Update(Input := CurrentInput);
CandidateRequested := CandidateController.Update(Input := CandidateInput);

(* Applied power remains a separate actuator-facing signal even though this
   capstone's ideal power stage has only the declared amplitude limits. *)
PApplied := LIMIT(LREAL#0.0, PRequested, LREAL#100.0);
HistoricalApplied := LIMIT(LREAL#0.0, HistoricalRequested, LREAL#100.0);
CurrentApplied := LIMIT(LREAL#0.0, CurrentRequested, LREAL#100.0);
CandidateApplied := LIMIT(LREAL#0.0, CandidateRequested, LREAL#100.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.ManualMode AT %MX96.1 : BOOL;
    P1.TemperatureQualityGood AT %MX96.2 : BOOL;
    P1.TemperatureSetpoint AT %ML0 : LREAL;
    P1.ManualOutput AT %ML8 : LREAL;
    P1.CycleTime AT %ML16 : LREAL;
    P1.ControllerRevision AT %MD24 : DINT;
    P1.PRequested AT %ML32 : LREAL;
    P1.HistoricalRequested AT %ML40 : LREAL;
    P1.CurrentRequested AT %ML48 : LREAL;
    P1.CandidateRequested AT %ML56 : LREAL;
    P1.PApplied AT %QL0 : LREAL;
    P1.HistoricalApplied AT %QL8 : LREAL;
    P1.CurrentApplied AT %QL16 : LREAL;
    P1.CandidateApplied AT %QL24 : LREAL;
    P1.PTemperature AT %IL0 : LREAL;
    P1.HistoricalTemperature AT %IL8 : LREAL;
    P1.CurrentTemperature AT %IL16 : LREAL;
    P1.CandidateTemperature AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
