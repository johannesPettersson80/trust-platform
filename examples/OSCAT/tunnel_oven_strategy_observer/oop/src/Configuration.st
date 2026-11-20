CONFIGURATION TunnelOvenStrategyObserverOopConfiguration
RESOURCE TunnelOvenStrategyObserverOop ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Oven WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Oven.ProductId AT %IW2 : INT;
    Oven.ZoneTempRaw AT %IW4 : INT;
    Oven.OvertempInput AT %IX0.0 : BOOL;
    Oven.HeatCommandRaw AT %QW2 : INT;
    Oven.ConveyorSpeedRaw AT %QW4 : INT;
    Oven.AlarmOut AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
