CONFIGURATION BatchReactorOopConfiguration
RESOURCE BatchReactorOop ON PLC
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
    Reactor.RecipeIdRaw AT %IW2 : INT;
    Reactor.ProductTempRaw AT %IW4 : INT;
    Reactor.PhRaw AT %IW6 : INT;
    Reactor.SteamValveRaw AT %QW2 : INT;
    Reactor.CoolingValveRaw AT %QW4 : INT;
    Reactor.AgitatorSpeedRaw AT %QW6 : INT;
    Reactor.BatchCompleteOut AT %QX0.0 : BOOL;
    Reactor.AlarmOut AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
