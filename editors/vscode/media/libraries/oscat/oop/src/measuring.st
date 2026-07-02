FUNCTION_BLOCK OntimeMeter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    SecondsValue : UDINT;
    CyclesValue : UDINT;
    MeterFb : ONTIME;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
MeterFb(IN := FALSE, RST := TRUE, SECONDS := SecondsValue, CYCLES := CyclesValue);
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

METHOD PUBLIC Update
VAR_INPUT
    Input : BOOL;
END_VAR
MeterFb(IN := Input, RST := FALSE, SECONDS := SecondsValue, CYCLES := CyclesValue);
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

PUBLIC PROPERTY Seconds : UDINT
GET
    Seconds := SecondsValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Cycles : UDINT
GET
    Cycles := CyclesValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK CycleTimeMeter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    MinValue : TIME;
    MaxValue : TIME;
    LastValue : TIME;
    SystemTimeValue : TIME;
    SystemDaysValue : INT;
    CyclesValue : DWORD;
    MeterFb : CYCLE_TIME;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
MeterFb(RST := TRUE);
MinValue := MeterFb.ct_min;
MaxValue := MeterFb.ct_max;
LastValue := MeterFb.ct_last;
SystemTimeValue := MeterFb.systime;
SystemDaysValue := MeterFb.sysdays;
CyclesValue := MeterFb.cycles;
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

METHOD PUBLIC Update
VAR_INPUT
    ResetStatistics : BOOL;
END_VAR
MeterFb(RST := ResetStatistics);
MinValue := MeterFb.ct_min;
MaxValue := MeterFb.ct_max;
LastValue := MeterFb.ct_last;
SystemTimeValue := MeterFb.systime;
SystemDaysValue := MeterFb.sysdays;
CyclesValue := MeterFb.cycles;
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

PUBLIC PROPERTY Cycles : DWORD
GET
    Cycles := CyclesValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCycleTime : TIME
GET
    LastCycleTime := LastValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK Calibrator IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    OffsetTargetValue : REAL;
    ScaleTargetValue : REAL := REAL#1.0;
    CalibratorFb : CALIBRATE;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
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
    OffsetTarget : REAL;
    ScaleTarget : REAL;
END_VAR
OffsetTargetValue := OffsetTarget;
ScaleTargetValue := ScaleTarget;
ClearError();
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    Sample : REAL;
    CalibrateOffset : BOOL;
    CalibrateScale : BOOL;
END_VAR
CalibratorFb(X := Sample, CO := CalibrateOffset, CS := CalibrateScale, Y_Offset := OffsetTargetValue, Y_Scale := ScaleTargetValue);
OutputValue := CalibratorFb.Y;
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

FUNCTION_BLOCK BarGraphMeter IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    LowValue : BOOL;
    Q1Value : BOOL;
    Q2Value : BOOL;
    Q3Value : BOOL;
    Q4Value : BOOL;
    Q5Value : BOOL;
    Q6Value : BOOL;
    HighValue : BOOL;
    AlarmValue : BOOL;
    TriggerLowValue : REAL;
    TriggerHighValue : REAL;
    AlarmLowValue : BOOL;
    AlarmHighValue : BOOL;
    LogScaleValue : BOOL;
    MeterFb : BAR_GRAPH;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
MeterFb(X := REAL#0.0, rst := TRUE, trigger_Low := TriggerLowValue, trigger_High := TriggerHighValue, Alarm_low := AlarmLowValue, Alarm_high := AlarmHighValue, log_scale := LogScaleValue);
CopyOutputs();
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
Result.Error := ErrorValue OR AlarmValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PRIVATE CopyOutputs
LowValue := MeterFb.LOW;
Q1Value := MeterFb.Q1;
Q2Value := MeterFb.Q2;
Q3Value := MeterFb.Q3;
Q4Value := MeterFb.Q4;
Q5Value := MeterFb.Q5;
Q6Value := MeterFb.Q6;
HighValue := MeterFb.HIGH;
AlarmValue := MeterFb.Alarm;
StatusValue := MeterFb.Status;
END_METHOD

METHOD PUBLIC Configure
VAR_INPUT
    TriggerLow : REAL;
    TriggerHigh : REAL;
    AlarmLow : BOOL;
    AlarmHigh : BOOL;
    LogScale : BOOL;
END_VAR
TriggerLowValue := TriggerLow;
TriggerHighValue := TriggerHigh;
AlarmLowValue := AlarmLow;
AlarmHighValue := AlarmHigh;
LogScaleValue := LogScale;
ClearError();
END_METHOD

METHOD PUBLIC Update
VAR_INPUT
    Sample : REAL;
    ResetAlarm : BOOL;
END_VAR
MeterFb(X := Sample, rst := ResetAlarm, trigger_Low := TriggerLowValue, trigger_High := TriggerHighValue, Alarm_low := AlarmLowValue, Alarm_high := AlarmHighValue, log_scale := LogScaleValue);
CopyOutputs();
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue OR AlarmValue;
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

PUBLIC PROPERTY Low : BOOL
GET
    Low := LowValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY High : BOOL
GET
    High := HighValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Alarm : BOOL
GET
    Alarm := AlarmValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
