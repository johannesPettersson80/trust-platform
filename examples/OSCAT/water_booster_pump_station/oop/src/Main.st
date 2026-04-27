(* Pattern role: IPump is the contract boundary for the Mediator and Observer example. *)
INTERFACE IPump EXTENDS IComponent
    PROPERTY PumpId : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY IsHealthy : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY IsRunning : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY RuntimeHours : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY SpeedReference : REAL
    GET
    END_GET
    END_PROPERTY

    METHOD Run
    VAR_INPUT
        SpeedReference : REAL;
    END_VAR
    END_METHOD

    METHOD Stop
    END_METHOD
END_INTERFACE

(* Pattern role: IAlarmSubscriber is the contract boundary for the Mediator and Observer example. *)
INTERFACE IAlarmSubscriber
    METHOD OnAlarm
    VAR_INPUT
        ClassId : BYTE;
        Code : DWORD;
        SourceId : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: PumpDrive is a concrete implementation in the Mediator and Observer example. *)
FUNCTION_BLOCK PumpDrive IMPLEMENTS IPump
VAR
    IdValue : INT;
    FaultValue : BOOL;
    RemoteValue : BOOL := TRUE;
    RunningValue : BOOL;
    RuntimeValue : REAL;
    SpeedValue : REAL;
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    RuntimeMeter : OntimeMeter;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    PumpId : INT;
    RuntimeHoursInitial : REAL;
    Faulted : BOOL;
END_VAR
IdValue := PumpId;
RuntimeValue := RuntimeHoursInitial;
FaultValue := Faulted;
END_METHOD

METHOD PUBLIC Initialize
ReadyValue := TRUE;
RuntimeMeter.Initialize();
RuntimeMeter.Reset();
ClearError();
END_METHOD

METHOD PUBLIC Reset
RunningValue := FALSE;
SpeedValue := REAL#0.0;
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

METHOD PUBLIC Run
VAR_INPUT
    SpeedReference : REAL;
END_VAR
IF NOT FaultValue AND RemoteValue THEN
    RunningValue := TRUE;
    SpeedValue := LIMIT(REAL#0.0, SpeedReference, REAL#100.0);
ELSE
    RunningValue := FALSE;
    SpeedValue := REAL#0.0;
END_IF;
RuntimeMeter.Update(Input := RunningValue);
END_METHOD

METHOD PUBLIC Stop
RunningValue := FALSE;
SpeedValue := REAL#0.0;
RuntimeMeter.Update(Input := FALSE);
END_METHOD

PUBLIC PROPERTY PumpId : INT
GET
    PumpId := IdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsHealthy : BOOL
GET
    IsHealthy := NOT FaultValue AND RemoteValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsRunning : BOOL
GET
    IsRunning := RunningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RuntimeHours : REAL
GET
    RuntimeHours := RuntimeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY SpeedReference : REAL
GET
    SpeedReference := SpeedValue;
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

(* Pattern role: HistorianAlarmSubscriber is a concrete implementation in the Mediator and Observer example. *)
FUNCTION_BLOCK HistorianAlarmSubscriber IMPLEMENTS IAlarmSubscriber
VAR
    LastCodeValue : DWORD;
    CountValue : INT;
    Journal : DwordFifo32;
END_VAR

METHOD PUBLIC Initialize
Journal.Initialize();
Journal.Reset();
LastCodeValue := DWORD#0;
CountValue := INT#0;
END_METHOD

METHOD PUBLIC OnAlarm
VAR_INPUT
    ClassId : BYTE;
    Code : DWORD;
    SourceId : INT;
END_VAR
LastCodeValue := Code;
CountValue := CountValue + INT#1;
IF Journal.Push(Value := Code) THEN
END_IF;
END_METHOD

PUBLIC PROPERTY Count : INT
GET
    Count := CountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCode : DWORD
GET
    LastCode := LastCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: MqttAlarmSubscriber is a concrete implementation in the Mediator and Observer example. *)
FUNCTION_BLOCK MqttAlarmSubscriber IMPLEMENTS IAlarmSubscriber
VAR
    PublishReadyValue : BOOL;
    LastCodeValue : DWORD;
END_VAR

METHOD PUBLIC OnAlarm
VAR_INPUT
    ClassId : BYTE;
    Code : DWORD;
    SourceId : INT;
END_VAR
PublishReadyValue := TRUE;
LastCodeValue := Code;
END_METHOD

PUBLIC PROPERTY PublishReady : BOOL
GET
    PublishReady := PublishReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastCode : DWORD
GET
    LastCode := LastCodeValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: AlarmBus owns the collaborating objects used by this Mediator and Observer example. *)
FUNCTION_BLOCK AlarmBus
VAR
    Historian : HistorianAlarmSubscriber;
    Mqtt : MqttAlarmSubscriber;
END_VAR

METHOD PUBLIC Initialize
Historian.Initialize();
END_METHOD

METHOD PUBLIC Publish
VAR_INPUT
    ClassId : BYTE;
    Code : DWORD;
    SourceId : INT;
END_VAR
Historian.OnAlarm(ClassId := ClassId, Code := Code, SourceId := SourceId);
Mqtt.OnAlarm(ClassId := ClassId, Code := Code, SourceId := SourceId);
END_METHOD

PUBLIC PROPERTY HistorianCount : INT
GET
    HistorianCount := Historian.Count;
END_GET
END_PROPERTY

PUBLIC PROPERTY MqttPublishReady : BOOL
GET
    MqttPublishReady := Mqtt.PublishReady;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: LeadLagMediator owns the collaborating objects used by this Mediator and Observer example. *)
FUNCTION_BLOCK LeadLagMediator
VAR
    LeadPumpIdValue : INT;
    RunningPumpCountValue : INT;
END_VAR

METHOD PUBLIC Update
VAR_IN_OUT
    Pump1 : PumpDrive;
    Pump2 : PumpDrive;
    Pump3 : PumpDrive;
END_VAR
VAR_INPUT
    DemandLevel : INT;
    SpeedReference : REAL;
END_VAR
VAR
    Lead : IPump;
END_VAR
Pump1.Stop();
Pump2.Stop();
Pump3.Stop();
RunningPumpCountValue := INT#0;

(* Mediator decision: choose the healthy pump with the lowest runtime as lead for wear leveling. *)
IF Pump1.IsHealthy AND (NOT Pump2.IsHealthy OR Pump1.RuntimeHours <= Pump2.RuntimeHours) AND (NOT Pump3.IsHealthy OR Pump1.RuntimeHours <= Pump3.RuntimeHours) THEN
    Lead := Pump1;
ELSIF Pump2.IsHealthy AND (NOT Pump3.IsHealthy OR Pump2.RuntimeHours <= Pump3.RuntimeHours) THEN
    Lead := Pump2;
ELSIF Pump3.IsHealthy THEN
    Lead := Pump3;
ELSE
    LeadPumpIdValue := INT#0;
    RETURN;
END_IF;

LeadPumpIdValue := Lead.PumpId;
Lead.Run(SpeedReference := SpeedReference);
RunningPumpCountValue := INT#1;

IF DemandLevel >= INT#2 THEN
    IF Pump1.IsHealthy AND Pump1.PumpId <> LeadPumpIdValue THEN
        Pump1.Run(SpeedReference := REAL#60.0);
        RunningPumpCountValue := RunningPumpCountValue + INT#1;
    ELSIF Pump2.IsHealthy AND Pump2.PumpId <> LeadPumpIdValue THEN
        Pump2.Run(SpeedReference := REAL#60.0);
        RunningPumpCountValue := RunningPumpCountValue + INT#1;
    ELSIF Pump3.IsHealthy AND Pump3.PumpId <> LeadPumpIdValue THEN
        Pump3.Run(SpeedReference := REAL#60.0);
        RunningPumpCountValue := RunningPumpCountValue + INT#1;
    END_IF;
END_IF;
END_METHOD

PUBLIC PROPERTY LeadPumpId : INT
GET
    LeadPumpId := LeadPumpIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RunningPumpCount : INT
GET
    RunningPumpCount := RunningPumpCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: BoosterStation owns the collaborating objects used by this Mediator and Observer example. *)
FUNCTION_BLOCK BoosterStation
VAR
    Pump1 : PumpDrive;
    Pump2 : PumpDrive;
    Pump3 : PumpDrive;
    Mediator : LeadLagMediator;
    AlarmBusValue : AlarmBus;
    PressureFilter : Pt1Filter;
    Controller : PidController;
    Limits : RealRange;
    Gains : PidGains;
    LeadPumpIdValue : INT;
    RunningPumpCountValue : INT;
    AlarmActiveValue : BOOL;
    CommandValue : REAL;
END_VAR

METHOD PUBLIC Initialize
Pump1.Initialize();
Pump2.Initialize();
Pump3.Initialize();
Pump1.Configure(PumpId := INT#1, RuntimeHoursInitial := REAL#120.0, Faulted := FALSE);
Pump2.Configure(PumpId := INT#2, RuntimeHoursInitial := REAL#20.0, Faulted := FALSE);
Pump3.Configure(PumpId := INT#3, RuntimeHoursInitial := REAL#60.0, Faulted := FALSE);
AlarmBusValue.Initialize();
PressureFilter.Initialize();
PressureFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#15.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
Controller.Initialize();
Controller.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
END_METHOD

METHOD PUBLIC SetPumpFaults
VAR_INPUT
    Fault1 : BOOL;
    Fault2 : BOOL;
    Fault3 : BOOL;
END_VAR
Pump1.Configure(PumpId := INT#1, RuntimeHoursInitial := Pump1.RuntimeHours, Faulted := Fault1);
Pump2.Configure(PumpId := INT#2, RuntimeHoursInitial := Pump2.RuntimeHours, Faulted := Fault2);
Pump3.Configure(PumpId := INT#3, RuntimeHoursInitial := Pump3.RuntimeHours, Faulted := Fault3);
END_METHOD

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    SuctionOk : BOOL;
    HeaderPressure : REAL;
    PressureSetpoint : REAL;
END_VAR
VAR
    FilteredPressure : REAL;
    DemandLevel : INT;
END_VAR
IF NOT SuctionOk THEN
    AlarmActiveValue := TRUE;
    AlarmBusValue.Publish(ClassId := BYTE#1, Code := DWORD#16#A201, SourceId := INT#0);
    Pump1.Stop();
    Pump2.Stop();
    Pump3.Stop();
    (* Suction loss latches lead/running state to zero — pumps are
       commanded stopped, count must reflect that. *)
    RunningPumpCountValue := INT#0;
    LeadPumpIdValue := INT#0;
ELSIF StartCommand THEN
    FilteredPressure := PressureFilter.Update(Sample := HeaderPressure);
    CommandValue := Controller.Update(Actual := FilteredPressure, Target := PressureSetpoint);
    IF PressureSetpoint - FilteredPressure > REAL#1.0 THEN
        DemandLevel := INT#2;
    ELSE
        DemandLevel := INT#1;
    END_IF;
    Mediator.Update(Pump1 := Pump1, Pump2 := Pump2, Pump3 := Pump3, DemandLevel := DemandLevel, SpeedReference := CommandValue);
    LeadPumpIdValue := Mediator.LeadPumpId;
    RunningPumpCountValue := Mediator.RunningPumpCount;
    IF LeadPumpIdValue = INT#0 THEN
        AlarmActiveValue := TRUE;
        AlarmBusValue.Publish(ClassId := BYTE#1, Code := DWORD#16#A202, SourceId := INT#0);
    END_IF;
ELSE
    Pump1.Stop();
    Pump2.Stop();
    Pump3.Stop();
    RunningPumpCountValue := INT#0;
END_IF;
Update := LeadPumpIdValue;
END_METHOD

PUBLIC PROPERTY LeadPumpId : INT
GET
    LeadPumpId := LeadPumpIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RunningPumpCount : INT
GET
    RunningPumpCount := RunningPumpCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmActive : BOOL
GET
    AlarmActive := AlarmActiveValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HistorianAlarmCount : INT
GET
    HistorianAlarmCount := AlarmBusValue.HistorianCount;
END_GET
END_PROPERTY

PUBLIC PROPERTY MqttPublishReady : BOOL
GET
    MqttPublishReady := AlarmBusValue.MqttPublishReady;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Station : BoosterStation;
    StartCommand : BOOL;
    SuctionOk : BOOL;
    Pump1Fault : BOOL;
    Pump2Fault : BOOL;
    Pump3Fault : BOOL;
    HeaderPressureRaw : INT;
    Pump1Enable : BOOL;
    Pump2Enable : BOOL;
    Pump3Enable : BOOL;
    AlarmOut : BOOL;
    HeaderPressure : REAL;
    LeadId : INT;
END_VAR

HeaderPressure := TO_REAL(HeaderPressureRaw) / REAL#100.0;
Station.Initialize();
Station.SetPumpFaults(Fault1 := Pump1Fault, Fault2 := Pump2Fault, Fault3 := Pump3Fault);
LeadId := Station.Update(StartCommand := StartCommand, SuctionOk := SuctionOk, HeaderPressure := HeaderPressure, PressureSetpoint := REAL#4.5);
Pump1Enable := LeadId = INT#1;
Pump2Enable := LeadId = INT#2;
Pump3Enable := LeadId = INT#3;
AlarmOut := Station.AlarmActive;
END_PROGRAM
