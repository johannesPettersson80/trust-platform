FUNCTION_BLOCK PulseGenerator IMPLEMENTS IPulseGenerator
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    EnabledValue : BOOL;
    OutputValue : BOOL;
    HighTimeValue : TIME := DefaultPulseHighTime;
    LowTimeValue : TIME := DefaultPulseLowTime;
    PulseFb : GEN_PULSE;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
EnabledValue := FALSE;
PulseFb(ENQ := FALSE, PTH := HighTimeValue, PTL := LowTimeValue);
OutputValue := PulseFb.Q;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := ComponentErrorNone;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC Configure
VAR_INPUT
    HighTime : TIME;
    LowTime : TIME;
END_VAR
HighTimeValue := HighTime;
LowTimeValue := LowTime;
ClearError();
END_METHOD

METHOD PUBLIC SetEnabled
VAR_INPUT
    Enabled : BOOL;
END_VAR
EnabledValue := Enabled;
ClearError();
END_METHOD

METHOD PUBLIC Update : BOOL
IF NOT ReadyValue THEN
    ReadyValue := TRUE;
END_IF;
PulseFb(ENQ := EnabledValue, PTH := HighTimeValue, PTL := LowTimeValue);
OutputValue := PulseFb.Q;
Update := OutputValue;
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Enabled : BOOL
GET
    Enabled := EnabledValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Output : BOOL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HighTime : TIME
GET
    HighTime := HighTimeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LowTime : TIME
GET
    LowTime := LowTimeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
