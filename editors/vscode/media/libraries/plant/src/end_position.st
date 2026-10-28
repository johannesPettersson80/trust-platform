(* Supervision of a two-position actuator (specification trust-plant-library §6.6–§6.8), shared by
   the on/off valve, the damper and the cylinder. Position A is open or extended, B closed or
   retracted. Without a switch, that end position follows the command. *)
NAMESPACE Plant

FUNCTION_BLOCK FB_EndPositionWatch
VAR_INPUT
    (* FALSE out of service, or while nothing is commanded. *)
    Enable : BOOL;
    (* Commanded to A; else to B. *)
    TowardsA : BOOL;
    AtA : BOOL;
    AtB : BOOL;
    HasA : BOOL;
    HasB : BOOL;
    TimeoutA : TIME;
    TimeoutB : TIME;
    (* A reset: clears the latched failures and restarts the timing. *)
    Clear : BOOL;
END_VAR
VAR_OUTPUT NON_RETAIN
    InA : BOOL;
    InB : BOOL;
    Moving : BOOL;
    FailedA : BOOL;
    FailedB : BOOL;
    Conflict : BOOL;
    (* TRUE for one scan when the commanded end position is reached: one operation. *)
    Arrived : BOOL;
END_VAR
VAR NON_RETAIN
    WatchA : TON;
    WatchB : TON;
    ConflictWatch : TON;
    WasInA : BOOL;
    WasInB : BOOL;
    Primed : BOOL;
END_VAR
    IF Clear THEN
        FailedA := FALSE;
        FailedB := FALSE;
        Conflict := FALSE;
        WatchA(IN := FALSE);
        WatchB(IN := FALSE);
        ConflictWatch(IN := FALSE);
    END_IF;
    IF HasA THEN
        InA := AtA;
    ELSE
        InA := TowardsA;
    END_IF;
    IF HasB THEN
        InB := AtB;
    ELSE
        InB := NOT TowardsA;
    END_IF;
    Moving := (TowardsA AND NOT InA) OR (NOT TowardsA AND NOT InB);
    (* Not reaching the commanded end position in time, or leaving it, is a failure. *)
    WatchA(IN := Enable AND TowardsA AND NOT InA, PT := TimeoutA);
    WatchB(IN := Enable AND NOT TowardsA AND NOT InB, PT := TimeoutB);
    ConflictWatch(IN := Enable AND HasA AND HasB AND AtA AND AtB, PT := T#1s);
    IF WatchA.Q THEN
        FailedA := TRUE;
    END_IF;
    IF WatchB.Q THEN
        FailedB := TRUE;
    END_IF;
    IF ConflictWatch.Q THEN
        Conflict := TRUE;
    END_IF;
    Arrived := Primed AND ((TowardsA AND InA AND NOT WasInA) OR (NOT TowardsA AND InB AND NOT WasInB));
    WasInA := InA;
    WasInB := InB;
    Primed := TRUE;
END_FUNCTION_BLOCK

END_NAMESPACE
