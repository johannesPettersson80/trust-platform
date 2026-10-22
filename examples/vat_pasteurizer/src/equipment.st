(* The vat's equipment: a valve or a motor with its feedback, and a 4-20 mA transmitter. *)

(* A valve with an open limit switch, or a motor with its contactor's running feedback: it is
   driven while it is commanded and its interlock allows it. A feedback that does not follow the
   output within the supervision time is a fault, which keeps it off until it is reset. *)
FUNCTION_BLOCK Actuator
VAR_INPUT
    Command : BOOL;
    Interlock : BOOL := TRUE;
    Feedback : BOOL;
    Reset : BOOL;
    SupervisionTime : TIME := T#3s;
END_VAR
VAR_OUTPUT
    Output : BOOL;
    Fault : BOOL;
END_VAR
VAR
    Mismatch : TON;
END_VAR

IF Reset THEN
    Fault := FALSE;
END_IF;
Output := Command AND Interlock AND NOT Fault;
Mismatch(IN := Output <> Feedback, PT := SupervisionTime);
IF Mismatch.Q THEN
    Fault := TRUE;
    Output := FALSE;
END_IF;
END_FUNCTION_BLOCK

(* A 4-20 mA transmitter read as raw counts, 0 at 4 mA and 27648 at 20 mA, scaled to its range.
   Below 3.6 mA or above 21 mA the loop is broken or shorted (NAMUR NE 43): the value is then a fault
   and keeps its last good reading. *)
FUNCTION_BLOCK AnalogInput
VAR_INPUT
    Raw : INT;
    RangeLow : REAL;
    RangeHigh : REAL;
END_VAR
VAR_OUTPUT
    Value : REAL;
    Fault : BOOL;
END_VAR

Fault := Raw < -691 OR Raw > 29376;
IF NOT Fault THEN
    Value := RangeLow + (RangeHigh - RangeLow) * INT_TO_REAL(Raw) / 27648.0;
END_IF;
END_FUNCTION_BLOCK
