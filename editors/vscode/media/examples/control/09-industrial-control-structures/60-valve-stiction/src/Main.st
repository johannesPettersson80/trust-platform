USING Control;

PROGRAM Main
VAR
    HealthyController : Control.PID;
    DeadbandController : Control.PID;
    StictionController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    HealthyInput : Control.PIDInput;
    DeadbandInput : Control.PIDInput;
    StictionInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := FALSE;
    ManualDemand : LREAL := 80.0;
    FlowSetpoint : LREAL := 64.0;
    HealthyFlow : LREAL := 64.0;
    DeadbandFlow : LREAL := 64.0;
    StictionFlow : LREAL := 64.0;
    HealthyRequest : LREAL := 80.0;
    DeadbandRequest : LREAL := 80.0;
    StictionRequest : LREAL := 80.0;
END_VAR

IF NOT Initialized THEN
    ManualDemand := LREAL#80.0;
    (* One parameter record keeps tuning identical while only valve condition changes. *)
    Parameters.ProportionalGain := LREAL#0.5;
    Parameters.IntegralGain := LREAL#0.25;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#80.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#0.5;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    HealthyController.Configure(Parameters := Parameters);
    DeadbandController.Configure(Parameters := Parameters);
    StictionController.Configure(Parameters := Parameters);

    (* Initialize at the measured operating point for bumpless Manual or Auto entry. *)
    InitialState.Setpoint := FlowSetpoint;
    InitialState.ProcessValue := HealthyFlow;
    InitialState.NominalOutput := LREAL#80.0;
    InitialState.SaturatedOutput := LREAL#80.0;
    HealthyController.Initialize(State := InitialState);
    InitialState.ProcessValue := DeadbandFlow;
    DeadbandController.Initialize(State := InitialState);
    InitialState.ProcessValue := StictionFlow;
    StictionController.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

HealthyInput.Setpoint := FlowSetpoint;
HealthyInput.ProcessValue := HealthyFlow;
HealthyInput.CycleTime := LREAL#0.1;
DeadbandInput.Setpoint := FlowSetpoint;
DeadbandInput.ProcessValue := DeadbandFlow;
DeadbandInput.CycleTime := LREAL#0.1;
StictionInput.Setpoint := FlowSetpoint;
StictionInput.ProcessValue := StictionFlow;
StictionInput.CycleTime := LREAL#0.1;

IF AutoMode THEN
    HealthyInput.Mode := PIDMode#Auto;
    DeadbandInput.Mode := PIDMode#Auto;
    StictionInput.Mode := PIDMode#Auto;
ELSE
    (* The common manual demand makes characterization and reversal probes equivalent. *)
    HealthyInput.Mode := PIDMode#Manual;
    DeadbandInput.Mode := PIDMode#Manual;
    StictionInput.Mode := PIDMode#Manual;
    HealthyInput.ManualOutput := ManualDemand;
    DeadbandInput.ManualOutput := ManualDemand;
    StictionInput.ManualOutput := ManualDemand;
END_IF;

HealthyRequest := HealthyController.Update(Input := HealthyInput);
DeadbandRequest := DeadbandController.Update(Input := DeadbandInput);
StictionRequest := StictionController.Update(Input := StictionInput);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX91.0 : BOOL;
    P1.ManualDemand AT %ML328 : LREAL;
    P1.FlowSetpoint AT %ML336 : LREAL;
    P1.HealthyFlow AT %IL168 : LREAL;
    P1.DeadbandFlow AT %IL176 : LREAL;
    P1.StictionFlow AT %IL184 : LREAL;
    P1.HealthyRequest AT %QL184 : LREAL;
    P1.DeadbandRequest AT %QL192 : LREAL;
    P1.StictionRequest AT %QL200 : LREAL;
END_VAR
END_CONFIGURATION
