CONFIGURATION PasteurizerQualityChainOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Pasteurizer WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Pasteurizer.RecipeId AT %IW0 : INT;
    Pasteurizer.ProductTempRaw AT %IW2 : INT;
    Pasteurizer.FlowRateRaw AT %IW4 : INT;
    Pasteurizer.OperatorAck AT %IX0.0 : BOOL;
    Pasteurizer.SupervisorOnline AT %IX0.1 : BOOL;
    Pasteurizer.ForwardValveOut AT %QX0.0 : BOOL;
    Pasteurizer.DivertValveOut AT %QX0.1 : BOOL;
    Pasteurizer.AlarmOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
