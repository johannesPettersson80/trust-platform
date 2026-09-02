USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    FirstFilter : Control.LowPass;
    SecondFilter : Control.LowPass;
    FirstParameters : Control.LowPassParameters;
    SecondParameters : Control.LowPassParameters;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := TRUE;
    FilterSelection : INT := 0;
    FilterTimeConstant : LREAL := 0.05;
    Setpoint : LREAL := 0.0;
    RawPosition : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    ControllerPosition : LREAL := 0.0;
    MotorTorque : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    (* Both filter orders use the selected time constant. This lets the lesson
       compare cutoff first, then compare order at one fixed cutoff. *)
    FirstParameters.TimeConstant := FilterTimeConstant;
    FirstParameters.Order := FilterOrder#FirstOrder;
    FirstFilter.Configure(Parameters := FirstParameters);
    FirstFilter.Initialize(Input := RawPosition);
    SecondParameters.TimeConstant := FilterTimeConstant;
    SecondParameters.Order := FilterOrder#SecondOrder;
    SecondFilter.Configure(Parameters := SecondParameters);
    SecondFilter.Initialize(Input := RawPosition);

    Parameters.ProportionalGain := LREAL#8.0;
    Parameters.IntegralGain := LREAL#2.0;
    Parameters.DerivativeGain := LREAL#3.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#-10.0;
    Parameters.OutputMaximum := LREAL#10.0;
    Parameters.RateMinimum := LREAL#-500.0;
    Parameters.RateMaximum := LREAL#500.0;
    Parameters.TrackingTime := LREAL#0.2;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := RawPosition;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

IF FilterSelection = 1 THEN
    ControllerPosition := FirstFilter.Update(Input := RawPosition, CycleTime := LREAL#0.02);
ELSIF FilterSelection = 2 THEN
    ControllerPosition := SecondFilter.Update(Input := RawPosition, CycleTime := LREAL#0.02);
ELSE
    ControllerPosition := RawPosition;
END_IF;

Input.Setpoint := Setpoint;
(* The raw encoder remains visible; only this explicit controller-facing copy
   is filtered, preventing a smooth plot from hiding sensor evidence. *)
Input.ProcessValue := ControllerPosition;
IF AutoMode THEN Input.Mode := PIDMode#Auto; ELSE Input.Mode := PIDMode#Manual; END_IF;
Input.CycleTime := LREAL#0.02;
MotorTorque := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.FilterSelection AT %MW32 : INT;
    P1.Setpoint AT %ML40 : LREAL;
    P1.FilterTimeConstant AT %ML48 : LREAL;
    P1.RawPosition AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.ControllerPosition AT %ML0 : LREAL;
    P1.MotorTorque AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
