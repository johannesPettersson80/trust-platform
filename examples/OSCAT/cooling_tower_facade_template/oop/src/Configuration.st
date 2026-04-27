CONFIGURATION CoolingTowerFacadeTemplateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Tower WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Tower.SeasonRequest AT %IW0 : INT;
    Tower.WaterTempRaw AT %IW2 : INT;
    Tower.OutdoorTempRaw AT %IW4 : INT;
    Tower.ConductivityRaw AT %IW6 : INT;
    Tower.FanStageRaw AT %QW0 : INT;
    Tower.BasinHeaterOut AT %QX0.0 : BOOL;
    Tower.BleedValveOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
