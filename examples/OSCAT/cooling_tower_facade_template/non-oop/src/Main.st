FUNCTION_BLOCK ClassicCoolingTower
VAR
    BasinHeaterValue : BOOL;
    BleedValveValue : BOOL;
    SeasonIdValue : INT;
END_VAR
METHOD PUBLIC Update : INT
VAR_INPUT
    SeasonRequest : INT;
    WaterTempC : REAL;
    OutdoorTempC : REAL;
    ConductivityUs : REAL;
END_VAR
VAR
    FanThreshold : REAL;
    FreezeThreshold : REAL;
END_VAR
IF SeasonRequest = INT#2 THEN
    SeasonIdValue := INT#2;
    FanThreshold := REAL#32.0;
    FreezeThreshold := REAL#4.0;
ELSE
    SeasonIdValue := INT#1;
    FanThreshold := REAL#26.0;
    FreezeThreshold := REAL#-10.0;
END_IF;
BasinHeaterValue := OutdoorTempC < FreezeThreshold;
BleedValveValue := ConductivityUs > REAL#1800.0;
IF WaterTempC > (FanThreshold + REAL#5.0) THEN
    Update := INT#2;
ELSIF WaterTempC > FanThreshold THEN
    Update := INT#1;
ELSE
    Update := INT#0;
END_IF;
END_METHOD
PUBLIC PROPERTY BasinHeater : BOOL
GET
    BasinHeater := BasinHeaterValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Tower : ClassicCoolingTower;
    FanStage : INT;
END_VAR
FanStage := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#32.0, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0);
END_PROGRAM
