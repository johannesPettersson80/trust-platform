CONFIGURATION TunnelOvenStrategyObserverOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Oven WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Oven.ProductId AT %IW0 : INT;
    Oven.ZoneTempRaw AT %IW2 : INT;
    Oven.OvertempInput AT %IX0.0 : BOOL;
    Oven.HeatCommandRaw AT %QW0 : INT;
    Oven.ConveyorSpeedRaw AT %QW2 : INT;
    Oven.AlarmOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
