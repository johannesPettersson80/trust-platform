CONFIGURATION AirportBaggageCommandObserverClassicConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#50ms, PRIORITY := 1);
    PROGRAM Diverter WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
