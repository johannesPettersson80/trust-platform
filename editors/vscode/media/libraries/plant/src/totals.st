(* Accumulating instruments (specification trust-plant-library §7.3). *)
NAMESPACE Plant

(* Integrates a rate given per TotalPer; a bad or non-finite rate, and a rate at or below CutOff,
   is not counted. *)
FUNCTION_BLOCK FB_Totalizer IMPLEMENTS ITotal
VAR_INPUT
    (* The rate: an instrument by composition, else Rate and its quality. *)
    Measurement : IMeasurement;
    Rate : LREAL;
    RateGood : BOOL := TRUE;
END_VAR
VAR_OUTPUT RETAIN
    Total : LREAL;
END_VAR
VAR PUBLIC RETAIN
    TotalPer : TIME := T#1h;
    CutOff : LREAL := 0.0;
END_VAR
VAR NON_RETAIN
    Clock : FB_ScanClock;
END_VAR
VAR_TEMP
    Per : LREAL;
    Value : LREAL;
    Good : BOOL;
END_VAR
    Clock();
    IF Measurement <> NULL THEN
        Value := Measurement.GetValue();
        Good := Measurement.GetQuality() = E_Quality#Good;
    ELSE
        Value := Rate;
        Good := RateGood;
    END_IF;
    Per := F_Seconds(TotalPer);
    IF Good AND F_Finite(Value) AND Value > CutOff AND Per > 0.0 THEN
        Total := Total + Value * Clock.Seconds / Per;
    END_IF;

METHOD PUBLIC ResetTotal : BOOL
    Total := 0.0;
    ResetTotal := TRUE;
END_METHOD
END_FUNCTION_BLOCK

(* Counts rising edges of Pulse. A pulse that is already high after a restart is not counted. *)
FUNCTION_BLOCK FB_Counter IMPLEMENTS ICounter
VAR_INPUT
    Pulse : BOOL;
END_VAR
VAR_OUTPUT RETAIN
    Count : UDINT;
END_VAR
VAR_OUTPUT NON_RETAIN
    Done : BOOL;
END_VAR
VAR PUBLIC RETAIN
    (* Done at this count; 0 for none. *)
    Preset : UDINT := 0;
END_VAR
VAR NON_RETAIN
    Primed : BOOL;
    Previous : BOOL;
END_VAR
    IF Primed AND Pulse AND NOT Previous AND Count < UDINT#4294967295 THEN
        Count := Count + 1;
    END_IF;
    Previous := Pulse;
    Primed := TRUE;
    Done := Preset > 0 AND Count >= Preset;

METHOD PUBLIC ResetCount : BOOL
    Count := 0;
    Done := FALSE;
    ResetCount := TRUE;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
