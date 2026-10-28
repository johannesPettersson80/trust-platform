(* Analog instruments (specification trust-plant-library §7.1): scaling, quality, the optional
   Control.LowPass filter and four limit alarms. A broken signal raises one signal alarm and
   suppresses the limit alarms. The defaults suit a 4-20 mA signal (NAMUR NE 43); set RawLow,
   RawHigh, BadBelow and BadAbove for other signals. *)
NAMESPACE Plant
USING Alarm;
USING Control;

FUNCTION_BLOCK FB_AnalogInput IMPLEMENTS IMeasurement, IAlarmSource
VAR_INPUT
    (* The channel's value, for example in mA. *)
    Raw : LREAL;
    (* The I/O module's diagnosis for this channel. *)
    SignalFault : BOOL;
END_VAR
VAR PUBLIC
    Unit : STRING[16];
END_VAR
VAR PUBLIC RETAIN
    RawLow : LREAL := 4.0;
    RawHigh : LREAL := 20.0;
    RangeLow : LREAL := 0.0;
    RangeHigh : LREAL := 100.0;
    BadBelow : LREAL := 3.6;
    BadAbove : LREAL := 21.0;
    FilterTime : TIME := T#0s;
    HighHighEnabled : BOOL;
    HighEnabled : BOOL;
    LowEnabled : BOOL;
    LowLowEnabled : BOOL;
    HighHighLimit : LREAL := 95.0;
    HighLimit : LREAL := 90.0;
    LowLimit : LREAL := 10.0;
    LowLowLimit : LREAL := 5.0;
    Deadband : LREAL := 1.0;
    AlarmDelay : TIME := T#0s;
END_VAR
VAR_OUTPUT NON_RETAIN
    Value : LREAL;
    Percent : LREAL;
    Quality : E_Quality := E_Quality#Bad;
    Fault : BOOL;
    HighHigh : BOOL;
    High : BOOL;
    Low : BOOL;
    LowLow : BOOL;
END_VAR
VAR NON_RETAIN
    Filter : LowPass;
    Filtering : BOOL;
    HasValue : BOOL;
    Clock : FB_ScanClock;
    SignalAlarm : Alarm.DigitalAlarm;
    HighHighAlarm : Alarm.LimitAlarm;
    HighAlarm : Alarm.LimitAlarm;
    LowAlarm : Alarm.LimitAlarm;
    LowLowAlarm : Alarm.LimitAlarm;
END_VAR
    Measure();

METHOD PROTECTED Measure
VAR
    Span : LREAL;
    Range : LREAL;
    Scaled : LREAL;
    Parameters : LowPassParameters;
END_VAR
    Clock();
    Span := RawHigh - RawLow;
    Range := RangeHigh - RangeLow;
    IF NOT SignalFault AND F_Finite(Raw) AND Raw >= BadBelow AND Raw <= BadAbove
        AND F_Finite(Span) AND Span <> 0.0 THEN
        Scaled := RangeLow + (Raw - RawLow) * Range / Span;
        IF FilterTime > T#0s AND HasValue THEN
            Parameters.TimeConstant := F_Seconds(FilterTime);
            Parameters.Order := FilterOrder#FirstOrder;
            Filter.Configure(Parameters := Parameters);
            IF NOT Filtering THEN
                Filter.Initialize(Input := Value);
                Filtering := TRUE;
            END_IF;
            IF Clock.Seconds > 0.0 THEN
                Value := Filter.Update(Input := Scaled, CycleTime := Clock.Seconds);
            END_IF;
        ELSE
            Value := Scaled;
            Filtering := FALSE;
        END_IF;
        HasValue := TRUE;
        Quality := E_Quality#Good;
    ELSE
        (* The value holds its last good reading. *)
        Quality := E_Quality#Bad;
        Filtering := FALSE;
    END_IF;
    Fault := Quality = E_Quality#Bad;
    IF Range <> 0.0 AND F_Finite(Range) THEN
        Percent := (Value - RangeLow) / Range * 100.0;
    ELSE
        Percent := 0.0;
    END_IF;

    (* A limit that is not enabled, and every limit while the signal is bad, is suppressed: it
       does not annunciate and its output stays FALSE. *)
    SignalAlarm(Condition := Fault, Message := 'Signal fault', Priority := AlarmPriority#Medium);
    HighHighAlarm(Value := Value, Limit := HighHighLimit, Deadband := ABS(Deadband),
        Direction := LimitDirection#High, OnDelay := AlarmDelay, Suppressed := Fault OR NOT HighHighEnabled,
        Message := 'High-high', Priority := AlarmPriority#High);
    HighAlarm(Value := Value, Limit := HighLimit, Deadband := ABS(Deadband),
        Direction := LimitDirection#High, OnDelay := AlarmDelay, Suppressed := Fault OR NOT HighEnabled,
        Message := 'High', Priority := AlarmPriority#Medium);
    LowAlarm(Value := Value, Limit := LowLimit, Deadband := ABS(Deadband),
        Direction := LimitDirection#Low, OnDelay := AlarmDelay, Suppressed := Fault OR NOT LowEnabled,
        Message := 'Low', Priority := AlarmPriority#Medium);
    LowLowAlarm(Value := Value, Limit := LowLowLimit, Deadband := ABS(Deadband),
        Direction := LimitDirection#Low, OnDelay := AlarmDelay, Suppressed := Fault OR NOT LowLowEnabled,
        Message := 'Low-low', Priority := AlarmPriority#High);
    HighHigh := HighHighAlarm.Active AND NOT HighHighAlarm.SuppressedByDesign;
    High := HighAlarm.Active AND NOT HighAlarm.SuppressedByDesign;
    Low := LowAlarm.Active AND NOT LowAlarm.SuppressedByDesign;
    LowLow := LowLowAlarm.Active AND NOT LowLowAlarm.SuppressedByDesign;
END_METHOD

METHOD PUBLIC GetValue : LREAL
    GetValue := Value;
END_METHOD

METHOD PUBLIC GetQuality : E_Quality
    GetQuality := Quality;
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 5;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    CASE Index OF
        1: GetAlarm := REF(SignalAlarm);
        2: GetAlarm := REF(HighHighAlarm);
        3: GetAlarm := REF(HighAlarm);
        4: GetAlarm := REF(LowAlarm);
        5: GetAlarm := REF(LowLowAlarm);
    ELSE
        GetAlarm := NULL;
    END_CASE;
END_METHOD
END_FUNCTION_BLOCK

(* The instruments below differ in name and symbol; their unit comes from the instance. *)
FUNCTION_BLOCK FB_LevelInput EXTENDS FB_AnalogInput
    Measure();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_PressureInput EXTENDS FB_AnalogInput
    Measure();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_TemperatureInput EXTENDS FB_AnalogInput
    Measure();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_AnalyzerInput EXTENDS FB_AnalogInput
    Measure();
END_FUNCTION_BLOCK

(* A flow instrument with its own totaliser on its measured value (§7.1.6). *)
FUNCTION_BLOCK FB_FlowInput EXTENDS FB_AnalogInput IMPLEMENTS ITotal
VAR_OUTPUT RETAIN
    (* Retained as well as the totaliser's, so it reads right before the first scan. *)
    Total : LREAL;
END_VAR
VAR PUBLIC RETAIN
    (* The time base of the flow's unit: T#1h for m3/h. *)
    TotalPer : TIME := T#1h;
END_VAR
VAR
    Totalizer : FB_Totalizer;
END_VAR
    Measure();
    Totalizer.TotalPer := TotalPer;
    Totalizer(Rate := Value, RateGood := Quality = E_Quality#Good);
    Total := Totalizer.Total;

METHOD PUBLIC ResetTotal : BOOL
    ResetTotal := Totalizer.ResetTotal();
    Total := 0.0;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
