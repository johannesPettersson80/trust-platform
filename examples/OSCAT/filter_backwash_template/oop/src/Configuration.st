CONFIGURATION FilterBackwashTemplateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Backwash WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Backwash.FilterType AT %IW0 : INT;
    Backwash.ElapsedSecondsRaw AT %IW2 : INT;
    Backwash.TurbidityRaw AT %IW4 : INT;
    Backwash.BackwashValveOut AT %QX0.0 : BOOL;
    Backwash.RinseValveOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
