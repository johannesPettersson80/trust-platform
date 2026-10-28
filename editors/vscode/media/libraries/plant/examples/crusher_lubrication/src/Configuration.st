CONFIGURATION Crusher
RESOURCE Cpu ON PLC
    TASK Lubrication (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Application WITH Lubrication : Main;
END_RESOURCE
VAR_CONFIG
    Application.StartButton AT %IX0.0 : BOOL;
    Application.StopButton AT %IX0.1 : BOOL;
    Application.OilLevelOk AT %IX0.2 : BOOL;
    Application.OilFlow AT %IX0.3 : BOOL;
    Application.MainPumpRunning AT %IX0.4 : BOOL;
    Application.StandbyPumpRunning AT %IX0.5 : BOOL;
    Application.MainPumpTrip AT %IX0.6 : BOOL;
    Application.StandbyPumpTrip AT %IX0.7 : BOOL;
    Application.PressureSignal AT %IL1 : LREAL;
    Application.MainPumpRun AT %QX0.0 : BOOL;
    Application.StandbyPumpRun AT %QX0.1 : BOOL;
    Application.CrusherPermit AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
