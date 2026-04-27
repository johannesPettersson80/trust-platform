(* Pattern role: IMotorDrive is the contract boundary for the Adapter example. *)
INTERFACE IMotorDrive EXTENDS IComponent
    PROPERTY ActualSpeedHz : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ActualCurrentA : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY MotorTempC : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY IsRunning : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY HasFault : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY FaultCode : DWORD
    GET
    END_GET
    END_PROPERTY

    METHOD Command
    VAR_INPUT
        SpeedReferenceHz : REAL;
        Direction : INT;
    END_VAR
    END_METHOD

    METHOD Stop
    END_METHOD

    METHOD ResetFault
    END_METHOD
END_INTERFACE

(* Pattern role: AbbAcs580Adapter is a concrete implementation in the Adapter example. *)
FUNCTION_BLOCK AbbAcs580Adapter IMPLEMENTS IMotorDrive
VAR
    StatusWordValue : WORD;
    FaultCodeRawValue : WORD;
    SpeedActualRaw : INT;
    CommandedSpeed : REAL;
    RunningValue : BOOL;
    FaultValue : BOOL;
    ReadyValue : BOOL;
END_VAR

METHOD PUBLIC ConfigureRaw
VAR_INPUT
    StatusWord : WORD;
    FaultCodeRaw : WORD;
    SpeedRaw : INT;
END_VAR
StatusWordValue := StatusWord;
FaultCodeRawValue := FaultCodeRaw;
SpeedActualRaw := SpeedRaw;
(* ABB ACS580 adapter: bit 2 is the running indication; bit 3 below is the fault indication. *)
RunningValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0004) <> DWORD#0;
FaultValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0008) <> DWORD#0;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
END_METHOD

METHOD PUBLIC Reset
FaultValue := FALSE;
END_METHOD

METHOD PUBLIC ClearError
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    S : ComponentStatus;
END_VAR
S.Ready := ReadyValue;
Snapshot := S;
END_METHOD

METHOD PUBLIC Command
VAR_INPUT
    SpeedReferenceHz : REAL;
    Direction : INT;
END_VAR
(* Command sets the requested speed; IsRunning is sourced from the
   vendor status word (ConfigureRaw) so injected fault/no-feedback
   scenarios are not masked by the command. *)
CommandedSpeed := SpeedReferenceHz;
END_METHOD

METHOD PUBLIC Stop
CommandedSpeed := REAL#0.0;
RunningValue := FALSE;
END_METHOD

METHOD PUBLIC ResetFault
FaultValue := FALSE;
FaultCodeRawValue := WORD#0;
END_METHOD

PUBLIC PROPERTY ActualSpeedHz : REAL
GET
    ActualSpeedHz := INT_TO_REAL(SpeedActualRaw) / REAL#10.0;
END_GET
END_PROPERTY

PUBLIC PROPERTY ActualCurrentA : REAL
GET
    ActualCurrentA := REAL#12.0;
END_GET
END_PROPERTY

PUBLIC PROPERTY MotorTempC : REAL
GET
    MotorTempC := REAL#45.0;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HasFault : BOOL
GET
    HasFault := FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCode : DWORD
GET
    FaultCode := DWORD#16#10000000 OR WORD_TO_DWORD(FaultCodeRawValue);
END_GET
END_PROPERTY

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := FaultValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := FaultCodeRawValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := BYTE#0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: DanfossFc302Adapter customizes inherited behavior in the Adapter example. *)
FUNCTION_BLOCK DanfossFc302Adapter EXTENDS AbbAcs580Adapter
METHOD PUBLIC ConfigureRaw
VAR_INPUT
    StatusWord : WORD;
    FaultCodeRaw : WORD;
    SpeedRaw : INT;
END_VAR
StatusWordValue := StatusWord;
FaultCodeRawValue := FaultCodeRaw;
SpeedActualRaw := SpeedRaw;
(* Danfoss FC-302 adapter: bit 8 is running and bit 1 is fault in this normalized test profile. *)
RunningValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0100) <> DWORD#0;
FaultValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0002) <> DWORD#0;
END_METHOD

PUBLIC PROPERTY FaultCode : DWORD
GET
    FaultCode := DWORD#16#20000000 OR WORD_TO_DWORD(FaultCodeRawValue);
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: SiemensG120Adapter customizes inherited behavior in the Adapter example. *)
FUNCTION_BLOCK SiemensG120Adapter EXTENDS AbbAcs580Adapter
METHOD PUBLIC ConfigureRaw
VAR_INPUT
    StatusWord : WORD;
    FaultCodeRaw : WORD;
    SpeedRaw : INT;
END_VAR
StatusWordValue := StatusWord;
FaultCodeRawValue := FaultCodeRaw;
SpeedActualRaw := SpeedRaw;
(* Siemens G120 adapter: PROFIdrive-style bit 10 is used as the running indication here. *)
RunningValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0400) <> DWORD#0;
FaultValue := (WORD_TO_DWORD(StatusWordValue) AND DWORD#16#0008) <> DWORD#0;
END_METHOD

PUBLIC PROPERTY FaultCode : DWORD
GET
    FaultCode := DWORD#16#30000000 OR WORD_TO_DWORD(FaultCodeRawValue);
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: MotorCell owns the collaborating objects used by this Adapter example. *)
FUNCTION_BLOCK MotorCell
VAR
    Abb1 : AbbAcs580Adapter;
    Danfoss1 : DanfossFc302Adapter;
    Siemens1 : SiemensG120Adapter;
    Drive1 : IMotorDrive;
    Drive2 : IMotorDrive;
    Drive3 : IMotorDrive;
    RunningCountValue : INT;
    FaultCountValue : INT;
    LastFaultCodeValue : DWORD;
END_VAR

METHOD PUBLIC Initialize
Abb1.Initialize();
Danfoss1.Initialize();
Siemens1.Initialize();
Drive1 := Abb1;
Drive2 := Danfoss1;
Drive3 := Siemens1;
END_METHOD

METHOD PRIVATE ApplyDrive
VAR_INPUT
    Drive : IMotorDrive;
    RunCommand : BOOL;
    SpeedHz : REAL;
END_VAR
IF RunCommand THEN
    Drive.Command(SpeedReferenceHz := SpeedHz, Direction := INT#1);
ELSE
    Drive.Stop();
END_IF;
IF Drive.IsRunning THEN
    RunningCountValue := RunningCountValue + INT#1;
END_IF;
IF Drive.HasFault THEN
    FaultCountValue := FaultCountValue + INT#1;
    LastFaultCodeValue := Drive.FaultCode;
END_IF;
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    RunCommand : BOOL;
    PullCordOk : BOOL;
    SpeedHz : REAL;
END_VAR
RunningCountValue := INT#0;
FaultCountValue := INT#0;
LastFaultCodeValue := DWORD#0;
IF PullCordOk THEN
    ApplyDrive(Drive := Drive1, RunCommand := RunCommand, SpeedHz := SpeedHz);
    ApplyDrive(Drive := Drive2, RunCommand := RunCommand, SpeedHz := SpeedHz);
    ApplyDrive(Drive := Drive3, RunCommand := RunCommand, SpeedHz := SpeedHz);
ELSE
    Drive1.Stop();
    Drive2.Stop();
    Drive3.Stop();
END_IF;
Update := FaultCountValue;
END_METHOD

METHOD PUBLIC InjectVendorStatus
VAR_INPUT
    AbbStatus : WORD;
    DanfossStatus : WORD;
    SiemensStatus : WORD;
END_VAR
Abb1.ConfigureRaw(StatusWord := AbbStatus, FaultCodeRaw := WORD#16#0011, SpeedRaw := INT#500);
Danfoss1.ConfigureRaw(StatusWord := DanfossStatus, FaultCodeRaw := WORD#16#0022, SpeedRaw := INT#500);
Siemens1.ConfigureRaw(StatusWord := SiemensStatus, FaultCodeRaw := WORD#16#0033, SpeedRaw := INT#500);
END_METHOD

PUBLIC PROPERTY RunningCount : INT
GET
    RunningCount := RunningCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FaultCount : INT
GET
    FaultCount := FaultCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastFaultCode : DWORD
GET
    LastFaultCode := LastFaultCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Cell : MotorCell;
    RunCommand : BOOL;
    PullCordOk : BOOL;
    SpeedSetpointRaw : INT;
    AnyFaultOut : BOOL;
END_VAR

Cell.Initialize();
Cell.Update(RunCommand := RunCommand, PullCordOk := PullCordOk, SpeedHz := INT_TO_REAL(SpeedSetpointRaw) / REAL#10.0);
AnyFaultOut := Cell.FaultCount > INT#0;
END_PROGRAM
