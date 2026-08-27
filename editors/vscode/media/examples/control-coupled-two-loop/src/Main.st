USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Controller2 : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input1 : Control.PIDInput;
    Input2 : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode1 : BOOL := TRUE;
    AutoMode2 : BOOL := TRUE;
    Setpoint1 : LREAL := 0.0;
    Setpoint2 : LREAL := 0.0;
    ProcessValue : LREAL := 0.0;
    ProcessValue2 : LREAL := 0.0;
    ManipulatedValue : LREAL := 0.0;
    ManipulatedValue2 : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#0.8;
    Parameters.IntegralGain := LREAL#0.4;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-5.0;
    Parameters.OutputMaximum := LREAL#5.0;
    Parameters.RateMinimum := LREAL#-100.0;
    Parameters.RateMaximum := LREAL#100.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    Controller2.Configure(Parameters := Parameters);
    Controller.Initialize(State := State);
    Controller2.Initialize(State := State);
    Initialized := TRUE;
END_IF;

Input1.Setpoint := Setpoint1;
Input1.ProcessValue := ProcessValue;
IF AutoMode1 THEN
    Input1.Mode := PIDMode#Auto;
ELSE
    Input1.Mode := PIDMode#Manual;
END_IF;
Input1.CycleTime := LREAL#0.05;
Input2.Setpoint := Setpoint2;
Input2.ProcessValue := ProcessValue2;
IF AutoMode2 THEN
    Input2.Mode := PIDMode#Auto;
ELSE
    Input2.Mode := PIDMode#Manual;
END_IF;
Input2.CycleTime := LREAL#0.05;
ManipulatedValue := Controller.Update(Input := Input1);
ManipulatedValue2 := Controller2.Update(Input := Input2);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode1 AT %MX16.0 : BOOL;
    P1.AutoMode2 AT %MX16.1 : BOOL;
    P1.Setpoint1 AT %ML0 : LREAL;
    P1.Setpoint2 AT %ML8 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ManipulatedValue2 AT %QL8 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.ProcessValue2 AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
