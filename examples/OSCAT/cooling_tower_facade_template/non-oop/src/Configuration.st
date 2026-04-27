CONFIGURATION CoolingTowerFacadeTemplateClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Tower WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
