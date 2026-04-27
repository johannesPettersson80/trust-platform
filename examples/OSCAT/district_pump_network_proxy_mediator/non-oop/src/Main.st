FUNCTION_BLOCK ClassicDistrictPumpNetwork
VAR
    TotalDemandM3HValue : REAL;
    PumpSpeedPctValue : REAL;
    RemoteStaleCountValue : INT;
    ClassAAlarmValue : BOOL;
END_VAR

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
TotalDemandM3HValue := LocalDemandM3H;
RemoteStaleCountValue := INT#0;

IF NorthQualityGood AND (NorthAgeSeconds <= INT#30) THEN
    TotalDemandM3HValue := TotalDemandM3HValue + NorthDemandM3H;
ELSE
    RemoteStaleCountValue := RemoteStaleCountValue + INT#1;
END_IF;

IF EastQualityGood AND (EastAgeSeconds <= INT#30) THEN
    TotalDemandM3HValue := TotalDemandM3HValue + EastDemandM3H;
ELSE
    RemoteStaleCountValue := RemoteStaleCountValue + INT#1;
END_IF;

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

PROGRAM Main
VAR
    Network : ClassicDistrictPumpNetwork;
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
Network.Update(LocalDemandM3H := INT_TO_REAL(LocalDemandRaw), NorthDemandM3H := INT_TO_REAL(NorthDemandRaw), NorthQualityGood := NorthQualityGood, NorthAgeSeconds := NorthAgeSeconds, EastDemandM3H := INT_TO_REAL(EastDemandRaw), EastQualityGood := EastQualityGood, EastAgeSeconds := EastAgeSeconds, SuctionHealthy := SuctionHealthy);
PumpSpeedRaw := REAL_TO_INT(Network.Update(LocalDemandM3H := INT_TO_REAL(LocalDemandRaw), NorthDemandM3H := INT_TO_REAL(NorthDemandRaw), NorthQualityGood := NorthQualityGood, NorthAgeSeconds := NorthAgeSeconds, EastDemandM3H := INT_TO_REAL(EastDemandRaw), EastQualityGood := EastQualityGood, EastAgeSeconds := EastAgeSeconds, SuctionHealthy := SuctionHealthy));
StaleAlarmOut := Network.RemoteStaleCount > INT#0;
END_PROGRAM
