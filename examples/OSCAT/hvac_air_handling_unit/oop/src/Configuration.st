CONFIGURATION AhuOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Ahu WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Ahu.ModeRequest AT %IW0 : INT;
    Ahu.OutdoorTempRaw AT %IW2 : INT;
    Ahu.SupplyTempRaw AT %IW4 : INT;
    Ahu.SmokeClear AT %IX0.0 : BOOL;
    Ahu.FreezeStatClear AT %IX0.1 : BOOL;
    Ahu.SupplyFanHealthy AT %IX0.2 : BOOL;
    Ahu.HeatingValveRaw AT %QW0 : INT;
    Ahu.SupplyFanEnable AT %QX0.0 : BOOL;
    Ahu.FireAlarmRelay AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
