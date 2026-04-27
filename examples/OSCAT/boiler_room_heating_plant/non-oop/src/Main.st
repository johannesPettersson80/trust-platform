TYPE ClassicBoilerRoomStatus :
STRUCT
    OverallReady : BOOL;
    LeadBoilerId : INT;
    LeadBoilerLoad : REAL;
    LagBoilerActive : BOOL;
    ActiveAlarmCount : INT;
    HighestAlarmClass : BYTE;
    MqttPublishReady : BOOL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK ClassicBoilerRoom
VAR
    OutdoorFilter : Pt1Filter;
    SupplyFilter : Pt1Filter;
    RunningCommand : BOOL;
    AlarmCountValue : INT;
    HighestClassValue : BYTE;
    MqttPublishReadyValue : BOOL;
    Boiler1Runtime : REAL := REAL#4200.0;
    Boiler2Runtime : REAL := REAL#3900.0;
    StatusValue : ClassicBoilerRoomStatus;
END_VAR

METHOD PUBLIC Initialize
OutdoorFilter.Initialize();
OutdoorFilter.Configure(TimeConstant := T#10s, Gain := REAL#1.0);
SupplyFilter.Initialize();
SupplyFilter.Configure(TimeConstant := T#2s, Gain := REAL#1.0);
AlarmCountValue := INT#0;
HighestClassValue := BYTE#0;
MqttPublishReadyValue := FALSE;
RunningCommand := FALSE;
END_METHOD

METHOD PUBLIC Start
RunningCommand := TRUE;
END_METHOD

METHOD PUBLIC Stop
RunningCommand := FALSE;
END_METHOD

METHOD PUBLIC Update : ClassicBoilerRoomStatus
VAR_INPUT
    OutdoorTemp : REAL;
    SupplyTemp : REAL;
    GasPressureOk : BOOL;
    LowWaterOk : BOOL;
    EStopHealthy : BOOL;
    HeatDemand : REAL;
END_VAR
VAR
    DemandLoad : REAL;
END_VAR
StatusValue.OverallReady := GasPressureOk AND LowWaterOk AND EStopHealthy;
DemandLoad := LIMIT(REAL#0.0, HeatDemand, REAL#100.0);
IF NOT StatusValue.OverallReady THEN
    AlarmCountValue := AlarmCountValue + INT#1;
    HighestClassValue := BYTE#3;
    MqttPublishReadyValue := TRUE;
    StatusValue.LeadBoilerLoad := REAL#0.0;
    StatusValue.LagBoilerActive := FALSE;
ELSIF RunningCommand THEN
    IF Boiler1Runtime <= Boiler2Runtime THEN
        StatusValue.LeadBoilerId := INT#1;
    ELSE
        StatusValue.LeadBoilerId := INT#2;
    END_IF;
    StatusValue.LeadBoilerLoad := DemandLoad;
    StatusValue.LagBoilerActive := DemandLoad > REAL#60.0;
ELSE
    StatusValue.LeadBoilerLoad := REAL#0.0;
    StatusValue.LagBoilerActive := FALSE;
END_IF;
StatusValue.ActiveAlarmCount := AlarmCountValue;
StatusValue.HighestAlarmClass := HighestClassValue;
StatusValue.MqttPublishReady := MqttPublishReadyValue;
Update := StatusValue;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Room : ClassicBoilerRoom;
    Snapshot : ClassicBoilerRoomStatus;
END_VAR
Room.Initialize();
Room.Start();
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
END_PROGRAM
