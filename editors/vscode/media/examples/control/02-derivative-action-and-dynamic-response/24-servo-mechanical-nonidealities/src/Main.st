USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Initialized : BOOL := FALSE;
    AutoMode : BOOL := TRUE;
    ProfileEnabled : BOOL := TRUE;
    TargetPosition : LREAL := 0.0;
    LoadPosition : LREAL := 0.0;
    MotorVelocity : LREAL := 0.0;
    ProfiledPosition : LREAL := 0.0;
    ProfiledVelocity : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    PositionRemaining : LREAL := 0.0;
    DesiredVelocity : LREAL := 0.0;
    StoppingVelocity : LREAL := 0.0;
    VelocityChange : LREAL := 0.0;
    PositionChange : LREAL := 0.0;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#8.0;
    Parameters.IntegralGain := LREAL#2.0;
    Parameters.DerivativeGain := LREAL#3.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    (* Wide controller limits retain the requested torque. The simulated drive
       applies its own physical torque and torque-slew limits afterward. *)
    Parameters.OutputMinimum := LREAL#-50.0;
    Parameters.OutputMaximum := LREAL#50.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#0.2;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    Controller.Configure(Parameters := Parameters);
    InitialState.Setpoint := ProfiledPosition;
    InitialState.ProcessValue := LoadPosition;
    Controller.Initialize(State := InitialState);
    Initialized := TRUE;
END_IF;

IF ProfileEnabled THEN
    (* This profile limits the reference to 0.5 rad/s and 1.0 rad/s2. These are
       trajectory limits; they are intentionally separate from torque limits. *)
    PositionRemaining := TargetPosition - ProfiledPosition;
    (* The stopping-speed bound starts deceleration early enough to reach the
       target without exceeding the acceleration limit. *)
    StoppingVelocity := SQRT(LREAL#2.0 * LREAL#1.0 * ABS(PositionRemaining));
    IF PositionRemaining >= LREAL#0.0 THEN
        DesiredVelocity := MIN(StoppingVelocity, LREAL#0.5);
    ELSE
        DesiredVelocity := MAX(-StoppingVelocity, LREAL#-0.5);
    END_IF;
    VelocityChange := DesiredVelocity - ProfiledVelocity;
    VelocityChange := MAX(MIN(VelocityChange, LREAL#0.02), LREAL#-0.02);
    ProfiledVelocity := ProfiledVelocity + VelocityChange;
    PositionChange := ProfiledVelocity * LREAL#0.02;
    ProfiledPosition := ProfiledPosition + PositionChange;
ELSE
    (* The unprofiled reference exists only to isolate what trajectory shaping
       changes in the controlled experiment. *)
    ProfiledPosition := TargetPosition;
    ProfiledVelocity := LREAL#0.0;
END_IF;

Input.Setpoint := ProfiledPosition;
Input.ProcessValue := LoadPosition;
IF AutoMode THEN Input.Mode := PIDMode#Auto; ELSE Input.Mode := PIDMode#Manual; END_IF;
Input.CycleTime := LREAL#0.02;
RequestedTorque := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.ProfileEnabled AT %MX16.1 : BOOL;
    P1.TargetPosition AT %ML40 : LREAL;
    P1.LoadPosition AT %IL0 : LREAL;
    P1.MotorVelocity AT %IL8 : LREAL;
    P1.ProfiledPosition AT %ML48 : LREAL;
    P1.ProfiledVelocity AT %ML56 : LREAL;
    P1.RequestedTorque AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
