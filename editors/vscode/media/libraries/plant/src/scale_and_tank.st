(* The weigh scale and the tank (specification trust-plant-library §7.4, §7.5). *)
NAMESPACE Plant

(* An analog instrument with tare and stability. Tare is accepted only while the reading is
   stable and good. *)
FUNCTION_BLOCK FB_WeighScale EXTENDS FB_AnalogInput IMPLEMENTS ITare
VAR_OUTPUT NON_RETAIN
    Net : LREAL;
    Stable : BOOL;
END_VAR
VAR_OUTPUT RETAIN
    TareWeight : LREAL;
END_VAR
VAR PUBLIC RETAIN
    StableBand : LREAL := 0.5;
    StableTime : TIME := T#2s;
END_VAR
VAR NON_RETAIN
    Reference : LREAL;
    StableTimer : TON;
END_VAR
    Measure();
    Weigh();

METHOD PROTECTED Weigh
    IF Quality = E_Quality#Good AND ABS(Value - Reference) <= ABS(StableBand) THEN
        StableTimer(IN := TRUE, PT := StableTime);
    ELSE
        Reference := Value;
        StableTimer(IN := FALSE, PT := StableTime);
    END_IF;
    Stable := StableTimer.Q AND Quality = E_Quality#Good;
    Net := Value - TareWeight;
END_METHOD

METHOD PUBLIC Tare : BOOL
    Tare := Stable;
    IF Tare THEN
        TareWeight := Value;
        Net := 0.0;
    END_IF;
END_METHOD

METHOD PUBLIC ClearTare : BOOL
    TareWeight := 0.0;
    Net := Value;
    ClearTare := TRUE;
END_METHOD
END_FUNCTION_BLOCK

(* A tank shows the level of its level instrument, chosen by composition. It owns no filter and
   no alarms: the instrument owns them. *)
FUNCTION_BLOCK FB_Tank IMPLEMENTS IMeasurement
VAR_INPUT
    LevelInstrument : IMeasurement;
END_VAR
VAR PUBLIC RETAIN
    (* The level at 100 %. *)
    Height : LREAL := 100.0;
    (* The volume at Height; 0 when the volume is not shown. *)
    Capacity : LREAL := 0.0;
END_VAR
VAR_OUTPUT NON_RETAIN
    Level : LREAL;
    Percent : LREAL;
    Volume : LREAL;
    Quality : E_Quality := E_Quality#Bad;
END_VAR
    IF LevelInstrument = NULL THEN
        Quality := E_Quality#Bad;
    ELSE
        Level := LevelInstrument.GetValue();
        Quality := LevelInstrument.GetQuality();
    END_IF;
    IF Height > 0.0 THEN
        Percent := Level / Height * 100.0;
    ELSE
        Percent := 0.0;
    END_IF;
    IF Capacity > 0.0 THEN
        Volume := Capacity * Percent / 100.0;
    ELSE
        Volume := 0.0;
    END_IF;

METHOD PUBLIC GetValue : LREAL
    GetValue := Level;
END_METHOD

METHOD PUBLIC GetQuality : E_Quality
    GetQuality := Quality;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
