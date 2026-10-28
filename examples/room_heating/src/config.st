CONFIGURATION Config
RESOURCE MainRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
    PROGRAM Screens WITH MainTask : Screens;
    PROGRAM Simulation WITH MainTask : Simulation;
END_RESOURCE
END_CONFIGURATION
