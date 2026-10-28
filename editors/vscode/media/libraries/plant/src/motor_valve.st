(* A motor-operated valve (specification trust-plant-library §6.7). It has no spring: the motor
   drives it to an end limit and stops there, the operator may stop it in between, and its safe
   state stops it in place. Every movement counts as a start. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK FB_MotorValve EXTENDS FB_Device IMPLEMENTS IOpenCloseDevice, ITravelStop, IAlarmSource
VAR_INPUT
    (* The program's position, followed in Auto: TRUE open, FALSE closed. *)
    AutoOpen : BOOL;
    OpenLimit : BOOL;
    ClosedLimit : BOOL;
    (* The actuator's torque switch and the motor's overload relay; each latches. *)
    TorqueTrip : BOOL;
    Trip : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    OpenCommand : BOOL;
    CloseCommand : BOOL;
    Opened : BOOL;
    Closed : BOOL;
    Moving : BOOL;
    FailedToOpen : BOOL;
    FailedToClose : BOOL;
    FeedbackConflict : BOOL;
    Tripped : BOOL;
END_VAR
VAR_OUTPUT RETAIN
    Operations : UDINT;
END_VAR
VAR PUBLIC RETAIN
    TravelTimeout : TIME := T#60s;
END_VAR
VAR NON_RETAIN
    (* 0 stopped in place, 1 open, 2 closed: the target now and the operator's choice. *)
    Target : INT;
    ManualTarget : INT;
    WasOpened : BOOL;
    WasClosed : BOOL;
    Primed : BOOL;
    TravelWatch : TON;
    ConflictWatch : TON;
    TripAlarm : Alarm.DigitalAlarm;
    FailedToOpenAlarm : Alarm.DigitalAlarm;
    FailedToCloseAlarm : Alarm.DigitalAlarm;
END_VAR
    ExecuteTravel();

METHOD PROTECTED ExecuteTravel
VAR
    Wanted : INT;
    Supervised : BOOL;
    WasMoving : BOOL;
END_VAR
    Supervised := Mode <> E_Mode#OutOfService;
    IF TakeReset() THEN
        IF NOT Trip AND NOT TorqueTrip THEN
            Tripped := FALSE;
        END_IF;
        FailedToOpen := FALSE;
        FailedToClose := FALSE;
        FeedbackConflict := FALSE;
        TravelWatch(IN := FALSE);
        ConflictWatch(IN := FALSE);
    END_IF;
    IF Supervised AND (Trip OR TorqueTrip) THEN
        Tripped := TRUE;
    END_IF;
    Fault := Tripped OR FailedToOpen OR FailedToClose OR FeedbackConflict;

    CASE Mode OF
        E_Mode#Auto:
            IF AutoOpen THEN
                Wanted := 1;
            ELSE
                Wanted := 2;
            END_IF;
        E_Mode#Manual:
            Wanted := ManualTarget;
    ELSE
        Wanted := 0;
    END_CASE;
    WasMoving := OpenCommand OR CloseCommand;
    IF Wanted <> 0 AND Wanted <> Target AND NOT MayStart() THEN
        Wanted := 0;
    END_IF;
    IF HeldSafe() THEN
        Wanted := 0;
    END_IF;
    Target := Wanted;

    (* Drive towards the target until its limit, never both ways. *)
    OpenCommand := Target = 1 AND NOT OpenLimit;
    CloseCommand := Target = 2 AND NOT ClosedLimit;

    TravelWatch(IN := Supervised AND (OpenCommand OR CloseCommand), PT := TravelTimeout);
    IF TravelWatch.Q THEN
        IF OpenCommand THEN
            FailedToOpen := TRUE;
        ELSE
            FailedToClose := TRUE;
        END_IF;
    END_IF;
    ConflictWatch(IN := Supervised AND OpenLimit AND ClosedLimit, PT := T#1s);
    IF ConflictWatch.Q THEN
        FeedbackConflict := TRUE;
    END_IF;
    IF Tripped OR FailedToOpen OR FailedToClose OR FeedbackConflict THEN
        Fault := TRUE;
        Target := 0;
        OpenCommand := FALSE;
        CloseCommand := FALSE;
    END_IF;
    Opened := OpenLimit;
    Closed := ClosedLimit;
    Moving := OpenCommand OR CloseCommand;
    IF Tripped THEN
        FaultText := 'Motor or torque trip: reset after the cause is cleared';
    ELSIF FeedbackConflict THEN
        FaultText := 'Both limit switches active: check them, then reset';
    ELSIF FailedToOpen THEN
        FaultText := 'Did not open in time: reset to try again';
    ELSIF FailedToClose THEN
        FaultText := 'Did not close in time: reset to try again';
    ELSE
        FaultText := '';
    END_IF;

    (* Bumpless (§3.4); in Manual a refused or stopped movement leaves nothing pending. *)
    ManualTarget := Target;

    TripAlarm(Condition := Tripped, Suppressed := NOT Supervised,
        Message := 'Motor or torque trip', Priority := AlarmPriority#High);
    FailedToOpenAlarm(Condition := FailedToOpen, Suppressed := NOT Supervised,
        Message := 'Failed to open', Priority := AlarmPriority#Medium);
    FailedToCloseAlarm(Condition := FailedToClose OR FeedbackConflict, Suppressed := NOT Supervised,
        Message := 'Failed to close', Priority := AlarmPriority#Medium);
    IF Supervised AND Primed AND WasMoving
        AND ((OpenLimit AND NOT WasOpened) OR (ClosedLimit AND NOT WasClosed))
        AND Operations < UDINT#4294967295 THEN
        Operations := Operations + 1;
    END_IF;
    WasOpened := OpenLimit;
    WasClosed := ClosedLimit;
    Primed := TRUE;
    Publish();
END_METHOD

METHOD PUBLIC Open : BOOL
    Open := AcceptsStart();
    IF Open THEN
        ManualTarget := 1;
    END_IF;
END_METHOD

METHOD PUBLIC Close : BOOL
    Close := AcceptsStart();
    IF Close THEN
        ManualTarget := 2;
    END_IF;
END_METHOD

METHOD PUBLIC StopTravel : BOOL
    StopTravel := AcceptsCommands();
    IF StopTravel THEN
        ManualTarget := 0;
    END_IF;
END_METHOD

METHOD PUBLIC IsOpen : BOOL
    IsOpen := Opened;
END_METHOD

METHOD PUBLIC IsClosed : BOOL
    IsClosed := Closed;
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 3;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    CASE Index OF
        1: GetAlarm := REF(TripAlarm);
        2: GetAlarm := REF(FailedToOpenAlarm);
        3: GetAlarm := REF(FailedToCloseAlarm);
    ELSE
        GetAlarm := NULL;
    END_CASE;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
