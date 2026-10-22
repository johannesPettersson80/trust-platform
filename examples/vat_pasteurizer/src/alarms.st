(* The vat's alarms in the manner of ISA-18.2: an alarm is raised while its condition is present,
   stays latched until the operator has acknowledged it and its condition has cleared, and the first
   alarm raised since all were clear is kept as the first-out. The horn sounds while an alarm is
   unacknowledged. A critical alarm aborts the batch. *)

FUNCTION_BLOCK AlarmList
VAR_INPUT
    Conditions : ARRAY[1..12] OF BOOL;
    Acknowledge : BOOL;
END_VAR
VAR_OUTPUT
    Active : INT;
    Unacknowledged : INT;
    CriticalActive : BOOL;
    FirstOut : INT;
    Horn : BOOL;
    Raised : ARRAY[1..12] OF BOOL;
END_VAR
VAR
    Entries : ARRAY[1..12] OF AlarmEntry;
    AckEdge : R_TRIG;
    Index : INT;
    (* Emergency stop, level high-high, the two transmitters, over-temperature, agitator, inlet
       and outlet valve abort the batch; cooling valve, pump, heating timeout and a restarted hold
       warn. A variable, since the compiler cannot yet index a constant array by a variable. *)
    Critical : ARRAY[1..12] OF BOOL := [8(TRUE), 4(FALSE)];
END_VAR

AckEdge(CLK := Acknowledge);
Active := 0;
Unacknowledged := 0;
CriticalActive := FALSE;
FOR Index := 1 TO 12 DO
    IF Conditions[Index] AND NOT Entries[Index].Active THEN
        (* A new occurrence needs a new acknowledgement. *)
        Entries[Index].Latched := TRUE;
        Entries[Index].Acknowledged := FALSE;
        IF FirstOut = 0 THEN
            FirstOut := Index;
        END_IF;
    END_IF;
    Entries[Index].Active := Conditions[Index];
    IF AckEdge.Q THEN
        Entries[Index].Acknowledged := TRUE;
    END_IF;
    IF Entries[Index].Latched AND Entries[Index].Acknowledged AND NOT Entries[Index].Active THEN
        Entries[Index].Latched := FALSE;
    END_IF;
    Raised[Index] := Entries[Index].Latched;
    IF Entries[Index].Latched THEN
        Active := Active + 1;
        IF NOT Entries[Index].Acknowledged THEN
            Unacknowledged := Unacknowledged + 1;
        END_IF;
        IF Critical[Index] THEN
            CriticalActive := TRUE;
        END_IF;
    END_IF;
END_FOR;
IF Active = 0 THEN
    FirstOut := 0;
END_IF;
Horn := Unacknowledged > 0;
END_FUNCTION_BLOCK
