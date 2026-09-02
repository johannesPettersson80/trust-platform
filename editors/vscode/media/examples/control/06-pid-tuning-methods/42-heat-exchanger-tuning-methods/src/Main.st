USING Control;

PROGRAM Main
VAR
    ManualPiController : Control.PID;
    HistoricalPiController : Control.PID;
    ManualPiParameters : Control.PIDParameters;
    HistoricalPiParameters : Control.PIDParameters;
    ManualPiState : Control.PIDInitialState;
    HistoricalPiState : Control.PIDInitialState;
    ManualPiInput : Control.PIDInput;
    HistoricalPiInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    Setpoint : LREAL := 50.0;
    HistoricalKp : LREAL := 10.0057565917;
    HistoricalKi : LREAL := 1.0579328485;
    ManualPRequested : LREAL := 20.0;
    CriticalPRequested : LREAL := 20.0;
    ManualPApplied : LREAL := 20.0;
    ManualPiApplied : LREAL := 20.0;
    CriticalPApplied : LREAL := 20.0;
    HistoricalPiApplied : LREAL := 20.0;
    ManualPTemperature : LREAL := 50.0;
    ManualPiTemperature : LREAL := 50.0;
    CriticalPTemperature : LREAL := 50.0;
    HistoricalPiTemperature : LREAL := 50.0;
    AutomaticMode : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    (* The mapped setpoint starts at zero in the process image. Establish the
       declared 50 degC comparison point once before Control Studio can step it. *)
    Setpoint := LREAL#50.0;
    HistoricalKp := LREAL#10.0057565917;
    HistoricalKi := LREAL#1.0579328485;
    ManualPiParameters.ProportionalGain := LREAL#2.0;
    ManualPiParameters.IntegralGain := LREAL#0.10;
    ManualPiParameters.DerivativeGain := LREAL#0.0;
    ManualPiParameters.ProportionalSetpointWeight := LREAL#1.0;
    ManualPiParameters.DerivativeSetpointWeight := LREAL#0.0;
    ManualPiParameters.Bias := LREAL#20.0;
    ManualPiParameters.OutputMinimum := LREAL#0.0;
    ManualPiParameters.OutputMaximum := LREAL#100.0;
    ManualPiParameters.RateMinimum := LREAL#-1000.0;
    ManualPiParameters.RateMaximum := LREAL#1000.0;
    ManualPiParameters.TrackingTime := LREAL#1.0;
    ManualPiParameters.AntiWindup := PIDAntiWindup#Combined;
    ManualPiController.Configure(Parameters := ManualPiParameters);

    HistoricalPiParameters.ProportionalGain := HistoricalKp;
    HistoricalPiParameters.IntegralGain := HistoricalKi;
    HistoricalPiParameters.DerivativeGain := LREAL#0.0;
    HistoricalPiParameters.ProportionalSetpointWeight := LREAL#1.0;
    HistoricalPiParameters.DerivativeSetpointWeight := LREAL#0.0;
    HistoricalPiParameters.Bias := LREAL#20.0;
    HistoricalPiParameters.OutputMinimum := LREAL#0.0;
    HistoricalPiParameters.OutputMaximum := LREAL#100.0;
    HistoricalPiParameters.RateMinimum := LREAL#-1000.0;
    HistoricalPiParameters.RateMaximum := LREAL#1000.0;
    HistoricalPiParameters.TrackingTime := LREAL#1.0;
    HistoricalPiParameters.AntiWindup := PIDAntiWindup#Combined;
    HistoricalPiController.Configure(Parameters := HistoricalPiParameters);

    (* Initialize both controllers at the physical 20-percent equilibrium so
       the comparison does not include an artificial startup output step. *)
    ManualPiState.Setpoint := LREAL#50.0;
    ManualPiState.ProcessValue := ManualPiTemperature;
    ManualPiState.NominalOutput := LREAL#20.0;
    ManualPiState.SaturatedOutput := LREAL#20.0;
    ManualPiController.Initialize(State := ManualPiState);
    HistoricalPiState.Setpoint := LREAL#50.0;
    HistoricalPiState.ProcessValue := HistoricalPiTemperature;
    HistoricalPiState.NominalOutput := LREAL#20.0;
    HistoricalPiState.SaturatedOutput := LREAL#20.0;
    HistoricalPiController.Initialize(State := HistoricalPiState);
    Initialized := TRUE;
END_IF;

(* The two proportional-only paths make the offset and critical experiment
   visible without hiding either law behind another controller instance. *)
ManualPRequested := LREAL#20.0 + LREAL#2.0 * (Setpoint - ManualPTemperature);
CriticalPRequested := LREAL#20.0
    + LREAL#22.2350146482 * (Setpoint - CriticalPTemperature);
ManualPApplied := MAX(MIN(ManualPRequested, LREAL#100.0), LREAL#0.0);
CriticalPApplied := MAX(MIN(CriticalPRequested, LREAL#100.0), LREAL#0.0);

ManualPiInput.Setpoint := Setpoint;
ManualPiInput.ProcessValue := ManualPiTemperature;
ManualPiInput.Mode := PIDMode#Auto;
ManualPiInput.CycleTime := LREAL#0.1;
ManualPiApplied := ManualPiController.Update(Input := ManualPiInput);

HistoricalPiInput.Setpoint := Setpoint;
HistoricalPiInput.ProcessValue := HistoricalPiTemperature;
HistoricalPiInput.Mode := PIDMode#Auto;
HistoricalPiInput.CycleTime := LREAL#0.1;
HistoricalPiApplied := HistoricalPiController.Update(Input := HistoricalPiInput);
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML24 : LREAL;
    P1.HistoricalKp AT %ML32 : LREAL;
    P1.HistoricalKi AT %ML40 : LREAL;
    P1.ManualPRequested AT %ML48 : LREAL;
    P1.CriticalPRequested AT %ML56 : LREAL;
    P1.AutomaticMode AT %MX64.0 : BOOL;
    P1.ManualPApplied AT %QL0 : LREAL;
    P1.ManualPiApplied AT %QL8 : LREAL;
    P1.CriticalPApplied AT %QL16 : LREAL;
    P1.HistoricalPiApplied AT %QL24 : LREAL;
    P1.ManualPTemperature AT %IL0 : LREAL;
    P1.ManualPiTemperature AT %IL8 : LREAL;
    P1.CriticalPTemperature AT %IL16 : LREAL;
    P1.HistoricalPiTemperature AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
