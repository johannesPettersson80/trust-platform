CONFIGURATION ColdStoragePlantOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Plant WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Plant.Freezer1TempRaw AT %IW0 : INT;
    Plant.Freezer2TempRaw AT %IW2 : INT;
    Plant.Chiller1TempRaw AT %IW4 : INT;
    Plant.Chiller2TempRaw AT %IW6 : INT;
    Plant.Freezer1Door AT %IX0.0 : BOOL;
    Plant.Freezer2Door AT %IX0.1 : BOOL;
    Plant.Chiller1Door AT %IX0.2 : BOOL;
    Plant.Chiller2Door AT %IX0.3 : BOOL;
    Plant.Compressor1Out AT %QX0.0 : BOOL;
    Plant.Compressor2Out AT %QX0.1 : BOOL;
    Plant.Compressor3Out AT %QX0.2 : BOOL;
    Plant.AlarmOut AT %QX0.3 : BOOL;
END_VAR
END_CONFIGURATION
