FUNCTION_BLOCK AhuClassic
VAR
    ActiveModeValue : INT;
    AlarmActiveValue : BOOL;
    HeatingCommandValue : REAL;
    EcoFilter : Pt1Filter;
    ComfortFilter : Pt1Filter;
END_VAR

METHOD PUBLIC Initialize
EcoFilter.Initialize();
EcoFilter.Configure(TimeConstant := T#5s, Gain := REAL#1.0);
ComfortFilter.Initialize();
ComfortFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
AlarmActiveValue := FALSE;
ActiveModeValue := INT#0;
HeatingCommandValue := REAL#0.0;
END_METHOD

METHOD PUBLIC Update : REAL
VAR_INPUT
    ModeRequest : INT;
    OutdoorTemp : REAL;
    SupplyTemp : REAL;
    SupplySetpoint : REAL;
    SmokeClear : BOOL;
    FreezeStatClear : BOOL;
    SupplyFanHealthy : BOOL;
END_VAR
VAR
    Error : REAL;
    Filtered : REAL;
    FrostTarget : REAL;
END_VAR
IF NOT SmokeClear OR NOT SupplyFanHealthy THEN
    AlarmActiveValue := TRUE;
    HeatingCommandValue := REAL#0.0;
ELSE
    AlarmActiveValue := FALSE;
    IF (OutdoorTemp < REAL#-5.0) OR NOT FreezeStatClear THEN
        ActiveModeValue := INT#3;
        FrostTarget := MAX(SupplySetpoint, REAL#5.0);
        HeatingCommandValue := LIMIT(REAL#20.0, (FrostTarget - SupplyTemp) * REAL#30.0, REAL#100.0);
    ELSIF ModeRequest = INT#1 THEN
        ActiveModeValue := INT#1;
        Filtered := EcoFilter.Update(Sample := SupplyTemp);
        Error := SupplySetpoint - Filtered;
        IF ABS(Error) <= REAL#2.0 THEN
            HeatingCommandValue := REAL#0.0;
        ELSE
            HeatingCommandValue := LIMIT(REAL#0.0, Error * REAL#8.0, REAL#100.0);
        END_IF;
    ELSE
        ActiveModeValue := INT#2;
        Filtered := ComfortFilter.Update(Sample := SupplyTemp);
        Error := SupplySetpoint - Filtered;
        IF ABS(Error) <= REAL#0.5 THEN
            HeatingCommandValue := REAL#0.0;
        ELSE
            HeatingCommandValue := LIMIT(REAL#0.0, Error * REAL#20.0, REAL#100.0);
        END_IF;
    END_IF;
END_IF;
Update := HeatingCommandValue;
END_METHOD

PUBLIC PROPERTY ActiveMode : INT
GET
    ActiveMode := ActiveModeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AlarmActive : BOOL
GET
    AlarmActive := AlarmActiveValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Ahu : AhuClassic;
    Command : REAL;
END_VAR
Ahu.Initialize();
Command := Ahu.Update(ModeRequest := INT#1, OutdoorTemp := REAL#10.0, SupplyTemp := REAL#18.0, SupplySetpoint := REAL#21.0, SmokeClear := TRUE, FreezeStatClear := TRUE, SupplyFanHealthy := TRUE);
END_PROGRAM
