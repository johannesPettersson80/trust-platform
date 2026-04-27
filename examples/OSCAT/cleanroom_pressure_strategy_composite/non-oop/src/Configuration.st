CONFIGURATION CleanroomPressureStrategyCompositeClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Cascade WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
