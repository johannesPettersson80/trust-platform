CONFIGURATION PackagingRejectPulseClassicConfiguration
RESOURCE PackagingRejectPulseClassic ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
