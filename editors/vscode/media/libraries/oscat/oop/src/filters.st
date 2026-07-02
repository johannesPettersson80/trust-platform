FUNCTION_BLOCK Pt1Filter IMPLEMENTS IPt1Filter
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    TimeConstantValue : TIME := DefaultPt1TimeConstant;
    GainValue : REAL := DefaultPt1Gain;
    ResetRequested : BOOL;
    FilterFb : FT_PT1;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ClearError();
OutputValue := REAL#0.0;
ResetRequested := TRUE;
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
    TimeConstant : TIME;
    Gain : REAL;
END_VAR
TimeConstantValue := TimeConstant;
GainValue := Gain;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Sample : REAL;
END_VAR
IF NOT ReadyValue THEN
    ReadyValue := TRUE;
END_IF;
IF ResetRequested THEN
    FilterFb(in := Sample, T := T#0s, K := GainValue);
    ResetRequested := FALSE;
ELSE
    FilterFb(in := Sample, T := TimeConstantValue, K := GainValue);
END_IF;
OutputValue := FilterFb.out;
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

PUBLIC PROPERTY TimeConstant : TIME
GET
    TimeConstant := TimeConstantValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Gain : REAL
GET
    Gain := GainValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerivativeFilter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    GainValue : REAL := REAL#1.0;
    LastSample : REAL;
    LastTime : DWORD;
    Initialized : BOOL;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
Initialized := FALSE;
OutputValue := REAL#0.0;
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
    Gain : REAL;
END_VAR
GainValue := Gain;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Sample : REAL;
END_VAR
VAR
    NowTime : DWORD;
    DeltaUs : REAL;
END_VAR
NowTime := T_PLC_US();
DeltaUs := UDINT_TO_REAL(DWORD_TO_UDINT(NowTime) - DWORD_TO_UDINT(LastTime));
IF NOT Initialized THEN
    Initialized := TRUE;
    LastSample := Sample;
    OutputValue := REAL#0.0;
ELSIF DeltaUs > REAL#0.0 THEN
    OutputValue := (Sample - LastSample) / DeltaUs * REAL#1000000.0 * GainValue;
    LastSample := Sample;
ELSE
    OutputValue := REAL#0.0;
END_IF;
LastTime := NowTime;
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
END_FUNCTION_BLOCK

FUNCTION_BLOCK IntegratorFilter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    LimitedValue : BOOL;
    GainValue : REAL := REAL#1.0;
    LowLimitValue : REAL := REAL#-1.0E37;
    HighLimitValue : REAL := REAL#1.0E37;
    ResetRequested : BOOL;
    FilterFb : FT_INT;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ResetRequested := TRUE;
OutputValue := REAL#0.0;
LimitedValue := FALSE;
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
    Gain : REAL;
    Limits : RealRange;
END_VAR
IF Limits.High < Limits.Low THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorInvalidConfiguration;
    StatusValue := BYTE#1;
ELSE
    GainValue := Gain;
    LowLimitValue := Limits.Low;
    HighLimitValue := Limits.High;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Sample : REAL;
    Run : BOOL;
END_VAR
FilterFb(IN := Sample, K := GainValue, RUN := Run, RST := ResetRequested, OUT_MIN := LowLimitValue, OUT_MAX := HighLimitValue);
ResetRequested := FALSE;
OutputValue := FilterFb.OUT;
LimitedValue := FilterFb.LIM;
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

PUBLIC PROPERTY Limited : BOOL
GET
    Limited := LimitedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pt2Filter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    TimeConstantValue : TIME := DefaultPt1TimeConstant;
    DampingValue : REAL := REAL#1.0;
    GainValue : REAL := DefaultPt1Gain;
    ResetRequested : BOOL;
    FilterFb : FT_PT2;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ResetRequested := TRUE;
OutputValue := REAL#0.0;
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
    TimeConstant : TIME;
    Damping : REAL;
    Gain : REAL;
END_VAR
TimeConstantValue := TimeConstant;
DampingValue := Damping;
GainValue := Gain;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Sample : REAL;
END_VAR
IF ResetRequested THEN
    FilterFb(in := Sample, T := T#0s, D := DampingValue, K := GainValue);
    ResetRequested := FALSE;
ELSE
    FilterFb(in := Sample, T := TimeConstantValue, D := DampingValue, K := GainValue);
END_IF;
OutputValue := FilterFb.out;
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
END_FUNCTION_BLOCK

FUNCTION_BLOCK DelayLine16 IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    TriggeredValue : BOOL;
    DelayTimeValue : TIME := T#1s;
    DelayFb : FT_TN16;
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
    DelayTime : TIME;
END_VAR
DelayTimeValue := DelayTime;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Sample : REAL;
END_VAR
DelayFb(in := Sample, T := DelayTimeValue);
OutputValue := DelayFb.out;
TriggeredValue := DelayFb.trig;
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

PUBLIC PROPERTY Triggered : BOOL
GET
    Triggered := TriggeredValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
