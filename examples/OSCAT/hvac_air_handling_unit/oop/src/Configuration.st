CONFIGURATION AhuOopConfiguration
RESOURCE AhuOop ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Ahu WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Ahu.ModeRequest AT %IW2 : INT;
    Ahu.OutdoorTempRaw AT %IW4 : INT;
    Ahu.SupplyTempRaw AT %IW6 : INT;
    Ahu.SmokeClear AT %IX0.0 : BOOL;
    Ahu.FreezeStatClear AT %IX0.1 : BOOL;
    Ahu.SupplyFanHealthy AT %IX0.2 : BOOL;
    Ahu.HeatingValveRaw AT %QW2 : INT;
    Ahu.SupplyFanEnable AT %QX0.0 : BOOL;
    Ahu.FireAlarmRelay AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
