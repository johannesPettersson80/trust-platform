CONFIGURATION RoboticPalletizerCommandStateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM Cell WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Cell.BoxPresent AT %IX0.0 : BOOL;
    Cell.PalletReady AT %IX0.1 : BOOL;
    Cell.RobotHealthy AT %IX0.2 : BOOL;
    Cell.PickVacuumOk AT %IX0.3 : BOOL;
    Cell.RobotMoveOut AT %QX0.0 : BOOL;
    Cell.RejectOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
