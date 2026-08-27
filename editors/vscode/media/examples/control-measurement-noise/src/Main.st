USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Filter : Control.LowPass;
    FilterParameters : Control.LowPassParameters;
    FilteredValue : LREAL;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := TRUE;
    Setpoint : LREAL := 0.0;
    ProcessValue : LREAL := 0.0;
    ManipulatedValue : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#0.667;
    Parameters.IntegralGain := LREAL#0.667;
    Parameters.DerivativeGain := LREAL#0;
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
    FilterParameters.TimeConstant := LREAL#0.1;
    FilterParameters.Order := FilterOrder#SecondOrder;
    Filter.Configure(Parameters := FilterParameters);
    Filter.Initialize(Input := ProcessValue);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
FilteredValue := Filter.Update(Input := ProcessValue, CycleTime := LREAL#0.1);
Input.ProcessValue := FilteredValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
END_IF;
Input.CycleTime := LREAL#0.1;
ManipulatedValue := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
