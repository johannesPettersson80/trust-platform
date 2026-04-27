CONFIGURATION CraneHoistAdapterStateClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Crane WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
