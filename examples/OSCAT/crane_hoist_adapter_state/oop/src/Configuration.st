CONFIGURATION CraneHoistAdapterStateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Crane WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Crane.LiftCommand AT %IX0.0 : BOOL;
    Crane.TravelCommand AT %IX0.1 : BOOL;
    Crane.LowerCommand AT %IX0.2 : BOOL;
    Crane.UpperLimit AT %IX0.3 : BOOL;
    Crane.Overload AT %IX0.4 : BOOL;
    Crane.HoistEnableOut AT %QX0.0 : BOOL;
    Crane.TravelEnableOut AT %QX0.1 : BOOL;
    Crane.AlarmOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
