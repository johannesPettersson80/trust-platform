FUNCTION_BLOCK ClassicKilnDryer
VAR
    ActiveProfileIdValue : INT;
    FanSpeedValue : REAL;
    MoistureQualityValue : BYTE;
END_VAR
METHOD PUBLIC Update : REAL
VAR_INPUT
    ProductId : INT;
    RawMoisturePct : REAL;
END_VAR
VAR
    Moisture : REAL;
END_VAR
IF RawMoisturePct < REAL#0.0 THEN
    Moisture := REAL#0.0;
    MoistureQualityValue := BYTE#0;
ELSIF RawMoisturePct > REAL#100.0 THEN
    Moisture := REAL#100.0;
    MoistureQualityValue := BYTE#0;
ELSE
    Moisture := RawMoisturePct;
    MoistureQualityValue := BYTE#2;
END_IF;
IF ProductId = INT#2 THEN
    ActiveProfileIdValue := INT#2;
    FanSpeedValue := REAL#30.0;
    Update := LIMIT(REAL#0.0, (Moisture - REAL#8.0) * REAL#2.0, REAL#80.0);
ELSE
    ActiveProfileIdValue := INT#1;
    FanSpeedValue := REAL#45.0;
    Update := LIMIT(REAL#0.0, (Moisture - REAL#12.0) * REAL#3.0, REAL#100.0);
END_IF;
END_METHOD
PUBLIC PROPERTY FanSpeed : REAL
GET
    FanSpeed := FanSpeedValue;
END_GET
END_PROPERTY
PUBLIC PROPERTY ActiveProfileId : INT
GET
    ActiveProfileId := ActiveProfileIdValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Dryer : ClassicKilnDryer;
    Heat : REAL;
END_VAR
Heat := Dryer.Update(ProductId := INT#1, RawMoisturePct := REAL#20.0);
END_PROGRAM
