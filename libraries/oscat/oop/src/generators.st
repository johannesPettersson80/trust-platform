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

FUNCTION_BLOCK RandomSignalGenerator IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    TriggeredValue : BOOL;
    PeriodValue : TIME := T#1s;
    AmplitudeValue : REAL := REAL#1.0;
    OffsetValue : REAL;
    GeneratorFb : GEN_RDM;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
OutputValue := REAL#0.0;
TriggeredValue := FALSE;
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
    Period : TIME;
    Amplitude : REAL;
    Offset : REAL;
END_VAR
PeriodValue := Period;
AmplitudeValue := Amplitude;
OffsetValue := Offset;
ClearError();
END_METHOD

METHOD PUBLIC Update : BOOL
GeneratorFb(PT := PeriodValue, AM := AmplitudeValue, OS := OffsetValue);
OutputValue := GeneratorFb.OUT;
TriggeredValue := GeneratorFb.Q;
Update := TriggeredValue;
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

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Triggered : BOOL
GET
    Triggered := TriggeredValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK SineSignalGenerator IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    PositiveHalfValue : BOOL;
    PeriodValue : TIME := T#1s;
    AmplitudeValue : REAL := REAL#1.0;
    OffsetValue : REAL;
    DelayValue : REAL;
    GeneratorFb : GEN_SIN;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
OutputValue := REAL#0.0;
PositiveHalfValue := FALSE;
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
    Period : TIME;
    Amplitude : REAL;
    Offset : REAL;
    Delay : REAL;
END_VAR
PeriodValue := Period;
AmplitudeValue := Amplitude;
OffsetValue := Offset;
DelayValue := Delay;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
GeneratorFb(PT := PeriodValue, AM := AmplitudeValue, OS := OffsetValue, DL := DelayValue);
OutputValue := GeneratorFb.OUT;
PositiveHalfValue := GeneratorFb.Q;
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

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY PositiveHalf : BOOL
GET
    PositiveHalf := PositiveHalfValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK SquareSignalGenerator IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    HighValue : BOOL;
    PeriodValue : TIME := T#1s;
    AmplitudeValue : REAL := REAL#1.0;
    OffsetValue : REAL;
    DutyCycleValue : REAL := REAL#0.5;
    DelayValue : REAL;
    GeneratorFb : GEN_SQR;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
OutputValue := REAL#0.0;
HighValue := FALSE;
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
    Period : TIME;
    Amplitude : REAL;
    Offset : REAL;
    DutyCycle : REAL;
    Delay : REAL;
END_VAR
PeriodValue := Period;
AmplitudeValue := Amplitude;
OffsetValue := Offset;
DutyCycleValue := DutyCycle;
DelayValue := Delay;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
GeneratorFb(PT := PeriodValue, AM := AmplitudeValue, OS := OffsetValue, DC := DutyCycleValue, DL := DelayValue);
OutputValue := GeneratorFb.OUT;
HighValue := GeneratorFb.Q;
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

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY High : BOOL
GET
    High := HighValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK ByteRamp IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : BYTE;
    BusyValue : BOOL;
    HighValue : BOOL;
    LowValue : BOOL;
    RampFb : RMP_B;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
RampFb(SET_HIGH := FALSE, PT := T#0s, E := FALSE, UP := TRUE, RST := TRUE);
OutputValue := RampFb.OUT;
BusyValue := RampFb.BUSY;
HighValue := RampFb.HIGH;
LowValue := RampFb.LOW;
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

METHOD PUBLIC Update : BYTE
VAR_INPUT
    SetHigh : BOOL;
    RampTime : TIME;
    Enabled : BOOL;
    Up : BOOL;
END_VAR
RampFb(SET_HIGH := SetHigh, PT := RampTime, E := Enabled, UP := Up, RST := FALSE);
OutputValue := RampFb.OUT;
BusyValue := RampFb.BUSY;
HighValue := RampFb.HIGH;
LowValue := RampFb.LOW;
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

PUBLIC PROPERTY Output : BYTE
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Busy : BOOL
GET
    Busy := BusyValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK WordRamp IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : WORD;
    BusyValue : BOOL;
    HighValue : BOOL;
    LowValue : BOOL;
    RampFb : RMP_W;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
RampFb(SET_HIGH := FALSE, PT := T#0s, E := FALSE, UP := TRUE, RST := TRUE);
OutputValue := RampFb.OUT;
BusyValue := RampFb.BUSY;
HighValue := RampFb.HIGH;
LowValue := RampFb.LOW;
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

METHOD PUBLIC Update : WORD
VAR_INPUT
    SetHigh : BOOL;
    RampTime : TIME;
    Enabled : BOOL;
    Up : BOOL;
END_VAR
RampFb(SET_HIGH := SetHigh, PT := RampTime, E := Enabled, UP := Up, RST := FALSE);
OutputValue := RampFb.OUT;
BusyValue := RampFb.BUSY;
HighValue := RampFb.HIGH;
LowValue := RampFb.LOW;
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

PUBLIC PROPERTY Output : WORD
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Busy : BOOL
GET
    Busy := BusyValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
