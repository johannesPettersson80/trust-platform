CONFIGURATION KilnDryerDecoratorStrategyOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Dryer WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Dryer.ProductId AT %IW0 : INT;
    Dryer.MoistureRaw AT %IW2 : INT;
    Dryer.HeatCommandRaw AT %QW0 : INT;
    Dryer.FanSpeedRaw AT %QW2 : INT;
END_VAR
END_CONFIGURATION
