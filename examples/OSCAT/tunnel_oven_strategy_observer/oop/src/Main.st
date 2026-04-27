(* Pattern role: IOvenProfile is the contract boundary for the Strategy and Observer example. *)
INTERFACE IOvenProfile
    PROPERTY ProfileId : INT
    GET
    END_GET
    END_PROPERTY

    METHOD HeatCommand : REAL
    VAR_INPUT
        ZoneTemp : REAL;
    END_VAR
    END_METHOD

    METHOD ConveyorSpeed : REAL
    END_METHOD
END_INTERFACE

(* Pattern role: IOvenEventSubscriber is the contract boundary for the Strategy and Observer example. *)
INTERFACE IOvenEventSubscriber
    METHOD OnOvenEvent
    VAR_INPUT
        EventClass : BYTE;
        EventCode : WORD;
        ProfileId : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: BreadProfile is a concrete implementation in the Strategy and Observer example. *)
FUNCTION_BLOCK BreadProfile IMPLEMENTS IOvenProfile
VAR
    Filter : Pt1Filter;
END_VAR

METHOD PUBLIC Initialize
Filter.Initialize();
Filter.Configure(TimeConstant := T#3s, Gain := REAL#1.0);
END_METHOD

METHOD PUBLIC HeatCommand : REAL
VAR_INPUT
    ZoneTemp : REAL;
END_VAR
HeatCommand := LIMIT(REAL#0.0, (REAL#185.0 - Filter.Update(Sample := ZoneTemp)) * REAL#1.5, REAL#100.0);
END_METHOD

METHOD PUBLIC ConveyorSpeed : REAL
ConveyorSpeed := REAL#35.0;
END_METHOD

PUBLIC PROPERTY ProfileId : INT
GET
    ProfileId := INT#1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: PizzaProfile is a concrete implementation in the Strategy and Observer example. *)
FUNCTION_BLOCK PizzaProfile IMPLEMENTS IOvenProfile
VAR
    Filter : Pt1Filter;
END_VAR

METHOD PUBLIC Initialize
Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
END_METHOD

METHOD PUBLIC HeatCommand : REAL
VAR_INPUT
    ZoneTemp : REAL;
END_VAR
HeatCommand := LIMIT(REAL#0.0, (REAL#235.0 - Filter.Update(Sample := ZoneTemp)) * REAL#2.0, REAL#100.0);
END_METHOD

METHOD PUBLIC ConveyorSpeed : REAL
ConveyorSpeed := REAL#55.0;
END_METHOD

PUBLIC PROPERTY ProfileId : INT
GET
    ProfileId := INT#2;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: OvenHistorianSubscriber is a concrete implementation in the Strategy and Observer example. *)
FUNCTION_BLOCK OvenHistorianSubscriber IMPLEMENTS IOvenEventSubscriber
VAR
    EventCountValue : INT;
END_VAR

METHOD PUBLIC OnOvenEvent
VAR_INPUT
    EventClass : BYTE;
    EventCode : WORD;
    ProfileId : INT;
END_VAR
EventCountValue := EventCountValue + INT#1;
END_METHOD

PUBLIC PROPERTY EventCount : INT
GET
    EventCount := EventCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: OvenMqttSubscriber is a concrete implementation in the Strategy and Observer example. *)
FUNCTION_BLOCK OvenMqttSubscriber IMPLEMENTS IOvenEventSubscriber
VAR
    PublishReadyValue : BOOL;
END_VAR

METHOD PUBLIC OnOvenEvent
VAR_INPUT
    EventClass : BYTE;
    EventCode : WORD;
    ProfileId : INT;
END_VAR
PublishReadyValue := TRUE;
END_METHOD

PUBLIC PROPERTY PublishReady : BOOL
GET
    PublishReady := PublishReadyValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: OvenEventBus owns the collaborating objects used by this Strategy and Observer example. *)
FUNCTION_BLOCK OvenEventBus
VAR
    Historian : IOvenEventSubscriber;
    Mqtt : IOvenEventSubscriber;
END_VAR

METHOD PUBLIC Subscribe
VAR_INPUT
    HistorianSub : IOvenEventSubscriber;
    MqttSub : IOvenEventSubscriber;
END_VAR
Historian := HistorianSub;
Mqtt := MqttSub;
END_METHOD

METHOD PUBLIC Publish
VAR_INPUT
    EventClass : BYTE;
    EventCode : WORD;
    ProfileId : INT;
END_VAR
Historian.OnOvenEvent(EventClass := EventClass, EventCode := EventCode, ProfileId := ProfileId);
Mqtt.OnOvenEvent(EventClass := EventClass, EventCode := EventCode, ProfileId := ProfileId);
END_METHOD
END_FUNCTION_BLOCK

(* Composition role: TunnelOvenController owns the collaborating objects used by this Strategy and Observer example. *)
FUNCTION_BLOCK TunnelOvenController
VAR
    Bread : BreadProfile;
    Pizza : PizzaProfile;
    ActiveProfile : IOvenProfile;
    Bus : OvenEventBus;
    Historian : OvenHistorianSubscriber;
    Mqtt : OvenMqttSubscriber;
    ActiveProfileIdValue : INT;
    ZoneHeatCommandValue : REAL;
    ConveyorSpeedValue : REAL;
END_VAR

METHOD PUBLIC Initialize
Bread.Initialize();
Pizza.Initialize();
Bus.Subscribe(HistorianSub := Historian, MqttSub := Mqtt);
ActiveProfile := Bread;
END_METHOD

METHOD PUBLIC SelectProfile
VAR_INPUT
    ProductId : INT;
END_VAR
IF ProductId = INT#2 THEN
    ActiveProfile := Pizza;
ELSE
    ActiveProfile := Bread;
END_IF;
ActiveProfileIdValue := ActiveProfile.ProfileId;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    ProductId : INT;
    ZoneTemp : REAL;
    Overtemp : BOOL;
END_VAR
SelectProfile(ProductId := ProductId);
ZoneHeatCommandValue := ActiveProfile.HeatCommand(ZoneTemp := ZoneTemp);
ConveyorSpeedValue := ActiveProfile.ConveyorSpeed();
IF Overtemp THEN
    ZoneHeatCommandValue := REAL#0.0;
    Bus.Publish(EventClass := BYTE#3, EventCode := WORD#16#A601, ProfileId := ActiveProfile.ProfileId);
END_IF;
Update := ZoneHeatCommandValue;
END_METHOD

PUBLIC PROPERTY ActiveProfileId : INT
GET
    ActiveProfileId := ActiveProfileIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ZoneHeatCommand : REAL
GET
    ZoneHeatCommand := ZoneHeatCommandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ConveyorSpeed : REAL
GET
    ConveyorSpeed := ConveyorSpeedValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY QualityEventCount : INT
GET
    QualityEventCount := Historian.EventCount;
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
    Oven : TunnelOvenController;
    ProductId : INT;
    ZoneTempRaw : INT;
    OvertempInput : BOOL;
    HeatCommandRaw : INT;
    ConveyorSpeedRaw : INT;
    AlarmOut : BOOL;
END_VAR

Oven.Initialize();
HeatCommandRaw := REAL_TO_INT(Oven.Update(ProductId := ProductId, ZoneTemp := INT_TO_REAL(ZoneTempRaw), Overtemp := OvertempInput));
ConveyorSpeedRaw := REAL_TO_INT(Oven.ConveyorSpeed);
AlarmOut := Oven.QualityEventCount > INT#0;
END_PROGRAM
