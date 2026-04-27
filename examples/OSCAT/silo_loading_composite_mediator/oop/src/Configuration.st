CONFIGURATION SiloLoadingCompositeMediatorOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM System WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    System.Silo1Request AT %IX0.0 : BOOL;
    System.Silo2Request AT %IX0.1 : BOOL;
    System.Silo3Request AT %IX0.2 : BOOL;
    System.Silo1LevelRaw AT %IW0 : INT;
    System.Silo2LevelRaw AT %IW2 : INT;
    System.Silo3LevelRaw AT %IW4 : INT;
    System.BlowerOut AT %QX0.0 : BOOL;
    System.Diverter1Out AT %QX0.1 : BOOL;
    System.Diverter2Out AT %QX0.2 : BOOL;
    System.Diverter3Out AT %QX0.3 : BOOL;
END_VAR
END_CONFIGURATION
