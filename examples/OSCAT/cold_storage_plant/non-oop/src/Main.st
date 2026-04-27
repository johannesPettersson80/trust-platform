FUNCTION_BLOCK ClassicColdStorePlant
VAR
    FreezerDemandValue : INT;
    ChillerDemandValue : INT;
    RunningCompressorCountValue : INT;
    AlarmCountValue : INT;
    MqttPublishReadyValue : BOOL;
END_VAR

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
FreezerDemandValue := INT#0;
ChillerDemandValue := INT#0;
AlarmCountValue := INT#0;
MqttPublishReadyValue := FALSE;
IF (Freezer1Temp > REAL#-24.0) AND NOT Freezer1Door THEN
    FreezerDemandValue := FreezerDemandValue + INT#2;
END_IF;
IF (Freezer2Temp > REAL#-24.0) AND NOT Freezer2Door THEN
    FreezerDemandValue := FreezerDemandValue + INT#2;
END_IF;
IF (Chiller1Temp > REAL#3.0) AND NOT Chiller1Door THEN
    ChillerDemandValue := ChillerDemandValue + INT#1;
END_IF;
IF (Chiller2Temp > REAL#3.0) AND NOT Chiller2Door THEN
    ChillerDemandValue := ChillerDemandValue + INT#1;
END_IF;
IF Freezer1Door AND (Freezer1Temp > REAL#-20.0) THEN
    AlarmCountValue := AlarmCountValue + INT#1;
    MqttPublishReadyValue := TRUE;
END_IF;
RunningCompressorCountValue := INT#0;
IF FreezerDemandValue > INT#0 THEN
    RunningCompressorCountValue := RunningCompressorCountValue + INT#1;
END_IF;
IF (FreezerDemandValue > INT#2) OR (ChillerDemandValue > INT#0) THEN
    RunningCompressorCountValue := RunningCompressorCountValue + INT#1;
END_IF;
Update := RunningCompressorCountValue;
END_METHOD

PUBLIC PROPERTY FreezerDemand : INT
GET
    FreezerDemand := FreezerDemandValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ChillerDemand : INT
GET
    ChillerDemand := ChillerDemandValue;
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
    Plant : ClassicColdStorePlant;
    Running : INT;
END_VAR
Running := Plant.Update(Freezer1Temp := REAL#-20.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-20.0, Freezer2Door := FALSE, Chiller1Temp := REAL#4.0, Chiller1Door := FALSE, Chiller2Temp := REAL#1.0, Chiller2Door := FALSE);
END_PROGRAM
