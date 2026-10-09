(* The truST alarm library (specification 40): ISA-18.2 and EEMUA 191 alarms as function blocks.
   Detection blocks extend the base `Alarm` and call `Evaluate` with their raw condition; a
   customer block does the same to get the whole lifecycle, the records and the display. *)
NAMESPACE Alarm

TYPE AlarmPriority :
(
    Low := 1,
    Medium := 2,
    High := 3
);
END_TYPE

TYPE AlarmState :
(
    Normal := 0,
    ActiveUnacknowledged := 1,
    ActiveAcknowledged := 2,
    ClearedUnacknowledged := 3
);
END_TYPE

TYPE LimitDirection :
(
    High := 0,
    Low := 1
);
END_TYPE

TYPE AlarmRequest :
STRUCT
    Acknowledge : BOOL;
    Shelve : TIME;
    Unshelve : BOOL;
    Reset : BOOL;
    Comment : STRING[120];
    RemoveFromService : BOOL;
    ReturnToService : BOOL;
END_STRUCT
END_TYPE

(* Marks the first alarm of a group to become active, until the group is reset (spec 40, 3.6).
   Each alarm that becomes active joins the group and gets a ticket; ticket 1 of the current
   epoch is first out. *)
FUNCTION_BLOCK FirstOutGroup
VAR_OUTPUT
    Epoch : UDINT;
END_VAR
VAR
    Joined : UDINT;
END_VAR
METHOD PUBLIC Reset
    Epoch := Epoch + 1;
    Joined := 0;
END_METHOD
METHOD PUBLIC Join : UDINT
    Joined := Joined + 1;
    Join := Joined;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Alarm
VAR_INPUT
    Message : STRING[120];
    Priority : AlarmPriority := AlarmPriority#Low;
    OnDelay : TIME := T#0s;
    OffDelay : TIME := T#0s;
    Latching : BOOL := FALSE;
    Suppressed : BOOL := FALSE;
    Group : REF_TO FirstOutGroup;
END_VAR
VAR_OUTPUT
    Active : BOOL;
    Unacknowledged : BOOL;
    State : AlarmState := AlarmState#Normal;
    Shelved : BOOL;
    OutOfService : BOOL;
    SuppressedByDesign : BOOL;
    FirstOut : BOOL;
    Annunciating : BOOL;
    RaiseCount : UDINT;
    AnnunciationCount : ULINT;
    AnnunciationExhausted : BOOL;
END_VAR
VAR
    (* Operator actions the runtime writes; applied at the next scan, then cleared (spec 40, 4). *)
    Request : AlarmRequest;
    Detected : BOOL;
    Latched : BOOL;
    WasActive : BOOL;
    WasHidden : BOOL;
    OnTimer : TON;
    OffTimer : TON;
    ShelveTimer : TON;
    ShelveDuration : TIME;
    Ticket : UDINT;
    TicketEpoch : UDINT;
END_VAR

(* One scan of the alarm with the detection block's raw condition (spec 40, 3). *)
METHOD PROTECTED Evaluate
VAR_INPUT
    Condition : BOOL;
END_VAR
VAR
    Hidden : BOOL;
END_VAR
    (* Operator actions waiting since the last scan. *)
    IF Request.Acknowledge THEN
        Acknowledge();
    END_IF;
    IF Request.Shelve > T#0s THEN
        ApplyShelving(Duration := Request.Shelve);
    END_IF;
    IF Request.Unshelve THEN Unshelve(); END_IF;
    IF Request.RemoveFromService THEN SetOutOfService(Value := TRUE); END_IF;
    IF Request.ReturnToService THEN SetOutOfService(Value := FALSE); END_IF;
    IF Request.Reset THEN
        Reset();
    END_IF;
    Request.Acknowledge := FALSE;
    Request.Shelve := T#0s;
    Request.Unshelve := FALSE;
    Request.Reset := FALSE;
    Request.Comment := '';
    Request.RemoveFromService := FALSE;
    Request.ReturnToService := FALSE;

    (* Shelving ends by itself. *)
    ShelveTimer(IN := Shelved, PT := ShelveDuration);
    IF ShelveTimer.Q THEN
        Shelved := FALSE;
        ShelveTimer(IN := FALSE, PT := ShelveDuration);
    END_IF;
    SuppressedByDesign := Suppressed;

    (* The delays: active after OnDelay, cleared after OffDelay. *)
    OnTimer(IN := Condition, PT := OnDelay);
    OffTimer(IN := NOT Condition, PT := OffDelay);
    IF OnTimer.Q AND NOT Detected THEN
        Detected := TRUE;
        RaiseCount := RaiseCount + 1;
        IF Latching THEN
            Latched := TRUE;
        END_IF;
    ELSIF OffTimer.Q THEN
        Detected := FALSE;
    END_IF;
    Active := Detected OR Latched;

    (* The ISA-18.2 states; a shelved, suppressed or out-of-service alarm does not annunciate and
       returns as a newly raised one. *)
    Hidden := Shelved OR OutOfService OR SuppressedByDesign;
    IF Hidden THEN
        State := AlarmState#Normal;
        Unacknowledged := FALSE;
    ELSIF (WasHidden OR NOT WasActive) AND Active THEN
        IF AnnunciationCount < (ULINT#9223372036854775807 * ULINT#2 + ULINT#1) THEN AnnunciationCount := AnnunciationCount + 1;
        ELSE AnnunciationExhausted := TRUE; END_IF;
        State := AlarmState#ActiveUnacknowledged;
        Unacknowledged := TRUE;
        IF Group <> NULL THEN
            Ticket := Group^.Join();
            TicketEpoch := Group^.Epoch;
        END_IF;
    ELSIF WasHidden AND NOT Active THEN
        State := AlarmState#Normal;
        Unacknowledged := FALSE;
    ELSIF WasActive AND NOT Active THEN
        IF Unacknowledged THEN
            State := AlarmState#ClearedUnacknowledged;
        ELSE
            State := AlarmState#Normal;
        END_IF;
    END_IF;
    WasActive := Active;
    WasHidden := Hidden;
    Annunciating := (Active OR Unacknowledged) AND NOT Hidden;
    IF Group <> NULL THEN
        FirstOut := Ticket = 1 AND TicketEpoch = Group^.Epoch;
    ELSE
        FirstOut := FALSE;
    END_IF;
END_METHOD

(* The same operations are available to typed PLC managers and to the existing Request
   fields. Detection and timer ownership stay in this alarm FB. *)
METHOD PUBLIC ApplyShelving
VAR_INPUT Duration : TIME; END_VAR
    IF Duration <= T#0s THEN RETURN; END_IF;
    Shelved := TRUE;
    ShelveDuration := LIMIT(T#1s, Duration, T#24h);
    ShelveTimer(IN := FALSE, PT := ShelveDuration);
END_METHOD
METHOD PUBLIC Unshelve
    Shelved := FALSE;
END_METHOD
METHOD PUBLIC SetOutOfService
VAR_INPUT Value : BOOL; END_VAR
    OutOfService := Value;
END_METHOD

(* Acknowledges the alarm, as an operator's acknowledgement does (spec 40, 3.1). *)
METHOD PUBLIC Acknowledge
    IF State = AlarmState#ActiveUnacknowledged THEN
        State := AlarmState#ActiveAcknowledged;
        Unacknowledged := FALSE;
    ELSIF State = AlarmState#ClearedUnacknowledged THEN
        State := AlarmState#Normal;
        Unacknowledged := FALSE;
    END_IF;
END_METHOD

(* Resets a latched alarm whose condition is gone (spec 40, 3.5). *)
METHOD PUBLIC Reset
    IF Latched AND NOT Detected THEN
        Latched := FALSE;
    END_IF;
END_METHOD
METHOD PUBLIC IsDetected : BOOL
    IsDetected := Detected;
END_METHOD
END_FUNCTION_BLOCK

(* Active while its condition is TRUE (spec 40, 3.7). *)
FUNCTION_BLOCK DigitalAlarm EXTENDS Alarm
VAR_INPUT
    Condition : BOOL;
END_VAR
    Evaluate(Condition := Condition);
END_FUNCTION_BLOCK

FUNCTION_BLOCK LimitAlarm EXTENDS Alarm
VAR_INPUT
    Value : LREAL;
    Limit : LREAL;
    Deadband : LREAL := 0.0;
    Direction : LimitDirection := LimitDirection#High;
END_VAR
VAR
    Beyond : BOOL;
END_VAR
    (* A value that is not finite is beyond the limit: a broken measurement is not hidden. *)
    IF Value <> Value OR ABS(Value) > LREAL#1.7976931348623157E308 THEN
        Beyond := TRUE;
    ELSIF Direction = LimitDirection#High THEN
        IF Value > Limit THEN
            Beyond := TRUE;
        ELSIF Value < Limit - Deadband THEN
            Beyond := FALSE;
        END_IF;
    ELSE
        IF Value < Limit THEN
            Beyond := TRUE;
        ELSIF Value > Limit + Deadband THEN
            Beyond := FALSE;
        END_IF;
    END_IF;
    Evaluate(Condition := Beyond);
END_FUNCTION_BLOCK

FUNCTION_BLOCK DeviationAlarm EXTENDS Alarm
VAR_INPUT
    Value : LREAL;
    Reference : LREAL;
    Limit : LREAL;
    Deadband : LREAL := 0.0;
END_VAR
VAR
    Apart : BOOL;
    Difference : LREAL;
END_VAR
    Difference := ABS(Value - Reference);
    IF Difference <> Difference OR Difference > LREAL#1.7976931348623157E308 THEN
        Apart := TRUE;
    ELSIF Difference > Limit THEN
        Apart := TRUE;
    ELSIF Difference < Limit - Deadband THEN
        Apart := FALSE;
    END_IF;
    Evaluate(Condition := Apart);
END_FUNCTION_BLOCK

FUNCTION_BLOCK DiscrepancyAlarm EXTENDS Alarm
VAR_INPUT
    Command : BOOL;
    Feedback : BOOL;
END_VAR
    (* OnDelay is the time the equipment may take to follow its command. *)
    Evaluate(Condition := Command <> Feedback);
END_FUNCTION_BLOCK

END_NAMESPACE
