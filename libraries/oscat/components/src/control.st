FUNCTION_BLOCK PidController IMPLEMENTS IPidController
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    ActualValue : REAL;
    SetpointValue : REAL;
    SupervisionBandValue : REAL := DefaultPidSupervisionBand;
    ManualValue : BOOL;
    ManualInputValue : REAL;
    OffsetValue : REAL := DefaultPidOffset;
    OutputValue : REAL;
    DifferenceValue : REAL;
    LimitedValue : BOOL;
    LowLimitValue : REAL := DefaultPidLowLimit;
    HighLimitValue : REAL := DefaultPidHighLimit;
    KpValue : REAL := DefaultPidKp;
    TnValue : REAL := DefaultPidTn;
    TvValue : REAL := DefaultPidTv;
    ResetRequested : BOOL;
    ControllerFb : CTRL_PID;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ResetRequested := TRUE;
OutputValue := REAL#0.0;
DifferenceValue := REAL#0.0;
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

METHOD PUBLIC SetKp
VAR_INPUT
    Kp : REAL;
END_VAR
KpValue := Kp;
ClearError();
END_METHOD

METHOD PUBLIC SetIntegralTime
VAR_INPUT
    Tn : REAL;
END_VAR
TnValue := Tn;
ClearError();
END_METHOD

METHOD PUBLIC SetDerivativeTime
VAR_INPUT
    Tv : REAL;
END_VAR
TvValue := Tv;
ClearError();
END_METHOD

METHOD PUBLIC SetNoiseBand
VAR_INPUT
    SupervisionBand : REAL;
END_VAR
SupervisionBandValue := SupervisionBand;
ClearError();
END_METHOD

METHOD PUBLIC SetOffset
VAR_INPUT
    Offset : REAL;
END_VAR
OffsetValue := Offset;
ClearError();
END_METHOD

METHOD PUBLIC SetLimits
VAR_INPUT
    Limits : RealRange;
END_VAR
IF Limits.High < Limits.Low THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorInvalidConfiguration;
    StatusValue := BYTE#1;
ELSE
    LowLimitValue := Limits.Low;
    HighLimitValue := Limits.High;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC SetManual
VAR_INPUT
    Manual : BOOL;
    ManualInput : REAL;
END_VAR
ManualValue := Manual;
ManualInputValue := ManualInput;
ClearError();
END_METHOD

METHOD PUBLIC ApplyGains
VAR_INPUT
    Gains : PidGains;
END_VAR
KpValue := Gains.Kp;
TnValue := Gains.Tn;
TvValue := Gains.Tv;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Actual : REAL;
    Target : REAL;
END_VAR
IF NOT ReadyValue THEN
    ReadyValue := TRUE;
END_IF;
ActualValue := Actual;
SetpointValue := Target;
ControllerFb(
    ACT := ActualValue,
    SET_POINT := SetpointValue,
    SUP := SupervisionBandValue,
    OFS := OffsetValue,
    M_I := ManualInputValue,
    MAN := ManualValue,
    RST := ResetRequested,
    KP := KpValue,
    TN := TnValue,
    TV := TvValue,
    LL := LowLimitValue,
    LH := HighLimitValue
);
ResetRequested := FALSE;
OutputValue := ControllerFb.Y;
DifferenceValue := ControllerFb.DIFF;
LimitedValue := ControllerFb.LIM;
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

PUBLIC PROPERTY Actual : REAL
GET
    Actual := ActualValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY TargetValue : REAL
GET
    TargetValue := SetpointValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY SupervisionBand : REAL
GET
    SupervisionBand := SupervisionBandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Manual : BOOL
GET
    Manual := ManualValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ManualInput : REAL
GET
    ManualInput := ManualInputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Offset : REAL
GET
    Offset := OffsetValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Difference : REAL
GET
    Difference := DifferenceValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Limited : BOOL
GET
    Limited := LimitedValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LowLimit : REAL
GET
    LowLimit := LowLimitValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HighLimit : REAL
GET
    HighLimit := HighLimitValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Kp : REAL
GET
    Kp := KpValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Tn : REAL
GET
    Tn := TnValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Tv : REAL
GET
    Tv := TvValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK HysteresisSwitch IMPLEMENTS IHysteresisSwitch
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    MeasuredValueValue : REAL;
    LowLimitValue : REAL;
    HighLimitValue : REAL;
    QValue : BOOL;
    WindowValue : BOOL;
    SwitchFb : HYST;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
QValue := FALSE;
WindowValue := FALSE;
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

METHOD PUBLIC SetLimits
VAR_INPUT
    Limits : RealRange;
END_VAR
LowLimitValue := Limits.Low;
HighLimitValue := Limits.High;
ClearError();
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    MeasuredValue : REAL;
END_VAR
IF NOT ReadyValue THEN
    ReadyValue := TRUE;
END_IF;
MeasuredValueValue := MeasuredValue;
SwitchFb(In := MeasuredValueValue, HIGH_LIMIT := HighLimitValue, LOW_LIMIT := LowLimitValue);
QValue := SwitchFb.Q;
WindowValue := SwitchFb.WIN;
Update := QValue;
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

PUBLIC PROPERTY MeasuredValue : REAL
GET
    MeasuredValue := MeasuredValueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LowLimit : REAL
GET
    LowLimit := LowLimitValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HighLimit : REAL
GET
    HighLimit := HighLimitValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Q : BOOL
GET
    Q := QValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Window : BOOL
GET
    Window := WindowValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
