CONFIGURATION TankFarmOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Farm WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Farm.AreaStopCommand AT %IX0.0 : BOOL;
    Farm.AreaIndex AT %IW0 : INT;
    Farm.AreaStoppedOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
