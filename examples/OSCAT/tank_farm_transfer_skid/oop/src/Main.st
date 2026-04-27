(* Pattern role: IPlantNode is the contract boundary for the Composite and Iterator example. *)
INTERFACE IPlantNode EXTENDS IComponent
    PROPERTY NodeId : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY ChildCount : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY StoppedCount : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY AlarmCount : INT
    GET
    END_GET
    END_PROPERTY

    METHOD GetChild : IPlantNode
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD

    METHOD EmergencyStop
    END_METHOD

    METHOD AcknowledgeAll
    END_METHOD
END_INTERFACE

(* Pattern role: TankNode is a concrete implementation in the Composite and Iterator example. *)
FUNCTION_BLOCK TankNode IMPLEMENTS IPlantNode
VAR
    NodeIdValue : INT;
    StoppedValue : BOOL;
    AlarmValue : BOOL;
    AckCountValue : INT;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    LevelFilter : Pt1Filter;
    HighAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    NodeId : INT;
    HighAlarmActive : BOOL;
END_VAR
NodeIdValue := NodeId;
AlarmValue := HighAlarmActive;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
LevelFilter.Initialize();
LevelFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
AlarmLimits.Low := REAL#5.0;
AlarmLimits.High := REAL#90.0;
HighAlarm.Initialize();
HighAlarm.SetLimits(Limits := AlarmLimits);
ClearError();
END_METHOD

METHOD PUBLIC Reset
StoppedValue := FALSE;
AckCountValue := INT#0;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
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

METHOD PUBLIC UpdateLevel
VAR_INPUT
    LevelPct : REAL;
END_VAR
AlarmValue := HighAlarm.Update(MeasuredValue := LevelFilter.Update(Sample := LevelPct));
END_METHOD

METHOD PUBLIC GetChild : IPlantNode
VAR_INPUT
    Index : INT;
END_VAR
GetChild := THIS;
END_METHOD

METHOD PUBLIC EmergencyStop
StoppedValue := TRUE;
END_METHOD

METHOD PUBLIC AcknowledgeAll
IF AlarmValue THEN
    AckCountValue := AckCountValue + INT#1;
END_IF;
AlarmValue := FALSE;
END_METHOD

PUBLIC PROPERTY NodeId : INT
GET
    NodeId := NodeIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#0;
END_GET
END_PROPERTY

PUBLIC PROPERTY StoppedCount : INT
GET
    IF StoppedValue THEN
        StoppedCount := INT#1;
    ELSE
        StoppedCount := INT#0;
    END_IF;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCount : INT
GET
    IF AlarmValue THEN
        AlarmCount := INT#1;
    ELSE
        AlarmCount := INT#0;
    END_IF;
END_GET
END_PROPERTY

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
END_FUNCTION_BLOCK

(* Pattern role: AreaNode is a concrete implementation in the Composite and Iterator example. *)
FUNCTION_BLOCK AreaNode IMPLEMENTS IPlantNode
VAR
    NodeIdValue : INT;
    Tank1 : TankNode;
    Tank2 : TankNode;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    NodeId : INT;
    Tank1Alarm : BOOL;
    Tank2Alarm : BOOL;
END_VAR
NodeIdValue := NodeId;
Tank1.Configure(NodeId := NodeId * INT#10 + INT#1, HighAlarmActive := Tank1Alarm);
Tank2.Configure(NodeId := NodeId * INT#10 + INT#2, HighAlarmActive := Tank2Alarm);
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Tank1.Initialize();
Tank2.Initialize();
ClearError();
END_METHOD

METHOD PUBLIC Reset
Tank1.Reset();
Tank2.Reset();
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
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

METHOD PUBLIC GetChild : IPlantNode
VAR_INPUT
    Index : INT;
END_VAR
IF Index = INT#1 THEN
    GetChild := Tank1;
ELSE
    GetChild := Tank2;
END_IF;
END_METHOD

METHOD PUBLIC EmergencyStop
Tank1.EmergencyStop();
Tank2.EmergencyStop();
END_METHOD

METHOD PUBLIC AcknowledgeAll
Tank1.AcknowledgeAll();
Tank2.AcknowledgeAll();
END_METHOD

PUBLIC PROPERTY NodeId : INT
GET
    NodeId := NodeIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#2;
END_GET
END_PROPERTY

PUBLIC PROPERTY StoppedCount : INT
GET
    StoppedCount := Tank1.StoppedCount + Tank2.StoppedCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := Tank1.AlarmCount + Tank2.AlarmCount;
END_GET
END_PROPERTY

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
END_FUNCTION_BLOCK

(* Pattern role: FarmNode is a concrete implementation in the Composite and Iterator example. *)
FUNCTION_BLOCK FarmNode IMPLEMENTS IPlantNode
VAR
    Area1 : AreaNode;
    Area2 : AreaNode;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Area1.Initialize();
Area2.Initialize();
Area1.Configure(NodeId := INT#1, Tank1Alarm := TRUE, Tank2Alarm := FALSE);
Area2.Configure(NodeId := INT#2, Tank1Alarm := FALSE, Tank2Alarm := TRUE);
ClearError();
END_METHOD

METHOD PUBLIC Reset
Area1.Reset();
Area2.Reset();
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
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

METHOD PUBLIC GetChild : IPlantNode
VAR_INPUT
    Index : INT;
END_VAR
IF Index = INT#1 THEN
    GetChild := Area1;
ELSE
    GetChild := Area2;
END_IF;
END_METHOD

METHOD PUBLIC EmergencyStop
Area1.EmergencyStop();
Area2.EmergencyStop();
END_METHOD

METHOD PUBLIC AcknowledgeAll
Area1.AcknowledgeAll();
Area2.AcknowledgeAll();
END_METHOD

METHOD PUBLIC EmergencyStopArea
VAR_INPUT
    AreaIndex : INT;
END_VAR
VAR
    Area : IPlantNode;
END_VAR
Area := GetChild(Index := AreaIndex);
Area.EmergencyStop();
END_METHOD

PUBLIC PROPERTY NodeId : INT
GET
    NodeId := INT#100;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#2;
END_GET
END_PROPERTY

PUBLIC PROPERTY StoppedCount : INT
GET
    StoppedCount := Area1.StoppedCount + Area2.StoppedCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := Area1.AlarmCount + Area2.AlarmCount;
END_GET
END_PROPERTY

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
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Farm : FarmNode;
    Area : IPlantNode;
    Tank : IPlantNode;
    AreaStopCommand : BOOL;
    AreaIndex : INT;
    TankIndex : INT;
    TotalTankAlarms : INT;
    AreaStoppedOut : BOOL;
END_VAR

Farm.Initialize();
IF AreaStopCommand THEN
    Farm.EmergencyStopArea(AreaIndex := AreaIndex);
END_IF;
TotalTankAlarms := INT#0;
FOR AreaIndex := INT#1 TO Farm.ChildCount DO
    Area := Farm.GetChild(Index := AreaIndex);
    FOR TankIndex := INT#1 TO Area.ChildCount DO
        Tank := Area.GetChild(Index := TankIndex);
        TotalTankAlarms := TotalTankAlarms + Tank.AlarmCount;
    END_FOR;
END_FOR;
AreaStoppedOut := Farm.StoppedCount > INT#0;
END_PROGRAM
