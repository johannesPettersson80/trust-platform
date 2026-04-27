(* Pattern role: IBaggageCommand is the contract boundary for the Command and Observer example. *)
INTERFACE IBaggageCommand
    PROPERTY CommandId : INT
    GET
    END_GET
    END_PROPERTY
    METHOD Execute : BOOL
    VAR_INPUT
        BagId : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: IBaggageEventSubscriber is the contract boundary for the Command and Observer example. *)
INTERFACE IBaggageEventSubscriber
    METHOD OnBaggageEvent
    VAR_INPUT
        BagId : INT;
        CommandId : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: DivertLeftCommand is a concrete implementation in the Command and Observer example. *)
FUNCTION_BLOCK DivertLeftCommand IMPLEMENTS IBaggageCommand
METHOD PUBLIC Execute : BOOL
VAR_INPUT
    BagId : INT;
END_VAR
Execute := TRUE;
END_METHOD
PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: DivertRightCommand is a concrete implementation in the Command and Observer example. *)
FUNCTION_BLOCK DivertRightCommand IMPLEMENTS IBaggageCommand
METHOD PUBLIC Execute : BOOL
VAR_INPUT
    BagId : INT;
END_VAR
Execute := TRUE;
END_METHOD
PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#2;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RejectBagCommand is a concrete implementation in the Command and Observer example. *)
FUNCTION_BLOCK RejectBagCommand IMPLEMENTS IBaggageCommand
METHOD PUBLIC Execute : BOOL
VAR_INPUT
    BagId : INT;
END_VAR
Execute := TRUE;
END_METHOD
PUBLIC PROPERTY CommandId : INT
GET
    CommandId := INT#3;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: BaggageHistorianSubscriber is a concrete implementation in the Command and Observer example. *)
FUNCTION_BLOCK BaggageHistorianSubscriber IMPLEMENTS IBaggageEventSubscriber
VAR
    EventCountValue : INT;
END_VAR
METHOD PUBLIC OnBaggageEvent
VAR_INPUT
    BagId : INT;
    CommandId : INT;
END_VAR
EventCountValue := EventCountValue + INT#1;
END_METHOD
PUBLIC PROPERTY EventCount : INT
GET
    EventCount := EventCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: BaggageMqttSubscriber is a concrete implementation in the Command and Observer example. *)
FUNCTION_BLOCK BaggageMqttSubscriber IMPLEMENTS IBaggageEventSubscriber
VAR
    PublishReadyValue : BOOL;
END_VAR
METHOD PUBLIC OnBaggageEvent
VAR_INPUT
    BagId : INT;
    CommandId : INT;
END_VAR
PublishReadyValue := TRUE;
END_METHOD
PUBLIC PROPERTY PublishReady : BOOL
GET
    PublishReady := PublishReadyValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: BaggageEventBus owns the collaborating objects used by this Command and Observer example. *)
FUNCTION_BLOCK BaggageEventBus
VAR
    Historian : IBaggageEventSubscriber;
    Mqtt : IBaggageEventSubscriber;
END_VAR
METHOD PUBLIC Subscribe
VAR_INPUT
    HistorianSub : IBaggageEventSubscriber;
    MqttSub : IBaggageEventSubscriber;
END_VAR
Historian := HistorianSub;
Mqtt := MqttSub;
END_METHOD
METHOD PUBLIC Publish
VAR_INPUT
    BagId : INT;
    CommandId : INT;
END_VAR
Historian.OnBaggageEvent(BagId := BagId, CommandId := CommandId);
Mqtt.OnBaggageEvent(BagId := BagId, CommandId := CommandId);
END_METHOD
END_FUNCTION_BLOCK

(* Composition role: BaggageDiverter owns the collaborating objects used by this Command and Observer example. *)
FUNCTION_BLOCK BaggageDiverter
VAR
    Left : DivertLeftCommand;
    Right : DivertRightCommand;
    Reject : RejectBagCommand;
    CurrentCommand : IBaggageCommand;
    Bus : BaggageEventBus;
    Historian : BaggageHistorianSubscriber;
    Mqtt : BaggageMqttSubscriber;
    LastBagIdValue : INT;
    LastCommandIdValue : INT;
END_VAR
METHOD PUBLIC Initialize
Bus.Subscribe(HistorianSub := Historian, MqttSub := Mqtt);
END_METHOD
METHOD PUBLIC SelectCommand : IBaggageCommand
VAR_INPUT
    ScannerResult : INT;
END_VAR
IF ScannerResult = INT#1 THEN
    SelectCommand := Left;
ELSIF ScannerResult = INT#2 THEN
    SelectCommand := Right;
ELSE
    SelectCommand := Reject;
END_IF;
END_METHOD
METHOD PUBLIC ProcessBag : INT
VAR_INPUT
    BagId : INT;
    ScannerResult : INT;
END_VAR
CurrentCommand := SelectCommand(ScannerResult := ScannerResult);
IF CurrentCommand.Execute(BagId := BagId) THEN
    LastBagIdValue := BagId;
    LastCommandIdValue := CurrentCommand.CommandId;
    Bus.Publish(BagId := BagId, CommandId := LastCommandIdValue);
END_IF;
ProcessBag := LastCommandIdValue;
END_METHOD
PUBLIC PROPERTY LastBagId : INT
GET
    LastBagId := LastBagIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY LastCommandId : INT
GET
    LastCommandId := LastCommandIdValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY EventCount : INT
GET
    EventCount := Historian.EventCount;
END_GET
END_PROPERTY
PUBLIC PROPERTY MqttPublishReady : BOOL
GET
    MqttPublishReady := Mqtt.PublishReady;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Diverter : BaggageDiverter;
    BagIdRaw : INT;
    ScannerResultRaw : INT;
    LeftOut : BOOL;
    RightOut : BOOL;
    RejectOut : BOOL;
END_VAR
Diverter.Initialize();
Diverter.ProcessBag(BagId := BagIdRaw, ScannerResult := ScannerResultRaw);
LeftOut := Diverter.LastCommandId = INT#1;
RightOut := Diverter.LastCommandId = INT#2;
RejectOut := Diverter.LastCommandId = INT#3;
END_PROGRAM
