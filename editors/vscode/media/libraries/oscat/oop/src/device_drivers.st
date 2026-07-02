FUNCTION_BLOCK SingleOutputDriver IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    OutputValue : BOOL;
    ToggleModeValue : BOOL;
    TimeoutValue : TIME;
    DriverFb : DRIVER_1;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
DriverFb(SET_REQ := FALSE, IN := FALSE, RST := TRUE, Toggle_Mode := ToggleModeValue, Timeout := TimeoutValue);
OutputValue := DriverFb.Q;
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
    ToggleMode : BOOL;
    Timeout : TIME;
END_VAR
ToggleModeValue := ToggleMode;
TimeoutValue := Timeout;
ClearError();
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    SetRequest : BOOL;
    Input : BOOL;
END_VAR
DriverFb(SET_REQ := SetRequest, IN := Input, RST := FALSE, Toggle_Mode := ToggleModeValue, Timeout := TimeoutValue);
OutputValue := DriverFb.Q;
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
