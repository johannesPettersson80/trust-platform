CONFIGURATION WaterBoosterOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Station WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Station.StartCommand AT %IX0.0 : BOOL;
    Station.SuctionOk AT %IX0.1 : BOOL;
    Station.Pump1Fault AT %IX0.2 : BOOL;
    Station.Pump2Fault AT %IX0.3 : BOOL;
    Station.Pump3Fault AT %IX0.4 : BOOL;
    Station.HeaderPressureRaw AT %IW0 : INT;
    Station.Pump1Enable AT %QX0.0 : BOOL;
    Station.Pump2Enable AT %QX0.1 : BOOL;
    Station.Pump3Enable AT %QX0.2 : BOOL;
    Station.AlarmOut AT %QX0.3 : BOOL;
END_VAR
END_CONFIGURATION
