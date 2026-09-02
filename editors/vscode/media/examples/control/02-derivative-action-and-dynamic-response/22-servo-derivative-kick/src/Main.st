USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := TRUE;
    DerivativeSetpointWeight : LREAL := 0.0;
    Setpoint : LREAL := 0.0;
    Position : LREAL := 0.0;
    Velocity : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#8.0;
    Parameters.IntegralGain := LREAL#2.0;
    Parameters.DerivativeGain := LREAL#3.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    (* c=1 differentiates the full setpoint step; c=0 differentiates only PV.
       The intermediate case makes the kick trade-off visible. *)
    Parameters.DerivativeSetpointWeight := DerivativeSetpointWeight;
    Parameters.Bias := LREAL#0.0;
    (* Wide controller limits preserve the requested kick as evidence. The real
       motor torque is clamped separately below and plotted alongside it. *)
    Parameters.OutputMinimum := LREAL#-100.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-10000.0;
    Parameters.RateMaximum := LREAL#10000.0;
    Parameters.TrackingTime := LREAL#0.2;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := Position;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := Position;
IF AutoMode THEN Input.Mode := PIDMode#Auto; ELSE Input.Mode := PIDMode#Manual; END_IF;
Input.CycleTime := LREAL#0.02;
RequestedTorque := Controller.Update(Input := Input);
AppliedTorque := MAX(MIN(RequestedTorque, LREAL#10.0), LREAL#-10.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX32.0 : BOOL;
    P1.DerivativeSetpointWeight AT %ML16 : LREAL;
    P1.Setpoint AT %ML24 : LREAL;
    P1.Position AT %IL0 : LREAL;
    P1.Velocity AT %IL8 : LREAL;
    P1.RequestedTorque AT %ML0 : LREAL;
    P1.AppliedTorque AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
