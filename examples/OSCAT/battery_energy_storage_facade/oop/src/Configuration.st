CONFIGURATION BatteryEnergyStorageFacadeOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Cabinet WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Cabinet.EnableCommand AT %IX0.0 : BOOL;
    Cabinet.FireInputHealthy AT %IX0.1 : BOOL;
    Cabinet.SocRaw AT %IW0 : INT;
    Cabinet.GridCommandRaw AT %IW2 : INT;
    Cabinet.InverterEnableOut AT %QX0.0 : BOOL;
    Cabinet.FireIsolateOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
