(* A pneumatic or hydraulic cylinder (specification trust-plant-library §6.8). Single-acting: one
   solenoid extends it and the spring retracts it, so the safe state is retracted. Double-acting:
   one solenoid each way; the safe state de-energises both and it stays where it is. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK FB_Cylinder EXTENDS FB_Device IMPLEMENTS IExtendRetractDevice, IAlarmSource
VAR_INPUT
    (* The program's position, followed in Auto: TRUE extended. *)
    AutoExtend : BOOL;
    ExtendedSensor : BOOL;
    RetractedSensor : BOOL;
    HasExtendedSensor : BOOL := TRUE;
    HasRetractedSensor : BOOL := TRUE;
    DoubleActing : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* Solenoid A, extending. *)
    ExtendCommand : BOOL;
    (* Solenoid B, retracting; double-acting only. *)
    RetractCommand : BOOL;
    Extended : BOOL;
    Retracted : BOOL;
    Moving : BOOL;
    FailedToExtend : BOOL;
    FailedToRetract : BOOL;
    FeedbackConflict : BOOL;
    ExtendPermit : BOOL;
    RetractPermit : BOOL;
END_VAR
VAR_OUTPUT RETAIN
    Operations : UDINT;
END_VAR
VAR PUBLIC RETAIN
    ExtendTimeout : TIME := T#5s;
    RetractTimeout : TIME := T#5s;
END_VAR
VAR NON_RETAIN
    (* The commanded position: TRUE extended. *)
    Wanted : BOOL;
    ManualExtend : BOOL;
    Watch : FB_EndPositionWatch;
    FailedToExtendAlarm : Alarm.DigitalAlarm;
    FailedToRetractAlarm : Alarm.DigitalAlarm;
    ConflictAlarm : Alarm.DigitalAlarm;
END_VAR
    ExecuteCylinder();

METHOD PROTECTED ExecuteCylinder
VAR
    Supervised : BOOL;
    Safe : BOOL;
    Desired : BOOL;
END_VAR
    Supervised := Mode <> E_Mode#OutOfService;
    CASE Mode OF
        E_Mode#Auto:
            Desired := AutoExtend;
        E_Mode#Manual:
            Desired := ManualExtend;
    ELSE
        Desired := FALSE;
    END_CASE;
    (* Extending, and with double action any movement, counts as a start. *)
    IF Desired <> Wanted AND (Desired OR DoubleActing) AND NOT MayStart() THEN
        Desired := Wanted;
    END_IF;
    Safe := HeldSafe();
    IF Safe AND NOT DoubleActing THEN
        Desired := FALSE;
    END_IF;
    Wanted := Desired;

    Watch(Enable := Supervised AND NOT (Safe AND DoubleActing), TowardsA := Wanted,
        AtA := ExtendedSensor, AtB := RetractedSensor, HasA := HasExtendedSensor,
        HasB := HasRetractedSensor, TimeoutA := ExtendTimeout, TimeoutB := RetractTimeout,
        Clear := TakeReset());
    FailedToExtend := Watch.FailedA;
    FailedToRetract := Watch.FailedB;
    FeedbackConflict := Watch.Conflict;
    Fault := FailedToExtend OR FailedToRetract OR FeedbackConflict;
    Safe := Safe OR Fault;
    IF Safe AND NOT DoubleActing THEN
        Wanted := FALSE;
    END_IF;
    IF Safe AND DoubleActing THEN
        ExtendCommand := FALSE;
        RetractCommand := FALSE;
    ELSE
        ExtendCommand := Wanted;
        RetractCommand := DoubleActing AND NOT Wanted;
    END_IF;
    Extended := Watch.InA;
    Retracted := Watch.InB;
    Moving := Watch.Moving AND NOT (Safe AND DoubleActing);
    IF FeedbackConflict THEN
        FaultText := 'Both end sensors active: check them, then reset';
    ELSIF FailedToExtend THEN
        FaultText := 'Failed to extend: reset to try again';
    ELSIF FailedToRetract THEN
        FaultText := 'Failed to retract: reset to try again';
    ELSE
        FaultText := '';
    END_IF;

    (* Bumpless (§3.4). *)
    ManualExtend := Wanted;

    FailedToExtendAlarm(Condition := FailedToExtend, Suppressed := NOT Supervised,
        Message := 'Failed to extend', Priority := AlarmPriority#Medium);
    FailedToRetractAlarm(Condition := FailedToRetract, Suppressed := NOT Supervised,
        Message := 'Failed to retract', Priority := AlarmPriority#Medium);
    ConflictAlarm(Condition := FeedbackConflict, Suppressed := NOT Supervised,
        Message := 'End sensors conflict', Priority := AlarmPriority#Medium);
    IF Supervised AND Watch.Arrived AND Operations < UDINT#4294967295 THEN
        Operations := Operations + 1;
    END_IF;
    Publish();
    ExtendPermit := StartPermit;
    RetractPermit := Manual AND (NOT DoubleActing OR NOT Blocked);
END_METHOD

METHOD PUBLIC Extend : BOOL
    Extend := AcceptsStart();
    IF Extend THEN
        ManualExtend := TRUE;
    END_IF;
END_METHOD

METHOD PUBLIC Retract : BOOL
    IF DoubleActing THEN
        Retract := AcceptsStart();
    ELSE
        Retract := AcceptsCommands();
    END_IF;
    IF Retract THEN
        ManualExtend := FALSE;
    END_IF;
END_METHOD

METHOD PUBLIC IsExtended : BOOL
    IsExtended := Extended;
END_METHOD

METHOD PUBLIC IsRetracted : BOOL
    IsRetracted := Retracted;
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 3;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    CASE Index OF
        1: GetAlarm := REF(FailedToExtendAlarm);
        2: GetAlarm := REF(FailedToRetractAlarm);
        3: GetAlarm := REF(ConflictAlarm);
    ELSE
        GetAlarm := NULL;
    END_CASE;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
