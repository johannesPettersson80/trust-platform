CONFIGURATION PharmaFillingBuilderStateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Filler WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Filler.StartCommand AT %IX0.0 : BOOL;
    Filler.VialPresent AT %IX0.1 : BOOL;
    Filler.FillDone AT %IX0.2 : BOOL;
    Filler.WeightRaw AT %IW0 : INT;
    Filler.PumpRunOut AT %QX0.0 : BOOL;
    Filler.RejectOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
