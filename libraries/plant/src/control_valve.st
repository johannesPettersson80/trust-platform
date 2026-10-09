(* Control valves and modulating dampers (specification trust-plant-library §6.9). The safe state
   is FailPosition. Leaving it is a start: it needs the permissive; once released the valve keeps
   following its position when the permissive drops. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK FB_ControlValve EXTENDS FB_Device IMPLEMENTS IPositionDevice, IAlarmSource
VAR_INPUT
    (* The program's position in % open, followed in Auto. *)
    AutoPosition : LREAL;
    PositionFeedback : LREAL;
    HasFeedback : BOOL;
    (* The position the valve takes on loss of signal or air, in % open. *)
    FailPosition : LREAL;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* The positioner's setpoint in % open. *)
    PositionSetpoint : LREAL;
    Position : LREAL;
    InPosition : BOOL;
    (* The operator's position in % open. *)
    ManualPosition : LREAL;
    Deviating : BOOL;
END_VAR
VAR PUBLIC RETAIN
    MinPosition : LREAL := 0.0;
    MaxPosition : LREAL := 100.0;
    PositionDeviation : LREAL := 5.0;
    DeviationTime : TIME := T#10s;
END_VAR
VAR NON_RETAIN
    Released : BOOL;
    Initialized : BOOL;
    DeviationAlarm : Alarm.DigitalAlarm;
END_VAR
    ExecutePosition();

METHOD PROTECTED ExecutePosition
VAR
    Low : LREAL;
    High : LREAL;
    Wanted : LREAL;
    Supervised : BOOL;
END_VAR
    (* After a restart a manually positioned valve is at its fail position (§4.8). *)
    IF NOT Initialized THEN
        ManualPosition := FailPosition;
        PositionSetpoint := FailPosition;
        Initialized := TRUE;
    END_IF;
    (* A control valve latches no fault, so a Reset has nothing to clear. *)
    ResetRequested := FALSE;
    Supervised := Mode <> E_Mode#OutOfService;
    Low := MIN(MinPosition, MaxPosition);
    High := MAX(MinPosition, MaxPosition);
    Fault := FALSE;
    FaultText := '';
    IF HeldSafe() THEN
        Released := FALSE;
    ELSIF NOT Released AND MayStart() THEN
        Released := TRUE;
    END_IF;
    IF Mode = E_Mode#Auto THEN
        Wanted := AutoPosition;
        IF NOT F_Finite(Wanted) THEN
            Wanted := PositionSetpoint;
        END_IF;
    ELSE
        Wanted := ManualPosition;
    END_IF;
    IF Released THEN
        PositionSetpoint := LIMIT(Low, Wanted, High);
    ELSE
        PositionSetpoint := FailPosition;
    END_IF;
    IF HasFeedback THEN
        Position := PositionFeedback;
    ELSE
        Position := PositionSetpoint;
    END_IF;
    Deviating := HasFeedback AND Supervised
        AND (NOT F_Finite(PositionFeedback) OR ABS(PositionFeedback - PositionSetpoint) > PositionDeviation);
    InPosition := NOT Deviating;
    (* Bumpless (§3.4): Manual holds the valve where it is; the safe state leaves it at the fail
       position. *)
    ManualPosition := PositionSetpoint;
    DeviationAlarm(Condition := Deviating, OnDelay := DeviationTime, Suppressed := NOT Supervised,
        Message := 'Position deviation', Priority := AlarmPriority#Low);
    Publish();
END_METHOD

(* Accepted in Manual while nothing blocks the valve; clamped to MinPosition..MaxPosition. *)
METHOD PUBLIC SetPosition : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
    SetPosition := AcceptsStart() AND F_Finite(Value);
    IF SetPosition THEN
        ManualPosition := LIMIT(MIN(MinPosition, MaxPosition), Value, MAX(MinPosition, MaxPosition));
    END_IF;
END_METHOD

METHOD PUBLIC GetPosition : LREAL
    GetPosition := Position;
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 1;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    IF Index = 1 THEN
        GetAlarm := REF(DeviationAlarm);
    ELSE
        GetAlarm := NULL;
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_ModulatingDamper EXTENDS FB_ControlValve
    ExecutePosition();
END_FUNCTION_BLOCK

END_NAMESPACE
