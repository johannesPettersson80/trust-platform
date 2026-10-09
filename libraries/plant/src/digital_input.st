(* Digital instruments (specification trust-plant-library §7.2): inversion, debounce, quality and
   an optional switch alarm. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK FB_DigitalInput IMPLEMENTS IAlarmSource
VAR_INPUT
    Input : BOOL;
    (* The I/O module's diagnosis for this channel. *)
    SignalFault : BOOL;
END_VAR
VAR PUBLIC RETAIN
    Invert : BOOL;
    Debounce : TIME := T#0s;
    AlarmEnabled : BOOL;
    (* The value that raises the switch alarm. *)
    AlarmWhen : BOOL := TRUE;
    AlarmText : STRING[80] := 'Switch active';
END_VAR
VAR_OUTPUT NON_RETAIN
    Value : BOOL;
    Quality : E_Quality := E_Quality#Bad;
END_VAR
VAR NON_RETAIN
    Initialized : BOOL;
    OnTimer : TON;
    OffTimer : TON;
    SwitchAlarm : Alarm.DigitalAlarm;
    SignalAlarm : Alarm.DigitalAlarm;
END_VAR
    Sense();

METHOD PROTECTED Sense
VAR
    Level : BOOL;
END_VAR
    Level := Input XOR Invert;
    IF NOT Initialized THEN
        Value := Level;
        Initialized := TRUE;
    END_IF;
    OnTimer(IN := Level AND NOT Value, PT := Debounce);
    OffTimer(IN := NOT Level AND Value, PT := Debounce);
    IF OnTimer.Q THEN
        Value := TRUE;
    ELSIF OffTimer.Q THEN
        Value := FALSE;
    END_IF;
    IF SignalFault THEN
        Quality := E_Quality#Bad;
    ELSE
        Quality := E_Quality#Good;
    END_IF;
    SwitchAlarm(Condition := AlarmEnabled AND Value = AlarmWhen, Suppressed := SignalFault,
        Message := AlarmText, Priority := AlarmPriority#Medium);
    SignalAlarm(Condition := SignalFault, Message := 'Signal fault', Priority := AlarmPriority#Medium);
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 2;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    CASE Index OF
        1: GetAlarm := REF(SwitchAlarm);
        2: GetAlarm := REF(SignalAlarm);
    ELSE
        GetAlarm := NULL;
    END_CASE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_LevelSwitch EXTENDS FB_DigitalInput
    Sense();
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_PressureSwitch EXTENDS FB_DigitalInput
    Sense();
END_FUNCTION_BLOCK

END_NAMESPACE
