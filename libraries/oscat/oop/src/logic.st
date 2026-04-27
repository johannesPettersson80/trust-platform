FUNCTION_BLOCK Latch IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : BOOL;
    LatchFb : LTCH;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
LatchFb(DATA := FALSE, L := FALSE, RST := TRUE);
OutputValue := LatchFb.Q;
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

METHOD PUBLIC Update : BOOL
VAR_INPUT
    Data : BOOL;
    Load : BOOL;
END_VAR
LatchFb(DATA := Data, L := Load, RST := FALSE);
OutputValue := LatchFb.Q;
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

PUBLIC PROPERTY Output : BOOL
GET
    Output := OutputValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK ToggleSwitch IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : BOOL;
    ToggleFb : TOGGLE;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ToggleFb(CLK := FALSE, RST := TRUE);
OutputValue := ToggleFb.Q;
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

METHOD PUBLIC Update : BOOL
VAR_INPUT
    Clock : BOOL;
END_VAR
ToggleFb(CLK := Clock, RST := FALSE);
OutputValue := ToggleFb.Q;
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

PUBLIC PROPERTY Output : BOOL
GET
    Output := OutputValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK DwordCounter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    CountValue : DWORD;
    StepSizeValue : DWORD := DWORD#1;
    MaxValueValue : DWORD := DWORD#16#FFFFFFFF;
    CounterFb : COUNT_DR;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
CounterFb(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := StepSizeValue, MX := MaxValueValue, RST := TRUE);
CountValue := CounterFb.CNT;
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
    StepSize : DWORD;
    MaxValue : DWORD;
END_VAR
StepSizeValue := StepSize;
MaxValueValue := MaxValue;
ClearError();
END_METHOD

METHOD PUBLIC SetValue
VAR_INPUT
    Value : DWORD;
END_VAR
CounterFb(SetIn := TRUE, IN := Value, UP := FALSE, DN := FALSE, StepSize := StepSizeValue, MX := MaxValueValue, RST := FALSE);
CountValue := CounterFb.CNT;
ClearError();
END_METHOD

METHOD PUBLIC Update : DWORD
VAR_INPUT
    Up : BOOL;
    Down : BOOL;
END_VAR
CounterFb(SetIn := FALSE, IN := DWORD#0, UP := Up, DN := Down, StepSize := StepSizeValue, MX := MaxValueValue, RST := FALSE);
CountValue := CounterFb.CNT;
Update := CountValue;
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

PUBLIC PROPERTY Count : DWORD
GET
    Count := CountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK ShiftRegister8 IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : BOOL;
    ShiftFb : SHR_8PLE;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ShiftFb(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := TRUE, LoadIn := FALSE, RST := TRUE);
OutputValue := ShiftFb.DOut;
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

METHOD PUBLIC Update : BOOL
VAR_INPUT
    DataIn : BOOL;
    LoadValue : BYTE;
    Clock : BOOL;
    Up : BOOL;
    Load : BOOL;
END_VAR
ShiftFb(Din := DataIn, Dload := LoadValue, CLK := Clock, UP := Up, LoadIn := Load, RST := FALSE);
OutputValue := ShiftFb.DOut;
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

PUBLIC PROPERTY Output : BOOL
GET
    Output := OutputValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
