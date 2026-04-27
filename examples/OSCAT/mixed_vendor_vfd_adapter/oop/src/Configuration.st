CONFIGURATION MixedVendorVfdAdapterOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Cell WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Cell.RunCommand AT %IX0.0 : BOOL;
    Cell.PullCordOk AT %IX0.1 : BOOL;
    Cell.SpeedSetpointRaw AT %IW0 : INT;
    Cell.AnyFaultOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
