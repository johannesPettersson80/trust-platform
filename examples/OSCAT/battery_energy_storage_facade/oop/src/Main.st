(* Record role: BessSnapshot is a deterministic PLC-side record for tests and runtime readers. *)
TYPE BessSnapshot :
STRUCT
    Ready : BOOL;
    StateOfChargePct : REAL;
    PowerCommandKw : REAL;
    AlarmCount : INT;
    FireIsolated : BOOL;
END_STRUCT
END_TYPE

(* Pattern role: IBessAlarmSubscriber is the contract boundary for the Facade and Observer example. *)
INTERFACE IBessAlarmSubscriber
    METHOD OnAlarm
    VAR_INPUT
        AlarmClass : BYTE;
        AlarmCode : WORD;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: BessHistorianSubscriber is a concrete implementation in the Facade and Observer example. *)
FUNCTION_BLOCK BessHistorianSubscriber IMPLEMENTS IBessAlarmSubscriber
VAR
    AlarmCountValue : INT;
END_VAR
METHOD PUBLIC OnAlarm
VAR_INPUT
    AlarmClass : BYTE;
    AlarmCode : WORD;
END_VAR
AlarmCountValue := AlarmCountValue + INT#1;
END_METHOD
PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := AlarmCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: BessMqttSubscriber is a concrete implementation in the Facade and Observer example. *)
FUNCTION_BLOCK BessMqttSubscriber IMPLEMENTS IBessAlarmSubscriber
VAR
    PublishReadyValue : BOOL;
END_VAR
METHOD PUBLIC OnAlarm
VAR_INPUT
    AlarmClass : BYTE;
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

(* Composition role: BessAlarmBus owns the collaborating objects used by this Facade and Observer example. *)
FUNCTION_BLOCK BessAlarmBus
VAR
    Historian : IBessAlarmSubscriber;
    Mqtt : IBessAlarmSubscriber;
END_VAR
METHOD PUBLIC Subscribe
VAR_INPUT
    HistorianSub : IBessAlarmSubscriber;
    MqttSub : IBessAlarmSubscriber;
END_VAR
Historian := HistorianSub;
Mqtt := MqttSub;
END_METHOD
METHOD PUBLIC Publish
VAR_INPUT
    AlarmClass : BYTE;
    AlarmCode : WORD;
END_VAR
Historian.OnAlarm(AlarmClass := AlarmClass, AlarmCode := AlarmCode);
Mqtt.OnAlarm(AlarmClass := AlarmClass, AlarmCode := AlarmCode);
END_METHOD
END_FUNCTION_BLOCK

(* Composition role: BatteryRack owns the collaborating objects used by this Facade and Observer example. *)
FUNCTION_BLOCK BatteryRack
VAR
    SocValue : REAL;
    TempFilter : Pt1Filter;
END_VAR
METHOD PUBLIC Initialize
TempFilter.Initialize();
TempFilter.Configure(TimeConstant := T#2s, Gain := REAL#1.0);
END_METHOD
METHOD PUBLIC Update
VAR_INPUT
    StateOfChargePct : REAL;
END_VAR
SocValue := LIMIT(REAL#0.0, StateOfChargePct, REAL#100.0);
END_METHOD
PUBLIC PROPERTY StateOfChargePct : REAL
GET
    StateOfChargePct := SocValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: InverterUnit owns the collaborating objects used by this Facade and Observer example. *)
FUNCTION_BLOCK InverterUnit
VAR
    EnabledValue : BOOL;
    PowerCommandValue : REAL;
END_VAR
METHOD PUBLIC Command
VAR_INPUT
    Enabled : BOOL;
    PowerKw : REAL;
END_VAR
EnabledValue := Enabled;
IF Enabled THEN
    PowerCommandValue := LIMIT(REAL#-250.0, PowerKw, REAL#250.0);
ELSE
    PowerCommandValue := REAL#0.0;
END_IF;
END_METHOD
PUBLIC PROPERTY PowerCommandKw : REAL
GET
    PowerCommandKw := PowerCommandValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY Enabled : BOOL
GET
    Enabled := EnabledValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: EnergyStorageCabinet owns the collaborating objects used by this Facade and Observer example. *)
FUNCTION_BLOCK EnergyStorageCabinet
VAR
    Rack : BatteryRack;
    Inverter : InverterUnit;
    AlarmBus : BessAlarmBus;
    Historian : BessHistorianSubscriber;
    Mqtt : BessMqttSubscriber;
    EnabledValue : BOOL;
    SnapshotValue : BessSnapshot;
END_VAR

METHOD PUBLIC Initialize
Rack.Initialize();
AlarmBus.Subscribe(HistorianSub := Historian, MqttSub := Mqtt);
END_METHOD

METHOD PUBLIC Enable
EnabledValue := TRUE;
END_METHOD

METHOD PUBLIC Disable
EnabledValue := FALSE;
Inverter.Command(Enabled := FALSE, PowerKw := REAL#0.0);
END_METHOD

METHOD PUBLIC Update : BessSnapshot
VAR_INPUT
    StateOfChargePct : REAL;
    GridPowerCommandKw : REAL;
    FireInputHealthy : BOOL;
END_VAR
Rack.Update(StateOfChargePct := StateOfChargePct);
SnapshotValue.StateOfChargePct := Rack.StateOfChargePct;
IF NOT FireInputHealthy THEN
    Disable();
    SnapshotValue.FireIsolated := TRUE;
    AlarmBus.Publish(AlarmClass := BYTE#3, AlarmCode := WORD#16#A701);
ELSE
    SnapshotValue.FireIsolated := FALSE;
    Inverter.Command(Enabled := EnabledValue, PowerKw := GridPowerCommandKw);
END_IF;
SnapshotValue.Ready := EnabledValue AND FireInputHealthy;
SnapshotValue.PowerCommandKw := Inverter.PowerCommandKw;
SnapshotValue.AlarmCount := Historian.AlarmCount;
Update := SnapshotValue;
END_METHOD

METHOD PUBLIC Snapshot : BessSnapshot
Snapshot := SnapshotValue;
END_METHOD

PUBLIC PROPERTY StateOfChargePct : REAL
GET
    StateOfChargePct := SnapshotValue.StateOfChargePct;
END_GET
END_PROPERTY
PUBLIC PROPERTY PowerCommandKw : REAL
GET
    PowerCommandKw := SnapshotValue.PowerCommandKw;
END_GET
END_PROPERTY
PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := SnapshotValue.AlarmCount;
END_GET
END_PROPERTY
PUBLIC PROPERTY FireIsolated : BOOL
GET
    FireIsolated := SnapshotValue.FireIsolated;
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
    Cabinet : EnergyStorageCabinet;
    EnableCommand : BOOL;
    FireInputHealthy : BOOL;
    SocRaw : INT;
    GridCommandRaw : INT;
    Snapshot : BessSnapshot;
    InverterEnableOut : BOOL;
    FireIsolateOut : BOOL;
END_VAR
Cabinet.Initialize();
IF EnableCommand THEN
    Cabinet.Enable();
END_IF;
Snapshot := Cabinet.Update(StateOfChargePct := INT_TO_REAL(SocRaw), GridPowerCommandKw := INT_TO_REAL(GridCommandRaw), FireInputHealthy := FireInputHealthy);
InverterEnableOut := Snapshot.Ready;
FireIsolateOut := Snapshot.FireIsolated;
END_PROGRAM
