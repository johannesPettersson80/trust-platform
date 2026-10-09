(* Variable-speed motors (specification trust-plant-library §6.2): run control from FB_RunDevice
   plus a speed reference for the drive. A drive starts at 0 %: the operator's speed is 0 % after
   a restart and after every stop. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK FB_VariableSpeedMotor EXTENDS FB_RunDevice IMPLEMENTS ISpeedDevice
VAR_INPUT
    (* The program's speed in %, followed in Auto while running. *)
    AutoSpeed : LREAL;
    SpeedFeedback : LREAL;
    HasSpeedFeedback : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* The drive's speed reference in %. *)
    SpeedSetpoint : LREAL;
    Speed : LREAL;
    (* The operator's speed in %. *)
    ManualSpeed : LREAL;
    SpeedDeviating : BOOL;
END_VAR
VAR PUBLIC RETAIN
    MinSpeed : LREAL := 0.0;
    MaxSpeed : LREAL := 100.0;
    SpeedDeviation : LREAL := 5.0;
    DeviationTime : TIME := T#10s;
END_VAR
VAR NON_RETAIN
    WasCommanded : BOOL;
    SpeedDeviationAlarm : Alarm.DigitalAlarm;
END_VAR
    ExecuteRun();
    ExecuteSpeed();

METHOD PROTECTED ExecuteSpeed
VAR
    Low : LREAL;
    High : LREAL;
    Wanted : LREAL;
END_VAR
    Low := MIN(MinSpeed, MaxSpeed);
    High := MAX(MinSpeed, MaxSpeed);
    IF RunCommand THEN
        IF Mode = E_Mode#Auto THEN
            Wanted := AutoSpeed;
            IF NOT F_Finite(Wanted) THEN
                Wanted := SpeedSetpoint;
            END_IF;
            (* Bumpless: Manual continues at the speed Auto had. *)
            ManualSpeed := LIMIT(Low, Wanted, High);
        ELSE
            Wanted := ManualSpeed;
        END_IF;
        SpeedSetpoint := LIMIT(Low, Wanted, High);
    ELSE
        SpeedSetpoint := 0.0;
        IF WasCommanded THEN
            ManualSpeed := 0.0;
        END_IF;
    END_IF;
    WasCommanded := RunCommand;
    IF HasSpeedFeedback THEN
        Speed := SpeedFeedback;
    ELSE
        Speed := SpeedSetpoint;
    END_IF;
    SpeedDeviating := HasSpeedFeedback AND RunCommand
        AND (NOT F_Finite(SpeedFeedback) OR ABS(SpeedFeedback - SpeedSetpoint) > SpeedDeviation);
    SpeedDeviationAlarm(Condition := SpeedDeviating, OnDelay := DeviationTime,
        Suppressed := Mode = E_Mode#OutOfService, Message := 'Speed deviation', Priority := AlarmPriority#Low);
END_METHOD

(* Accepted in Manual, also while stopped, so a speed can be chosen before the start. *)
METHOD PUBLIC SetSpeed : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
    SetSpeed := AcceptsCommands() AND F_Finite(Value);
    IF SetSpeed THEN
        ManualSpeed := LIMIT(MIN(MinSpeed, MaxSpeed), Value, MAX(MinSpeed, MaxSpeed));
    END_IF;
END_METHOD

METHOD PUBLIC OVERRIDE GetAlarmCount : INT
    GetAlarmCount := 4;
END_METHOD

METHOD PUBLIC OVERRIDE GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    IF Index = 4 THEN
        GetAlarm := REF(SpeedDeviationAlarm);
    ELSE
        GetAlarm := SUPER.GetAlarm(Index);
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_VariableSpeedPump EXTENDS FB_VariableSpeedMotor
    ExecuteRun();
    ExecuteSpeed();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_VariableSpeedFan EXTENDS FB_VariableSpeedMotor
    ExecuteRun();
    ExecuteSpeed();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_DosingPump EXTENDS FB_VariableSpeedMotor
    ExecuteRun();
    ExecuteSpeed();
END_FUNCTION_BLOCK

END_NAMESPACE
