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
    ControlErrorValue : REAL;
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
ControlErrorValue := REAL#0.0;
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

METHOD PUBLIC SetSupervisionBand
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

METHOD PUBLIC Configure
VAR_INPUT
    Gains : PidGains;
    Limits : RealRange;
    SupervisionBand : REAL;
    Offset : REAL;
END_VAR
IF Limits.High < Limits.Low THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorInvalidConfiguration;
    StatusValue := BYTE#1;
ELSE
    KpValue := Gains.Kp;
    TnValue := Gains.Tn;
    TvValue := Gains.Tv;
    LowLimitValue := Limits.Low;
    HighLimitValue := Limits.High;
    SupervisionBandValue := SupervisionBand;
    OffsetValue := Offset;
    ClearError();
END_IF;
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
ControlErrorValue := ControllerFb.DIFF;
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

PUBLIC PROPERTY ControlError : REAL
GET
    ControlError := ControlErrorValue;
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

FUNCTION_BLOCK PiController IMPLEMENTS IClosedLoopController
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    ActualValue : REAL;
    SetpointValue : REAL;
    OutputValue : REAL;
    ControlErrorValue : REAL;
    LimitedValue : BOOL;
    SupervisionBandValue : REAL := DefaultPidSupervisionBand;
    ManualValue : BOOL;
    ManualInputValue : REAL;
    OffsetValue : REAL := DefaultPidOffset;
    LowLimitValue : REAL := DefaultPidLowLimit;
    HighLimitValue : REAL := DefaultPidHighLimit;
    KpValue : REAL := DefaultPidKp;
    KiValue : REAL := REAL#1.0;
    ResetRequested : BOOL;
    ControllerFb : CTRL_PI;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ResetRequested := TRUE;
OutputValue := REAL#0.0;
ControlErrorValue := REAL#0.0;
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
    Gains : PiGains;
    Limits : RealRange;
    SupervisionBand : REAL;
    Offset : REAL;
END_VAR
IF Limits.High < Limits.Low THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorInvalidConfiguration;
    StatusValue := BYTE#1;
ELSE
    KpValue := Gains.Kp;
    KiValue := Gains.Ki;
    LowLimitValue := Limits.Low;
    HighLimitValue := Limits.High;
    SupervisionBandValue := SupervisionBand;
    OffsetValue := Offset;
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
    KI := KiValue,
    LL := LowLimitValue,
    LH := HighLimitValue
);
ResetRequested := FALSE;
OutputValue := ControllerFb.Y;
ControlErrorValue := ControllerFb.DIFF;
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

PUBLIC PROPERTY Output : REAL
GET
    Output := OutputValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ControlError : REAL
GET
    ControlError := ControlErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Limited : BOOL
GET
    Limited := LimitedValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK PwmController IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : BOOL;
    FrequencyValue : REAL;
    ManualValue : BOOL;
    ManualInputValue : REAL;
    ControllerFb : CTRL_PWM;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ControllerFb(CI := REAL#0.0, MAN_IN := REAL#0.0, MANUAL := FALSE, F := REAL#0.0);
OutputValue := ControllerFb.Q;
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
    Frequency : REAL;
END_VAR
FrequencyValue := Frequency;
ClearError();
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

METHOD PUBLIC Update : BOOL
VAR_INPUT
    ControlInput : REAL;
END_VAR
IF NOT ReadyValue THEN
    ReadyValue := TRUE;
END_IF;
ControllerFb(CI := ControlInput, MAN_IN := ManualInputValue, MANUAL := ManualValue, F := FrequencyValue);
OutputValue := ControllerFb.Q;
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

PUBLIC PROPERTY Frequency : REAL
GET
    Frequency := FrequencyValue;
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

METHOD PUBLIC Update : BOOL
VAR_INPUT
    MeasuredValue : REAL;
END_VAR
IF NOT ReadyValue THEN
    ReadyValue := TRUE;
END_IF;
MeasuredValueValue := MeasuredValue;
IF MeasuredValueValue < LowLimitValue THEN
    QValue := FALSE;
    WindowValue := FALSE;
ELSIF MeasuredValueValue > HighLimitValue THEN
    QValue := TRUE;
    WindowValue := FALSE;
ELSE
    WindowValue := TRUE;
END_IF;
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

FUNCTION_BLOCK TankLevelController IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    ValveValue : BOOL;
    AlarmValue : BOOL;
    MaxValveTimeValue : TIME;
    LevelDelayTimeValue : TIME;
    TankFb : TANK_LEVEL;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
TankFb(LEVEL := FALSE, LEAK := FALSE, ACLR := TRUE, MAX_VALVE_TIME := MaxValveTimeValue, LEVEL_DELAY_TIME := LevelDelayTimeValue);
ValveValue := TankFb.VALVE;
AlarmValue := TankFb.ALARM;
StatusValue := TankFb.STATUS;
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
    MaxValveTime : TIME;
    LevelDelayTime : TIME;
END_VAR
MaxValveTimeValue := MaxValveTime;
LevelDelayTimeValue := LevelDelayTime;
ClearError();
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    LevelReached : BOOL;
    LeakDetected : BOOL;
    AlarmClear : BOOL;
END_VAR
TankFb(LEVEL := LevelReached, LEAK := LeakDetected, ACLR := AlarmClear, MAX_VALVE_TIME := MaxValveTimeValue, LEVEL_DELAY_TIME := LevelDelayTimeValue);
ValveValue := TankFb.VALVE;
AlarmValue := TankFb.ALARM;
StatusValue := TankFb.STATUS;
Update := ValveValue;
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

PUBLIC PROPERTY Valve : BOOL
GET
    Valve := ValveValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Alarm : BOOL
GET
    Alarm := AlarmValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK HeatCurve IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    FlowTemperatureValue : REAL;
    HeatRequiredValue : BOOL;
    MaxFlowTemperatureValue : REAL := REAL#70.0;
    MinFlowTemperatureValue : REAL := REAL#25.0;
    ConfigFlowTemperatureValue : REAL := REAL#70.0;
    ConfigInsideTemperatureValue : REAL := REAL#20.0;
    ConfigOutsideTemperatureValue : REAL := REAL#-15.0;
    FlowDifferenceValue : REAL := REAL#10.0;
    CurveValue : REAL := REAL#1.33;
    HysteresisValue : REAL := REAL#3.0;
    HeatFb : HEAT_TEMP;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
FlowTemperatureValue := REAL#0.0;
HeatRequiredValue := FALSE;
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
    MaxFlowTemperature : REAL;
    MinFlowTemperature : REAL;
    ConfigFlowTemperature : REAL;
    ConfigInsideTemperature : REAL;
    ConfigOutsideTemperature : REAL;
    FlowDifference : REAL;
    Curve : REAL;
    Hysteresis : REAL;
END_VAR
MaxFlowTemperatureValue := MaxFlowTemperature;
MinFlowTemperatureValue := MinFlowTemperature;
ConfigFlowTemperatureValue := ConfigFlowTemperature;
ConfigInsideTemperatureValue := ConfigInsideTemperature;
ConfigOutsideTemperatureValue := ConfigOutsideTemperature;
FlowDifferenceValue := FlowDifference;
CurveValue := Curve;
HysteresisValue := Hysteresis;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    OutsideTemperature : REAL;
    InsideTemperature : REAL;
    Offset : REAL;
    RequestedTemperature : REAL;
END_VAR
HeatFb(
    T_EXT := OutsideTemperature,
    T_INT := InsideTemperature,
    OFFSET := Offset,
    T_REQ := RequestedTemperature,
    TY_MAX := MaxFlowTemperatureValue,
    TY_MIN := MinFlowTemperatureValue,
    TY_CONFIG := ConfigFlowTemperatureValue,
    T_INT_CONFIG := ConfigInsideTemperatureValue,
    T_EXT_CONFIG := ConfigOutsideTemperatureValue,
    T_DIFF := FlowDifferenceValue,
    C := CurveValue,
    H := HysteresisValue
);
FlowTemperatureValue := HeatFb.TY;
HeatRequiredValue := HeatFb.HEAT;
Update := FlowTemperatureValue;
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

PUBLIC PROPERTY FlowTemperature : REAL
GET
    FlowTemperature := FlowTemperatureValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HeatRequired : BOOL
GET
    HeatRequired := HeatRequiredValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK BoilerController IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    HeatValue : BOOL;
    BoilerErrorValue : BOOL;
    BoilerFb : BOILER;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
HeatValue := FALSE;
BoilerErrorValue := FALSE;
StatusValue := BYTE#0;
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
Result.Error := ErrorValue OR BoilerErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    UpperTemperature : REAL;
    LowerTemperature : REAL;
    PressureOk : BOOL;
    Enabled : BOOL;
    Request1 : BOOL;
    Request2 : BOOL;
    Boost : BOOL;
END_VAR
BoilerFb(
    T_upper := UpperTemperature,
    T_lower := LowerTemperature,
    Pressure := PressureOk,
    enable := Enabled,
    Req_1 := Request1,
    Req_2 := Request2,
    Boost := Boost
);
HeatValue := BoilerFb.Heat;
BoilerErrorValue := BoilerFb.error;
StatusValue := BoilerFb.status;
Update := HeatValue;
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue OR BoilerErrorValue;
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

PUBLIC PROPERTY Heat : BOOL
GET
    Heat := HeatValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
