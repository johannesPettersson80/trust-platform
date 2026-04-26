FUNCTION_BLOCK Pt1Filter IMPLEMENTS IPt1Filter
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : REAL;
    TimeConstantValue : TIME := DefaultPt1TimeConstant;
    GainValue : REAL := DefaultPt1Gain;
    FilterFb : FT_PT1;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ClearError();
OutputValue := REAL#0.0;
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
FilterFb(in := Sample, T := TimeConstantValue, K := GainValue);
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
