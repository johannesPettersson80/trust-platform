CONFIGURATION TunnelWasherChainOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Washer WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Washer.SoilLevelRaw AT %IW0 : INT;
    Washer.ConductivityRaw AT %IW2 : INT;
    Washer.OperatorAvailable AT %IX0.0 : BOOL;
    Washer.DosePumpOut AT %QX0.0 : BOOL;
    Washer.RejectOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
