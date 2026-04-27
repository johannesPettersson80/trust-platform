CONFIGURATION ChemicalDosingCommandOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Dosing WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Dosing.EnqueueChlorine AT %IX0.0 : BOOL;
    Dosing.ExecuteCommand AT %IX0.1 : BOOL;
    Dosing.CalibrationValid AT %IX0.2 : BOOL;
    Dosing.PHRaw AT %IW0 : INT;
    Dosing.TankLevelRaw AT %IW2 : INT;
    Dosing.PumpStartOut AT %QX0.0 : BOOL;
    Dosing.RejectOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
