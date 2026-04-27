CONFIGURATION RefineryTemperatureConditioningOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Conditioning WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Conditioning.Tc1Raw AT %IW0 : INT;
    Conditioning.Tc2Raw AT %IW2 : INT;
    Conditioning.Tc3Raw AT %IW4 : INT;
    Conditioning.ConditionedRaw AT %QW0 : INT;
    Conditioning.GoodQualityOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
