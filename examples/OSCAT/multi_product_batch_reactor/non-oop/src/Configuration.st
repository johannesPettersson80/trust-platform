CONFIGURATION BatchReactorClassicConfiguration
RESOURCE BatchReactorClassic ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Reactor WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
