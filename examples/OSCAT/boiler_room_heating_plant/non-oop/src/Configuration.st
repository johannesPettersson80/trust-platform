CONFIGURATION BoilerRoomHeatingPlantClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM BoilerRoom WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
