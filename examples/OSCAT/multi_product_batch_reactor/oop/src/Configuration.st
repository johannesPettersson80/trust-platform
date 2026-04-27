CONFIGURATION BatchReactorOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Reactor WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Reactor.StartCommand AT %IX0.0 : BOOL;
    Reactor.AbortCommand AT %IX0.1 : BOOL;
    Reactor.LidClosedA AT %IX0.2 : BOOL;
    Reactor.LidClosedB AT %IX0.3 : BOOL;
    Reactor.EStopHealthy AT %IX0.4 : BOOL;
    Reactor.PressureReliefIntact AT %IX0.5 : BOOL;
    Reactor.RecipeIdRaw AT %IW0 : INT;
    Reactor.ProductTempRaw AT %IW2 : INT;
    Reactor.PhRaw AT %IW4 : INT;
    Reactor.SteamValveRaw AT %QW0 : INT;
    Reactor.CoolingValveRaw AT %QW2 : INT;
    Reactor.AgitatorSpeedRaw AT %QW4 : INT;
    Reactor.BatchCompleteOut AT %QX0.0 : BOOL;
    Reactor.AlarmOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
