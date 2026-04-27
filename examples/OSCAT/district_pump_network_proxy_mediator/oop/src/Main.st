(* Pattern role: IStationProxy is the contract boundary for the Proxy and Mediator example. *)
INTERFACE IStationProxy
    PROPERTY StationId : INT
    GET
    END_GET
    END_PROPERTY

    PROPERTY DemandM3H : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY IsStale : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY QualityGood : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD UpdateProxy
    VAR_INPUT
        DemandM3H : REAL;
        QualityGoodInput : BOOL;
        AgeSeconds : INT;
    END_VAR
    END_METHOD
END_INTERFACE

(* Pattern role: LocalStationProxy is a concrete implementation in the Proxy and Mediator example. *)
FUNCTION_BLOCK LocalStationProxy IMPLEMENTS IStationProxy
VAR
    DemandValue : REAL;
END_VAR

METHOD PUBLIC UpdateProxy
VAR_INPUT
    DemandM3H : REAL;
    QualityGoodInput : BOOL;
    AgeSeconds : INT;
END_VAR
DemandValue := DemandM3H;
END_METHOD

PUBLIC PROPERTY StationId : INT
GET
    StationId := INT#0;
END_GET
END_PROPERTY

PUBLIC PROPERTY DemandM3H : REAL
GET
    DemandM3H := DemandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsStale : BOOL
GET
    IsStale := FALSE;
END_GET
END_PROPERTY

PUBLIC PROPERTY QualityGood : BOOL
GET
    QualityGood := TRUE;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Pattern role: RemoteStationProxy is a concrete implementation in the Proxy and Mediator example. *)
FUNCTION_BLOCK RemoteStationProxy IMPLEMENTS IStationProxy
VAR
    StationIdValue : INT;
    DemandValue : REAL;
    StaleValue : BOOL;
    QualityGoodValue : BOOL;
    LastAgeSecondsValue : INT;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    StationId : INT;
END_VAR
StationIdValue := StationId;
END_METHOD

METHOD PUBLIC UpdateProxy
VAR_INPUT
    DemandM3H : REAL;
    QualityGoodInput : BOOL;
    AgeSeconds : INT;
END_VAR
DemandValue := DemandM3H;
QualityGoodValue := QualityGoodInput;
LastAgeSecondsValue := AgeSeconds;
StaleValue := (NOT QualityGoodInput) OR (AgeSeconds > INT#30);
END_METHOD

PUBLIC PROPERTY StationId : INT
GET
    StationId := StationIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY DemandM3H : REAL
GET
    DemandM3H := DemandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsStale : BOOL
GET
    IsStale := StaleValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY QualityGood : BOOL
GET
    QualityGood := QualityGoodValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LastAgeSeconds : INT
GET
    LastAgeSeconds := LastAgeSecondsValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Composition role: DemandMediator owns the collaborating objects used by this Proxy and Mediator example. *)
FUNCTION_BLOCK DemandMediator
VAR
    Local : LocalStationProxy;
    NorthRemote : RemoteStationProxy;
    EastRemote : RemoteStationProxy;
    TotalDemandM3HValue : REAL;
    PumpSpeedPctValue : REAL;
    RemoteStaleCountValue : INT;
    ClassAAlarmValue : BOOL;
END_VAR

METHOD PUBLIC Initialize
NorthRemote.Configure(StationId := INT#1);
EastRemote.Configure(StationId := INT#2);
END_METHOD

METHOD PRIVATE AcceptProxy
VAR_INPUT
    Station : IStationProxy;
    CountStale : BOOL;
END_VAR
IF Station.IsStale THEN
    IF CountStale THEN
        RemoteStaleCountValue := RemoteStaleCountValue + INT#1;
    END_IF;
ELSE
    TotalDemandM3HValue := TotalDemandM3HValue + Station.DemandM3H;
END_IF;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    LocalDemandM3H : REAL;
    NorthDemandM3H : REAL;
    NorthQualityGood : BOOL;
    NorthAgeSeconds : INT;
    EastDemandM3H : REAL;
    EastQualityGood : BOOL;
    EastAgeSeconds : INT;
    SuctionHealthy : BOOL;
END_VAR
Local.UpdateProxy(DemandM3H := LocalDemandM3H, QualityGoodInput := TRUE, AgeSeconds := INT#0);
NorthRemote.UpdateProxy(DemandM3H := NorthDemandM3H, QualityGoodInput := NorthQualityGood, AgeSeconds := NorthAgeSeconds);
EastRemote.UpdateProxy(DemandM3H := EastDemandM3H, QualityGoodInput := EastQualityGood, AgeSeconds := EastAgeSeconds);

TotalDemandM3HValue := REAL#0.0;
RemoteStaleCountValue := INT#0;
AcceptProxy(Station := Local, CountStale := FALSE);
AcceptProxy(Station := NorthRemote, CountStale := TRUE);
AcceptProxy(Station := EastRemote, CountStale := TRUE);

ClassAAlarmValue := NOT SuctionHealthy;
IF ClassAAlarmValue THEN
    PumpSpeedPctValue := REAL#0.0;
ELSE
    PumpSpeedPctValue := LIMIT(REAL#0.0, TotalDemandM3HValue * REAL#2.0, REAL#100.0);
END_IF;
Update := PumpSpeedPctValue;
END_METHOD

PUBLIC PROPERTY TotalDemandM3H : REAL
GET
    TotalDemandM3H := TotalDemandM3HValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY PumpSpeedPct : REAL
GET
    PumpSpeedPct := PumpSpeedPctValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY RemoteStaleCount : INT
GET
    RemoteStaleCount := RemoteStaleCountValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ClassAAlarm : BOOL
GET
    ClassAAlarm := ClassAAlarmValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    Network : DemandMediator;
    LocalDemandRaw : INT;
    NorthDemandRaw : INT;
    EastDemandRaw : INT;
    NorthAgeSeconds : INT;
    EastAgeSeconds : INT;
    NorthQualityGood : BOOL;
    EastQualityGood : BOOL;
    SuctionHealthy : BOOL;
    PumpSpeedRaw : INT;
    StaleAlarmOut : BOOL;
END_VAR
Network.Initialize();
Network.Update(LocalDemandM3H := INT_TO_REAL(LocalDemandRaw), NorthDemandM3H := INT_TO_REAL(NorthDemandRaw), NorthQualityGood := NorthQualityGood, NorthAgeSeconds := NorthAgeSeconds, EastDemandM3H := INT_TO_REAL(EastDemandRaw), EastQualityGood := EastQualityGood, EastAgeSeconds := EastAgeSeconds, SuctionHealthy := SuctionHealthy);
PumpSpeedRaw := REAL_TO_INT(Network.PumpSpeedPct);
StaleAlarmOut := Network.RemoteStaleCount > INT#0;
END_PROGRAM
