(* On/off valves and dampers (specification trust-plant-library §6.6). The safe state is the fail
   position: the de-energised solenoid, closed or, with FailOpen, open. Moving away from the fail
   position counts as a start: it needs the permissive and is refused while blocked. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK FB_OnOffValve EXTENDS FB_Device IMPLEMENTS IOpenCloseDevice, IAlarmSource
VAR_INPUT
    (* The program's position, followed in Auto: TRUE open. *)
    AutoOpen : BOOL;
    OpenFeedback : BOOL;
    ClosedFeedback : BOOL;
    HasOpenFeedback : BOOL := TRUE;
    HasClosedFeedback : BOOL := TRUE;
    (* The valve opens when its solenoid loses power. *)
    FailOpen : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* The commanded position: TRUE open. *)
    OpenCommand : BOOL;
    (* The solenoid output: energised to leave the fail position. *)
    Solenoid : BOOL;
    Opened : BOOL;
    Closed : BOOL;
    Moving : BOOL;
    FailedToOpen : BOOL;
    FailedToClose : BOOL;
    FeedbackConflict : BOOL;
    (* The operator may open, or close, now. *)
    OpenPermit : BOOL;
    ClosePermit : BOOL;
END_VAR
VAR_OUTPUT RETAIN
    Operations : UDINT;
END_VAR
VAR PUBLIC RETAIN
    OpenTimeout : TIME := T#10s;
    CloseTimeout : TIME := T#10s;
END_VAR
VAR NON_RETAIN
    ManualOpen : BOOL;
    Initialized : BOOL;
    Watch : FB_EndPositionWatch;
    FailedToOpenAlarm : Alarm.DigitalAlarm;
    FailedToCloseAlarm : Alarm.DigitalAlarm;
    ConflictAlarm : Alarm.DigitalAlarm;
END_VAR
    ExecuteValve();

METHOD PROTECTED ExecuteValve
VAR
    Wanted : BOOL;
    Supervised : BOOL;
END_VAR
    (* After a restart a manually commanded valve is in its fail position (§4.8). *)
    IF NOT Initialized THEN
        ManualOpen := FailOpen;
        OpenCommand := FailOpen;
        Initialized := TRUE;
    END_IF;
    Supervised := Mode <> E_Mode#OutOfService;
    CASE Mode OF
        E_Mode#Auto:
            Wanted := AutoOpen;
        E_Mode#Manual:
            Wanted := ManualOpen;
    ELSE
        Wanted := FailOpen;
    END_CASE;
    IF Wanted <> FailOpen AND OpenCommand = FailOpen AND NOT MayStart() THEN
        Wanted := FailOpen;
    END_IF;
    IF HeldSafe() THEN
        Wanted := FailOpen;
    END_IF;
    OpenCommand := Wanted;

    Watch(Enable := Supervised, TowardsA := OpenCommand, AtA := OpenFeedback, AtB := ClosedFeedback,
        HasA := HasOpenFeedback, HasB := HasClosedFeedback, TimeoutA := OpenTimeout,
        TimeoutB := CloseTimeout, Clear := TakeReset());
    FailedToOpen := Watch.FailedA;
    FailedToClose := Watch.FailedB;
    FeedbackConflict := Watch.Conflict;
    Fault := FailedToOpen OR FailedToClose OR FeedbackConflict;
    IF Fault THEN
        OpenCommand := FailOpen;
    END_IF;
    Solenoid := OpenCommand XOR FailOpen;
    Opened := Watch.InA;
    Closed := Watch.InB;
    Moving := Watch.Moving;
    IF FeedbackConflict THEN
        FaultText := 'Both end switches active: check them, then reset';
    ELSIF FailedToOpen THEN
        FaultText := 'Failed to open: reset to try again';
    ELSIF FailedToClose THEN
        FaultText := 'Failed to close: reset to try again';
    ELSE
        FaultText := '';
    END_IF;

    (* Bumpless (§3.4). *)
    ManualOpen := OpenCommand;

    FailedToOpenAlarm(Condition := FailedToOpen, Suppressed := NOT Supervised,
        Message := 'Failed to open', Priority := AlarmPriority#Medium);
    FailedToCloseAlarm(Condition := FailedToClose, Suppressed := NOT Supervised,
        Message := 'Failed to close', Priority := AlarmPriority#Medium);
    ConflictAlarm(Condition := FeedbackConflict, Suppressed := NOT Supervised,
        Message := 'End switches conflict', Priority := AlarmPriority#Medium);
    IF Supervised AND Watch.Arrived AND Operations < UDINT#4294967295 THEN
        Operations := Operations + 1;
    END_IF;
    Publish();
    OpenPermit := Manual AND (FailOpen OR NOT Blocked);
    ClosePermit := Manual AND (NOT FailOpen OR NOT Blocked);
END_METHOD

METHOD PUBLIC Open : BOOL
    IF FailOpen THEN
        Open := AcceptsCommands();
    ELSE
        Open := AcceptsStart();
    END_IF;
    IF Open THEN
        ManualOpen := TRUE;
    END_IF;
END_METHOD

METHOD PUBLIC Close : BOOL
    IF FailOpen THEN
        Close := AcceptsStart();
    ELSE
        Close := AcceptsCommands();
    END_IF;
    IF Close THEN
        ManualOpen := FALSE;
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
        1: GetAlarm := REF(FailedToOpenAlarm);
        2: GetAlarm := REF(FailedToCloseAlarm);
        3: GetAlarm := REF(ConflictAlarm);
    ELSE
        GetAlarm := NULL;
    END_CASE;
END_METHOD
END_FUNCTION_BLOCK

(* An on/off damper: the valve's behaviour with a damper's name and symbol. *)
FUNCTION_BLOCK FB_Damper EXTENDS FB_OnOffValve
    ExecuteValve();
END_FUNCTION_BLOCK

END_NAMESPACE
