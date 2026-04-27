CONFIGURATION BoilerRoomHeatingPlantOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM BoilerRoom WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    BoilerRoom.StartCommand AT %IX0.0 : BOOL;
    BoilerRoom.StopCommand AT %IX0.1 : BOOL;
    BoilerRoom.GasPressureOk AT %IX0.2 : BOOL;
    BoilerRoom.LowWaterOk AT %IX0.3 : BOOL;
    BoilerRoom.EStopHealthy AT %IX0.4 : BOOL;
    BoilerRoom.OutdoorTempRaw AT %IW0 : INT;
    BoilerRoom.SupplyTempRaw AT %IW2 : INT;
    BoilerRoom.LeadBoilerEnable AT %QX0.0 : BOOL;
    BoilerRoom.LagBoilerEnable AT %QX0.1 : BOOL;
    BoilerRoom.AlarmOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
