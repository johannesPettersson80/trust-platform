FUNCTION_BLOCK PumpController
VAR_INPUT
    Command : PumpCommand;
END_VAR
VAR_OUTPUT
    Status : PumpStatus;
END_VAR
VAR
    RampTimer : TON;
    Ramp : REAL;
END_VAR
VAR CONSTANT
    RampTime : TIME := T#1s;
END_VAR

Status.Running := FALSE;
Status.Alarm := 0;

IF NOT Command.Enable THEN
    Status.State := PumpState#Idle;
    Status.ActualSpeed := 0.0;
    Ramp := 0.0;
    RampTimer(IN := FALSE);
ELSE
    CASE Status.State OF
        PumpState#Idle:
            Status.State := PumpState#Starting;
            RampTimer(IN := TRUE, PT := RampTime);
        PumpState#Starting:
            RampTimer(IN := TRUE, PT := RampTime);
            IF RampTimer.Q THEN
                Status.State := PumpState#Running;
            ELSE
                Ramp := Ramp + 0.2;
            END_IF
        PumpState#Running:
            Status.Running := TRUE;
            Ramp := Command.TargetSpeed;
        PumpState#Fault:
            Status.Running := FALSE;
            Status.Alarm := 16#BEEF;
    END_CASE
END_IF

Status.ActualSpeed := Ramp;
END_FUNCTION_BLOCK
