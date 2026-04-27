FUNCTION_BLOCK DwordFifo16 IMPLEMENTS IDwordQueue
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    EmptyValue : BOOL := TRUE;
    FullValue : BOOL;
    ValueValue : DWORD;
    QueueFb : FIFO_16;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Reset();
END_METHOD

METHOD PUBLIC Reset
QueueFb(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ValueValue := QueueFb.DOUT;
EmptyValue := QueueFb.EMPTY;
FullValue := QueueFb.FULL;
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

METHOD PUBLIC Push : BOOL
VAR_INPUT
    Value : DWORD;
END_VAR
IF FullValue THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorQueueFull;
    StatusValue := BYTE#1;
    Push := FALSE;
ELSE
    QueueFb(DIN := Value, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
    ValueValue := QueueFb.DOUT;
    EmptyValue := QueueFb.EMPTY;
    FullValue := QueueFb.FULL;
    Push := TRUE;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC TryPop : BOOL
IF EmptyValue THEN
    TryPop := FALSE;
ELSE
    QueueFb(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
    ValueValue := QueueFb.DOUT;
    EmptyValue := QueueFb.EMPTY;
    FullValue := QueueFb.FULL;
    TryPop := TRUE;
    ClearError();
END_IF;
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

PUBLIC PROPERTY Empty : BOOL
GET
    Empty := EmptyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Full : BOOL
GET
    Full := FullValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Value : DWORD
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Capacity : UINT
GET
    Capacity := MaxDwordFifo16Capacity;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK DwordStack16 IMPLEMENTS IDwordStack
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    EmptyValue : BOOL := TRUE;
    FullValue : BOOL;
    ValueValue : DWORD;
    StackFb : STACK_16;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Reset();
END_METHOD

METHOD PUBLIC Reset
StackFb(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ValueValue := StackFb.DOUT;
EmptyValue := StackFb.EMPTY;
FullValue := StackFb.FULL;
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

METHOD PUBLIC Push : BOOL
VAR_INPUT
    Value : DWORD;
END_VAR
IF FullValue THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorQueueFull;
    StatusValue := BYTE#1;
    Push := FALSE;
ELSE
    StackFb(DIN := Value, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
    ValueValue := StackFb.DOUT;
    EmptyValue := StackFb.EMPTY;
    FullValue := StackFb.FULL;
    Push := TRUE;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC TryPop : BOOL
IF EmptyValue THEN
    TryPop := FALSE;
ELSE
    StackFb(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
    ValueValue := StackFb.DOUT;
    EmptyValue := StackFb.EMPTY;
    FullValue := StackFb.FULL;
    TryPop := TRUE;
    ClearError();
END_IF;
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

PUBLIC PROPERTY Empty : BOOL
GET
    Empty := EmptyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Full : BOOL
GET
    Full := FullValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Value : DWORD
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Capacity : UINT
GET
    Capacity := MaxDwordStack16Capacity;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK DwordFifo32 IMPLEMENTS IDwordQueue
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    EmptyValue : BOOL := TRUE;
    FullValue : BOOL;
    ValueValue : DWORD;
    QueueFb : FIFO_32;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Reset();
END_METHOD

METHOD PUBLIC Reset
QueueFb(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ValueValue := QueueFb.DOUT;
EmptyValue := QueueFb.EMPTY;
FullValue := QueueFb.FULL;
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

METHOD PUBLIC Push : BOOL
VAR_INPUT
    Value : DWORD;
END_VAR
IF FullValue THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorQueueFull;
    StatusValue := BYTE#1;
    Push := FALSE;
ELSE
    QueueFb(DIN := Value, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
    ValueValue := QueueFb.DOUT;
    EmptyValue := QueueFb.EMPTY;
    FullValue := QueueFb.FULL;
    Push := TRUE;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC TryPop : BOOL
IF EmptyValue THEN
    TryPop := FALSE;
ELSE
    QueueFb(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
    ValueValue := QueueFb.DOUT;
    EmptyValue := QueueFb.EMPTY;
    FullValue := QueueFb.FULL;
    TryPop := TRUE;
    ClearError();
END_IF;
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

PUBLIC PROPERTY Empty : BOOL
GET
    Empty := EmptyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Full : BOOL
GET
    Full := FullValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Value : DWORD
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Capacity : UINT
GET
    Capacity := MaxDwordFifo32Capacity;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK DwordStack32 IMPLEMENTS IDwordStack
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    EmptyValue : BOOL := TRUE;
    FullValue : BOOL;
    ValueValue : DWORD;
    StackFb : STACK_32;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Reset();
END_METHOD

METHOD PUBLIC Reset
StackFb(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ValueValue := StackFb.DOUT;
EmptyValue := StackFb.EMPTY;
FullValue := StackFb.FULL;
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

METHOD PUBLIC Push : BOOL
VAR_INPUT
    Value : DWORD;
END_VAR
IF FullValue THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorQueueFull;
    StatusValue := BYTE#1;
    Push := FALSE;
ELSE
    StackFb(DIN := Value, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
    ValueValue := StackFb.DOUT;
    EmptyValue := StackFb.EMPTY;
    FullValue := StackFb.FULL;
    Push := TRUE;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC TryPop : BOOL
IF EmptyValue THEN
    TryPop := FALSE;
ELSE
    StackFb(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
    ValueValue := StackFb.DOUT;
    EmptyValue := StackFb.EMPTY;
    FullValue := StackFb.FULL;
    TryPop := TRUE;
    ClearError();
END_IF;
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

PUBLIC PROPERTY Empty : BOOL
GET
    Empty := EmptyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Full : BOOL
GET
    Full := FullValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Value : DWORD
GET
    Value := ValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Capacity : UINT
GET
    Capacity := MaxDwordStack32Capacity;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
