(* Small calculations shared by the blocks. *)
NAMESPACE Plant

(* TRUE for a finite number: neither NaN nor infinite. *)
FUNCTION F_Finite : BOOL
VAR_INPUT
    Value : LREAL;
END_VAR
    F_Finite := Value = Value AND ABS(Value) <= 1.7976931348623157E308;
END_FUNCTION

(* A duration in seconds. A negative duration counts as zero; durations are capped at 49 days,
   the range of the millisecond conversion. *)
FUNCTION F_Seconds : LREAL
VAR_INPUT
    Value : TIME;
END_VAR
    IF Value <= T#0s THEN
        F_Seconds := 0.0;
    ELSIF Value >= T#49d THEN
        F_Seconds := 4233600.0;
    ELSE
        F_Seconds := UDINT_TO_LREAL(DWORD_TO_UDINT(TIME_TO_DWORD(Value))) / 1000.0;
    END_IF;
END_FUNCTION

(* The time since the previous call, from the runtime clock; zero at the first call after a
   restart, so a restart never counts the time the PLC was off. *)
FUNCTION_BLOCK FB_ScanClock
VAR_OUTPUT
    Elapsed : TIME;
    Seconds : LREAL;
END_VAR
VAR NON_RETAIN
    Started : BOOL;
    Last : TIME;
END_VAR
VAR_TEMP
    Now : TIME;
END_VAR
    Now := TIME();
    IF Started AND Now >= Last THEN
        Elapsed := Now - Last;
    ELSE
        Elapsed := T#0s;
    END_IF;
    Started := TRUE;
    Last := Now;
    Seconds := F_Seconds(Elapsed);
END_FUNCTION_BLOCK

END_NAMESPACE
