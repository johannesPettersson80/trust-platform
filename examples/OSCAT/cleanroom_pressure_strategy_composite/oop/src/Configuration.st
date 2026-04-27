CONFIGURATION CleanroomPressureStrategyCompositeOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Cascade WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Cascade.ModeRequest AT %IW0 : INT;
    Cascade.Room1PressureRaw AT %IW2 : INT;
    Cascade.Room2PressureRaw AT %IW4 : INT;
    Cascade.DoorOpen AT %IX0.0 : BOOL;
    Cascade.SupplyDamperRaw AT %QW0 : INT;
    Cascade.AlarmOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
