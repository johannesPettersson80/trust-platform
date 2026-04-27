(* Pattern role: IColdNode is the contract boundary for the Composite, Observer, and Mediator example. *)
INTERFACE IColdNode
    PROPERTY ChildCount : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY DemandWeight : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY AlarmCount : INT
    GET
    END_GET
    END_PROPERTY

    METHOD GetChild : IColdNode
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: IColdAlarmSubscriber is the contract boundary for the Composite, Observer, and Mediator example. *)
INTERFACE IColdAlarmSubscriber
    METHOD OnColdAlarm
    VAR_INPUT
        AlarmClass : BYTE;
        RoomId : INT;
        AlarmCode : WORD;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: RoomNode is a concrete implementation in the Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK RoomNode IMPLEMENTS IColdNode
VAR
    RoomIdValue : INT;
    SetpointValue : REAL;
    DemandValue : BOOL;
    AlarmValue : BOOL;
    TempFilter : Pt1Filter;
    DoorCounter : DwordCounter;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    RoomId : INT;
    SetpointC : REAL;
END_VAR
RoomIdValue := RoomId;
SetpointValue := SetpointC;
TempFilter.Initialize();
TempFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
DoorCounter.Initialize();
DoorCounter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
END_METHOD

METHOD PUBLIC UpdateRoom
VAR_INPUT
    TempC : REAL;
    DoorOpen : BOOL;
END_VAR
VAR
    FilteredTemp : REAL;
END_VAR
FilteredTemp := TempFilter.Update(Sample := TempC);
DemandValue := (FilteredTemp > (SetpointValue + REAL#1.0)) AND NOT DoorOpen;
(* Either fault path is an alarm: an open door is a maintenance event
   on its own (energy loss), and an over-temperature is its own alarm.
   The product-temp check uses a +5 C deviation guard. *)
AlarmValue := DoorOpen OR (FilteredTemp > (SetpointValue + REAL#5.0));
IF DoorOpen THEN
    DoorCounter.Update(Up := TRUE, Down := FALSE);
END_IF;
END_METHOD

METHOD PUBLIC GetChild : IColdNode
VAR_INPUT
    Index : INT;
END_VAR
GetChild := THIS;
END_METHOD

PUBLIC PROPERTY RoomId : INT
GET
    RoomId := RoomIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#0;
END_GET
END_PROPERTY

PUBLIC PROPERTY DemandWeight : INT
GET
    IF DemandValue THEN
        DemandWeight := INT#1;
    ELSE
        DemandWeight := INT#0;
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
END_FUNCTION_BLOCK

(* Pattern role: RoomCluster is a concrete implementation in the Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK RoomCluster IMPLEMENTS IColdNode
VAR
    Room1 : RoomNode;
    Room2 : RoomNode;
    ClusterWeight : INT;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    BaseRoomId : INT;
    SetpointC : REAL;
    PriorityWeight : INT;
END_VAR
ClusterWeight := PriorityWeight;
Room1.Configure(RoomId := BaseRoomId + INT#1, SetpointC := SetpointC);
Room2.Configure(RoomId := BaseRoomId + INT#2, SetpointC := SetpointC);
END_METHOD

METHOD PUBLIC UpdateCluster
VAR_INPUT
    Temp1 : REAL;
    Door1 : BOOL;
    Temp2 : REAL;
    Door2 : BOOL;
END_VAR
Room1.UpdateRoom(TempC := Temp1, DoorOpen := Door1);
Room2.UpdateRoom(TempC := Temp2, DoorOpen := Door2);
END_METHOD

METHOD PUBLIC GetChild : IColdNode
VAR_INPUT
    Index : INT;
END_VAR
IF Index = INT#1 THEN
    GetChild := Room1;
ELSE
    GetChild := Room2;
END_IF;
END_METHOD

PUBLIC PROPERTY ChildCount : INT
GET
    ChildCount := INT#2;
END_GET
END_PROPERTY

PUBLIC PROPERTY DemandWeight : INT
GET
    DemandWeight := (Room1.DemandWeight + Room2.DemandWeight) * ClusterWeight;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := Room1.AlarmCount + Room2.AlarmCount;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: EnergyLossSubscriber is a concrete implementation in the Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK EnergyLossSubscriber IMPLEMENTS IColdAlarmSubscriber
VAR
    DoorAlarmCountValue : INT;
END_VAR

METHOD PUBLIC OnColdAlarm
VAR_INPUT
    AlarmClass : BYTE;
    RoomId : INT;
    AlarmCode : WORD;
END_VAR
DoorAlarmCountValue := DoorAlarmCountValue + INT#1;
END_METHOD

PUBLIC PROPERTY DoorAlarmCount : INT
GET
    DoorAlarmCount := DoorAlarmCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: MaintenanceStackSubscriber is a concrete implementation in the Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK MaintenanceStackSubscriber IMPLEMENTS IColdAlarmSubscriber
VAR
    WorkStack : DwordStack32;
    WorkItemCountValue : INT;
END_VAR

METHOD PUBLIC Initialize
WorkStack.Initialize();
END_METHOD

METHOD PUBLIC OnColdAlarm
VAR_INPUT
    AlarmClass : BYTE;
    RoomId : INT;
    AlarmCode : WORD;
END_VAR
WorkStack.Push(Value := INT_TO_DWORD(RoomId));
WorkItemCountValue := WorkItemCountValue + INT#1;
END_METHOD

PUBLIC PROPERTY WorkItemCount : INT
GET
    WorkItemCount := WorkItemCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: MqttColdAlarmSubscriber is a concrete implementation in the Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK MqttColdAlarmSubscriber IMPLEMENTS IColdAlarmSubscriber
VAR
    PublishReadyValue : BOOL;
END_VAR

METHOD PUBLIC OnColdAlarm
VAR_INPUT
    AlarmClass : BYTE;
    RoomId : INT;
    AlarmCode : WORD;
END_VAR
PublishReadyValue := TRUE;
END_METHOD

PUBLIC PROPERTY PublishReady : BOOL
GET
    PublishReady := PublishReadyValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: ColdAlarmBus owns the collaborating objects used by this Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK ColdAlarmBus
VAR
    Energy : IColdAlarmSubscriber;
    Maintenance : IColdAlarmSubscriber;
    Mqtt : IColdAlarmSubscriber;
END_VAR

METHOD PUBLIC Subscribe
VAR_INPUT
    EnergySub : IColdAlarmSubscriber;
    MaintenanceSub : IColdAlarmSubscriber;
    MqttSub : IColdAlarmSubscriber;
END_VAR
Energy := EnergySub;
Maintenance := MaintenanceSub;
Mqtt := MqttSub;
END_METHOD

METHOD PUBLIC Publish
VAR_INPUT
    AlarmClass : BYTE;
    RoomId : INT;
    AlarmCode : WORD;
END_VAR
Energy.OnColdAlarm(AlarmClass := AlarmClass, RoomId := RoomId, AlarmCode := AlarmCode);
Maintenance.OnColdAlarm(AlarmClass := AlarmClass, RoomId := RoomId, AlarmCode := AlarmCode);
Mqtt.OnColdAlarm(AlarmClass := AlarmClass, RoomId := RoomId, AlarmCode := AlarmCode);
END_METHOD
END_FUNCTION_BLOCK

(* Composition role: CompressorRackMediator owns the collaborating objects used by this Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK CompressorRackMediator
VAR
    RunningCompressorCountValue : INT;
    FreezerDemandValue : INT;
    ChillerDemandValue : INT;
END_VAR

METHOD PUBLIC Allocate : INT
VAR_INPUT
    FreezerDemand : INT;
    ChillerDemand : INT;
    Compressor1Healthy : BOOL;
    Compressor2Healthy : BOOL;
    Compressor3Healthy : BOOL;
END_VAR
FreezerDemandValue := FreezerDemand;
ChillerDemandValue := ChillerDemand;
RunningCompressorCountValue := INT#0;
IF (FreezerDemand > INT#0) AND Compressor1Healthy THEN
    RunningCompressorCountValue := RunningCompressorCountValue + INT#1;
END_IF;
IF ((FreezerDemand > INT#2) OR (ChillerDemand > INT#0)) AND Compressor2Healthy THEN
    RunningCompressorCountValue := RunningCompressorCountValue + INT#1;
END_IF;
IF ((FreezerDemand + ChillerDemand) > INT#5) AND Compressor3Healthy THEN
    RunningCompressorCountValue := RunningCompressorCountValue + INT#1;
END_IF;
Allocate := RunningCompressorCountValue;
END_METHOD

PUBLIC PROPERTY RunningCompressorCount : INT
GET
    RunningCompressorCount := RunningCompressorCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: ColdStorePlant owns the collaborating objects used by this Composite, Observer, and Mediator example. *)
FUNCTION_BLOCK ColdStorePlant
VAR
    Freezers : RoomCluster;
    Chillers : RoomCluster;
    Rack : CompressorRackMediator;
    AlarmBus : ColdAlarmBus;
    EnergySubscriber : EnergyLossSubscriber;
    MaintenanceSubscriber : MaintenanceStackSubscriber;
    MqttSubscriber : MqttColdAlarmSubscriber;
    RunningCompressorCountValue : INT;
END_VAR

METHOD PUBLIC Initialize
Freezers.Configure(BaseRoomId := INT#100, SetpointC := REAL#-25.0, PriorityWeight := INT#2);
Chillers.Configure(BaseRoomId := INT#200, SetpointC := REAL#2.0, PriorityWeight := INT#1);
MaintenanceSubscriber.Initialize();
AlarmBus.Subscribe(EnergySub := EnergySubscriber, MaintenanceSub := MaintenanceSubscriber, MqttSub := MqttSubscriber);
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    Freezer1Temp : REAL;
    Freezer1Door : BOOL;
    Freezer2Temp : REAL;
    Freezer2Door : BOOL;
    Chiller1Temp : REAL;
    Chiller1Door : BOOL;
    Chiller2Temp : REAL;
    Chiller2Door : BOOL;
END_VAR
Freezers.UpdateCluster(Temp1 := Freezer1Temp, Door1 := Freezer1Door, Temp2 := Freezer2Temp, Door2 := Freezer2Door);
Chillers.UpdateCluster(Temp1 := Chiller1Temp, Door1 := Chiller1Door, Temp2 := Chiller2Temp, Door2 := Chiller2Door);
IF Freezers.AlarmCount > INT#0 THEN
    AlarmBus.Publish(AlarmClass := BYTE#3, RoomId := INT#100, AlarmCode := WORD#16#A501);
END_IF;
IF Chillers.AlarmCount > INT#0 THEN
    AlarmBus.Publish(AlarmClass := BYTE#2, RoomId := INT#200, AlarmCode := WORD#16#B501);
END_IF;
RunningCompressorCountValue := Rack.Allocate(FreezerDemand := Freezers.DemandWeight, ChillerDemand := Chillers.DemandWeight, Compressor1Healthy := TRUE, Compressor2Healthy := TRUE, Compressor3Healthy := TRUE);
Update := RunningCompressorCountValue;
END_METHOD

PUBLIC PROPERTY RunningCompressorCount : INT
GET
    RunningCompressorCount := RunningCompressorCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY FreezerDemand : INT
GET
    FreezerDemand := Freezers.DemandWeight;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChillerDemand : INT
GET
    ChillerDemand := Chillers.DemandWeight;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := Freezers.AlarmCount + Chillers.AlarmCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY MaintenanceWorkItems : INT
GET
    MaintenanceWorkItems := MaintenanceSubscriber.WorkItemCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY MqttPublishReady : BOOL
GET
    MqttPublishReady := MqttSubscriber.PublishReady;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Plant : ColdStorePlant;
    Freezer1TempRaw : INT;
    Freezer2TempRaw : INT;
    Chiller1TempRaw : INT;
    Chiller2TempRaw : INT;
    Freezer1Door : BOOL;
    Freezer2Door : BOOL;
    Chiller1Door : BOOL;
    Chiller2Door : BOOL;
    Compressor1Out : BOOL;
    Compressor2Out : BOOL;
    Compressor3Out : BOOL;
    AlarmOut : BOOL;
END_VAR

Plant.Initialize();
Plant.Update(Freezer1Temp := INT_TO_REAL(Freezer1TempRaw), Freezer1Door := Freezer1Door, Freezer2Temp := INT_TO_REAL(Freezer2TempRaw), Freezer2Door := Freezer2Door, Chiller1Temp := INT_TO_REAL(Chiller1TempRaw), Chiller1Door := Chiller1Door, Chiller2Temp := INT_TO_REAL(Chiller2TempRaw), Chiller2Door := Chiller2Door);
Compressor1Out := Plant.RunningCompressorCount >= INT#1;
Compressor2Out := Plant.RunningCompressorCount >= INT#2;
Compressor3Out := Plant.RunningCompressorCount >= INT#3;
AlarmOut := Plant.AlarmCount > INT#0;
END_PROGRAM
