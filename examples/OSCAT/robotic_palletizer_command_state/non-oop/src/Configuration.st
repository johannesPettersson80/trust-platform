CONFIGURATION RoboticPalletizerCommandStateClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM Cell WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
