FUNCTION_BLOCK BoosterStationClassic
VAR
    Pump1Runtime : REAL := REAL#120.0;
    Pump2Runtime : REAL := REAL#20.0;
    Pump3Runtime : REAL := REAL#60.0;
    LeadPumpIdValue : INT;
    RunningPumpCountValue : INT;
    AlarmActiveValue : BOOL;
    AlarmCountValue : INT;
END_VAR

METHOD PUBLIC Update : INT
VAR_INPUT
    StartCommand : BOOL;
    SuctionOk : BOOL;
    Pump1Fault : BOOL;
    Pump2Fault : BOOL;
    Pump3Fault : BOOL;
END_VAR
IF NOT SuctionOk THEN
    AlarmActiveValue := TRUE;
    AlarmCountValue := AlarmCountValue + INT#1;
    LeadPumpIdValue := INT#0;
ELSIF StartCommand THEN
    IF NOT Pump1Fault AND (Pump2Fault OR Pump1Runtime <= Pump2Runtime) AND (Pump3Fault OR Pump1Runtime <= Pump3Runtime) THEN
        LeadPumpIdValue := INT#1;
        RunningPumpCountValue := INT#1;
    ELSIF NOT Pump2Fault AND (Pump3Fault OR Pump2Runtime <= Pump3Runtime) THEN
        LeadPumpIdValue := INT#2;
        RunningPumpCountValue := INT#1;
    ELSIF NOT Pump3Fault THEN
        LeadPumpIdValue := INT#3;
        RunningPumpCountValue := INT#1;
    ELSE
        AlarmActiveValue := TRUE;
        AlarmCountValue := AlarmCountValue + INT#1;
        LeadPumpIdValue := INT#0;
        RunningPumpCountValue := INT#0;
    END_IF;
ELSE
    LeadPumpIdValue := INT#0;
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

PUBLIC PROPERTY AlarmCount : INT
GET
    AlarmCount := AlarmCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Station : BoosterStationClassic;
    LeadId : INT;
END_VAR
LeadId := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, Pump1Fault := FALSE, Pump2Fault := FALSE, Pump3Fault := FALSE);
END_PROGRAM
