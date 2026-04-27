(* Record role: BoilerStationStatus is a deterministic PLC-side record for tests and runtime readers. *)
TYPE BoilerStationStatus :
STRUCT
    OverallReady : BOOL;
    LeadBoilerId : INT;
    LeadBoilerLoad : REAL;
    LagBoilerActive : BOOL;
    SupplyTemp : REAL;
    SupplySetpoint : REAL;
    ActiveAlarmCount : INT;
    HighestAlarmClass : BYTE;
    GasMeterReadingM3 : REAL;
END_STRUCT
END_TYPE

(* Pattern role: IBoilerAlarmSubscriber is the contract boundary for the Facade and Observer example. *)
INTERFACE IBoilerAlarmSubscriber
    METHOD OnAlarm
    VAR_INPUT
        AlarmClass : BYTE;
        AlarmCode : WORD;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: BoilerUnit is a concrete implementation in the Facade and Observer example. *)
FUNCTION_BLOCK BoilerUnit IMPLEMENTS IComponent
VAR
    BoilerIdValue : INT;
    RuntimeHoursValue : REAL;
    RunningValue : BOOL;
    LoadValue : REAL;
    FaultValue : BOOL;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    BoilerId : INT;
    RuntimeHours : REAL;
END_VAR
BoilerIdValue := BoilerId;
RuntimeHoursValue := RuntimeHours;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
RunningValue := FALSE;
LoadValue := REAL#0.0;
FaultValue := FALSE;
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

METHOD PUBLIC Fire
VAR_INPUT
    LoadPercent : REAL;
END_VAR
RunningValue := TRUE;
LoadValue := LIMIT(REAL#0.0, LoadPercent, REAL#100.0);
END_METHOD

METHOD PUBLIC Stop
RunningValue := FALSE;
LoadValue := REAL#0.0;
END_METHOD

METHOD PUBLIC SetFault
VAR_INPUT
    Faulted : BOOL;
END_VAR
FaultValue := Faulted;
END_METHOD

PUBLIC PROPERTY BoilerId : INT
GET
    BoilerId := BoilerIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RuntimeHours : REAL
GET
    RuntimeHours := RuntimeHoursValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Running : BOOL
GET
    Running := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Load : REAL
GET
    Load := LoadValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Fault : BOOL
GET
    Fault := FaultValue;
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

(* Pattern role: HistorianAlarmSubscriber is a concrete implementation in the Facade and Observer example. *)
FUNCTION_BLOCK HistorianAlarmSubscriber IMPLEMENTS IBoilerAlarmSubscriber
VAR
    AlarmCountValue : INT;
    HighestClassValue : BYTE;
END_VAR

METHOD PUBLIC OnAlarm
VAR_INPUT
    AlarmClass : BYTE;
    AlarmCode : WORD;
END_VAR
AlarmCountValue := AlarmCountValue + INT#1;
IF AlarmClass > HighestClassValue THEN
    HighestClassValue := AlarmClass;
END_IF;
END_METHOD

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := AlarmCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HighestClass : BYTE
GET
    HighestClass := HighestClassValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: MqttAlarmSubscriber is a concrete implementation in the Facade and Observer example. *)
FUNCTION_BLOCK MqttAlarmSubscriber IMPLEMENTS IBoilerAlarmSubscriber
VAR
    PublishReadyValue : BOOL;
    LastCodeValue : WORD;
END_VAR

METHOD PUBLIC OnAlarm
VAR_INPUT
    AlarmClass : BYTE;
    AlarmCode : WORD;
END_VAR
PublishReadyValue := TRUE;
LastCodeValue := AlarmCode;
END_METHOD

PUBLIC PROPERTY PublishReady : BOOL
GET
    PublishReady := PublishReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCode : WORD
GET
    LastCode := LastCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: BoilerAlarmBus owns the collaborating objects used by this Facade and Observer example. *)
FUNCTION_BLOCK BoilerAlarmBus
VAR
    Subscriber1 : IBoilerAlarmSubscriber;
    Subscriber2 : IBoilerAlarmSubscriber;
    SubscriberCount : INT;
END_VAR

METHOD PUBLIC Subscribe
VAR_INPUT
    Sub : IBoilerAlarmSubscriber;
END_VAR
IF SubscriberCount = INT#0 THEN
    Subscriber1 := Sub;
    SubscriberCount := INT#1;
ELSIF SubscriberCount = INT#1 THEN
    Subscriber2 := Sub;
    SubscriberCount := INT#2;
END_IF;
END_METHOD

METHOD PUBLIC Publish
VAR_INPUT
    AlarmClass : BYTE;
    AlarmCode : WORD;
END_VAR
IF SubscriberCount >= INT#1 THEN
    Subscriber1.OnAlarm(AlarmClass := AlarmClass, AlarmCode := AlarmCode);
END_IF;
IF SubscriberCount >= INT#2 THEN
    Subscriber2.OnAlarm(AlarmClass := AlarmClass, AlarmCode := AlarmCode);
END_IF;
END_METHOD
END_FUNCTION_BLOCK

(* Pattern role: BoilerStation is a concrete implementation in the Facade and Observer example. *)
FUNCTION_BLOCK BoilerStation IMPLEMENTS IComponent
VAR
    Boiler1 : BoilerUnit;
    Boiler2 : BoilerUnit;
    OutdoorFilter : Pt1Filter;
    SupplyFilter : Pt1Filter;
    SupplyDeviationAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
    GasMeter : DwordCounter;
    AlarmBus : BoilerAlarmBus;
    Historian : HistorianAlarmSubscriber;
    MqttPublisher : MqttAlarmSubscriber;
    StatusValue : BoilerStationStatus;
    RunningCommand : BOOL;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    ComponentStatusValue : BYTE;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
Boiler1.Initialize();
Boiler2.Initialize();
Boiler1.Configure(BoilerId := INT#1, RuntimeHours := REAL#4200.0);
Boiler2.Configure(BoilerId := INT#2, RuntimeHours := REAL#3900.0);
OutdoorFilter.Initialize();
OutdoorFilter.Configure(TimeConstant := T#10s, Gain := REAL#1.0);
SupplyFilter.Initialize();
SupplyFilter.Configure(TimeConstant := T#2s, Gain := REAL#1.0);
AlarmLimits.Low := REAL#0.0;
AlarmLimits.High := REAL#15.0;
SupplyDeviationAlarm.Initialize();
SupplyDeviationAlarm.SetLimits(Limits := AlarmLimits);
GasMeter.Initialize();
GasMeter.Configure(StepSize := DWORD#1, MaxValue := DWORD#16#FFFFFFFF);
AlarmBus.Subscribe(Sub := Historian);
AlarmBus.Subscribe(Sub := MqttPublisher);
ClearError();
END_METHOD

METHOD PUBLIC Reset
Boiler1.Reset();
Boiler2.Reset();
RunningCommand := FALSE;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := WORD#0;
ComponentStatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := ComponentStatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC Start
RunningCommand := TRUE;
END_METHOD

METHOD PUBLIC Stop
RunningCommand := FALSE;
Boiler1.Stop();
Boiler2.Stop();
END_METHOD

METHOD PUBLIC Update : BoilerStationStatus
VAR_INPUT
    OutdoorTemp : REAL;
    SupplyTemp : REAL;
    ReturnTemp : REAL;
    GasPressureOk : BOOL;
    LowWaterOk : BOOL;
    EStopHealthy : BOOL;
    HeatDemand : REAL;
END_VAR
VAR
    FilteredOutdoor : REAL;
    FilteredSupply : REAL;
    CalculatedSetpoint : REAL;
    DemandLoad : REAL;
    LeadIsBoiler1 : BOOL;
    Deviation : REAL;
    PermissivesOk : BOOL;
END_VAR
FilteredOutdoor := OutdoorFilter.Update(Sample := OutdoorTemp);
FilteredSupply := SupplyFilter.Update(Sample := SupplyTemp);
CalculatedSetpoint := LIMIT(REAL#45.0, REAL#70.0 - FilteredOutdoor, REAL#85.0);
DemandLoad := LIMIT(REAL#0.0, HeatDemand, REAL#100.0);
StatusValue.SupplyTemp := FilteredSupply;
StatusValue.SupplySetpoint := CalculatedSetpoint;
(* Permissives gate fault publish; OverallReady gates "operationally
   ready" (permissives AND commanded to run). Stop() clears
   RunningCommand so OverallReady drops without raising a fresh fault. *)
PermissivesOk := GasPressureOk AND LowWaterOk AND EStopHealthy;
StatusValue.OverallReady := PermissivesOk AND RunningCommand;

IF NOT PermissivesOk THEN
    Stop();
    AlarmBus.Publish(AlarmClass := BYTE#3, AlarmCode := WORD#16#A101);
ELSE
    LeadIsBoiler1 := Boiler1.RuntimeHours <= Boiler2.RuntimeHours;
    IF RunningCommand THEN
        IF LeadIsBoiler1 THEN
            Boiler1.Fire(LoadPercent := DemandLoad);
            Boiler2.Stop();
            StatusValue.LeadBoilerId := INT#1;
            StatusValue.LeadBoilerLoad := Boiler1.Load;
            IF DemandLoad > REAL#60.0 THEN
                Boiler2.Fire(LoadPercent := DemandLoad - REAL#40.0);
            END_IF;
            StatusValue.LagBoilerActive := Boiler2.Running;
        ELSE
            Boiler2.Fire(LoadPercent := DemandLoad);
            Boiler1.Stop();
            StatusValue.LeadBoilerId := INT#2;
            StatusValue.LeadBoilerLoad := Boiler2.Load;
            IF DemandLoad > REAL#60.0 THEN
                Boiler1.Fire(LoadPercent := DemandLoad - REAL#40.0);
            END_IF;
            StatusValue.LagBoilerActive := Boiler1.Running;
        END_IF;
    ELSE
        Stop();
    END_IF;
END_IF;

Deviation := ABS(CalculatedSetpoint - FilteredSupply);
IF SupplyDeviationAlarm.Update(MeasuredValue := Deviation) THEN
    AlarmBus.Publish(AlarmClass := BYTE#2, AlarmCode := WORD#16#B201);
END_IF;
StatusValue.ActiveAlarmCount := Historian.AlarmCount;
StatusValue.HighestAlarmClass := Historian.HighestClass;
StatusValue.GasMeterReadingM3 := DWORD_TO_REAL(GasMeter.Update(Up := RunningCommand, Down := FALSE));
Update := StatusValue;
END_METHOD

METHOD PUBLIC StationSnapshot : BoilerStationStatus
StationSnapshot := StatusValue;
END_METHOD

PUBLIC PROPERTY LeadBoilerId : INT
GET
    LeadBoilerId := StatusValue.LeadBoilerId;
END_GET
END_PROPERTY

PUBLIC PROPERTY SupplySetpoint : REAL
GET
    SupplySetpoint := StatusValue.SupplySetpoint;
END_GET
END_PROPERTY

PUBLIC PROPERTY ActiveAlarmCount : INT
GET
    ActiveAlarmCount := StatusValue.ActiveAlarmCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY MqttPublishReady : BOOL
GET
    MqttPublishReady := MqttPublisher.PublishReady;
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
    Status := ComponentStatusValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Room : BoilerStation;
    StartCommand : BOOL;
    StopCommand : BOOL;
    GasPressureOk : BOOL;
    LowWaterOk : BOOL;
    EStopHealthy : BOOL;
    OutdoorTempRaw : INT;
    SupplyTempRaw : INT;
    Snapshot : BoilerStationStatus;
    LeadBoilerEnable : BOOL;
    LagBoilerEnable : BOOL;
    AlarmOut : BOOL;
END_VAR

Room.Initialize();
IF StartCommand THEN
    Room.Start();
END_IF;
IF StopCommand THEN
    Room.Stop();
END_IF;
Snapshot := Room.Update(OutdoorTemp := INT_TO_REAL(OutdoorTempRaw), SupplyTemp := INT_TO_REAL(SupplyTempRaw), ReturnTemp := REAL#45.0, GasPressureOk := GasPressureOk, LowWaterOk := LowWaterOk, EStopHealthy := EStopHealthy, HeatDemand := REAL#75.0);
LeadBoilerEnable := Snapshot.LeadBoilerLoad > REAL#0.0;
LagBoilerEnable := Snapshot.LagBoilerActive;
AlarmOut := Snapshot.ActiveAlarmCount > INT#0;
END_PROGRAM
