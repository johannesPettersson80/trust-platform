CONFIGURATION AirportBaggageCommandObserverOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Diverter WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Diverter.BagIdRaw AT %IW0 : INT;
    Diverter.ScannerResultRaw AT %IW2 : INT;
    Diverter.LeftOut AT %QX0.0 : BOOL;
    Diverter.RightOut AT %QX0.1 : BOOL;
    Diverter.RejectOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
