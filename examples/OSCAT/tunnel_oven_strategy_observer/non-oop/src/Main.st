FUNCTION_BLOCK ClassicTunnelOven
VAR
    ActiveProfileIdValue : INT;
    ConveyorSpeedValue : REAL;
    QualityEventCountValue : INT;
    MqttPublishReadyValue : BOOL;
END_VAR

METHOD PUBLIC Update : REAL
VAR_INPUT
    ProductId : INT;
    ZoneTemp : REAL;
    Overtemp : BOOL;
END_VAR
IF ProductId = INT#2 THEN
    ActiveProfileIdValue := INT#2;
    ConveyorSpeedValue := REAL#55.0;
    Update := LIMIT(REAL#0.0, (REAL#235.0 - ZoneTemp) * REAL#2.0, REAL#100.0);
ELSE
    ActiveProfileIdValue := INT#1;
    ConveyorSpeedValue := REAL#35.0;
    Update := LIMIT(REAL#0.0, (REAL#185.0 - ZoneTemp) * REAL#1.5, REAL#100.0);
END_IF;
IF Overtemp THEN
    Update := REAL#0.0;
    QualityEventCountValue := QualityEventCountValue + INT#1;
    MqttPublishReadyValue := TRUE;
END_IF;
END_METHOD

PUBLIC PROPERTY ActiveProfileId : INT
GET
    ActiveProfileId := ActiveProfileIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ConveyorSpeed : REAL
GET
    ConveyorSpeed := ConveyorSpeedValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY QualityEventCount : INT
GET
    QualityEventCount := QualityEventCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Oven : ClassicTunnelOven;
    Heat : REAL;
END_VAR
Heat := Oven.Update(ProductId := INT#1, ZoneTemp := REAL#180.0, Overtemp := FALSE);
END_PROGRAM
