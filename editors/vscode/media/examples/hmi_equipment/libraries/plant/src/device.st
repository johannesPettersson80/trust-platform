(* What every actuator shares (specification trust-plant-library §3, §4): the mode, the interlock
   and permissive, the latched fault and its reset, and the permits and texts the operator panels
   show. A concrete device takes the reset with TakeReset at the start of its scan, sets Fault and
   FaultText, and calls Publish at the end. *)
NAMESPACE Plant

FUNCTION_BLOCK ABSTRACT FB_Device IMPLEMENTS IDevice
VAR_INPUT
    (* While TRUE the device is held in its safe state in every mode. *)
    Interlock : BOOL;
    InterlockText : STRING[80];
    (* While FALSE a start is refused; a running device continues. *)
    Permissive : BOOL := TRUE;
    PermissiveText : STRING[80];
END_VAR
VAR_OUTPUT RETAIN
    Mode : E_Mode;
END_VAR
VAR_OUTPUT NON_RETAIN
    Manual : BOOL;
    OutOfService : BOOL;
    InService : BOOL := TRUE;
    (* A latched trip or supervision failure; Reset clears it once the cause is gone. *)
    Fault : BOOL;
    Interlocked : BOOL;
    (* A start would be refused now, and why. *)
    Blocked : BOOL;
    BlockReason : STRING[80];
    (* The operator may start the device now: Manual and not blocked. *)
    StartPermit : BOOL;
    (* Why an operator command is refused now; empty when it is accepted. *)
    CommandText : STRING[80];
    (* The panel's message line: why the device is blocked, else why a command is refused. *)
    ReasonText : STRING[80];
END_VAR
VAR NON_RETAIN
    ResetRequested : BOOL;
    FaultText : STRING[80];
END_VAR

METHOD PUBLIC SetMode : BOOL
VAR_INPUT
    Value : E_Mode;
END_VAR
    Mode := Value;
    UpdateMode();
    SetMode := TRUE;
END_METHOD

METHOD PUBLIC GetMode : E_Mode
    GetMode := Mode;
END_METHOD

(* Accepted in every mode; applied at the device's next scan. It never starts anything. *)
METHOD PUBLIC Reset : BOOL
    ResetRequested := TRUE;
    Reset := TRUE;
END_METHOD

(* TRUE once per Reset, at the start of the device's next scan. *)
METHOD PROTECTED TakeReset : BOOL
    TakeReset := ResetRequested;
    ResetRequested := FALSE;
END_METHOD

(* Nothing prevents a start: in service, no fault, no interlock and the permissive present. *)
METHOD PROTECTED MayStart : BOOL
    MayStart := Mode <> E_Mode#OutOfService AND NOT Fault AND NOT Interlock AND Permissive;
END_METHOD

(* An operator command applies only in Manual (§3.5). *)
METHOD PROTECTED AcceptsCommands : BOOL
    AcceptsCommands := Mode = E_Mode#Manual;
END_METHOD

(* An operator start applies in Manual while nothing blocks it. *)
METHOD PROTECTED AcceptsStart : BOOL
    AcceptsStart := Mode = E_Mode#Manual AND MayStart();
END_METHOD

(* The device is held in its safe state (§4.6). *)
METHOD PROTECTED HeldSafe : BOOL
    HeldSafe := Interlock OR Fault OR Mode = E_Mode#OutOfService;
END_METHOD

METHOD PROTECTED UpdateMode
    Manual := Mode = E_Mode#Manual;
    OutOfService := Mode = E_Mode#OutOfService;
    InService := NOT OutOfService;
END_METHOD

(* The state, the permits and the texts (§4.2), once Fault and FaultText are set for this scan. *)
METHOD PROTECTED Publish
    UpdateMode();
    Interlocked := Interlock;
    Blocked := OutOfService OR Fault OR Interlock OR NOT Permissive;
    IF OutOfService THEN
        BlockReason := 'Out of service';
    ELSIF Fault THEN
        BlockReason := FaultText;
    ELSIF Interlock THEN
        IF InterlockText = '' THEN
            BlockReason := 'Interlocked';
        ELSE
            BlockReason := CONCAT('Interlocked: ', InterlockText);
        END_IF;
    ELSIF NOT Permissive THEN
        IF PermissiveText = '' THEN
            BlockReason := 'Not permitted';
        ELSE
            BlockReason := CONCAT('Not permitted: ', PermissiveText);
        END_IF;
    ELSE
        BlockReason := '';
    END_IF;
    StartPermit := Manual AND NOT Blocked;
    IF Mode = E_Mode#Auto THEN
        CommandText := 'Auto: switch to Manual';
    ELSIF OutOfService THEN
        CommandText := 'Out of service';
    ELSIF Blocked THEN
        CommandText := BlockReason;
    ELSE
        CommandText := '';
    END_IF;
    IF BlockReason <> '' THEN
        ReasonText := BlockReason;
    ELSE
        ReasonText := CommandText;
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
