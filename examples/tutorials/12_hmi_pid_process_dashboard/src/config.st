CONFIGURATION TutorialPidProcess
TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : ProcessPid;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.StopCmd AT %IX0.1 : BOOL;
    P1.PressureSpikeCmd AT %IX0.2 : BOOL;
    P1.BypassCmd AT %IX0.3 : BOOL;

    P1.PumpRunning AT %QX0.0 : BOOL;
    P1.HighPressureAlarm AT %QX0.1 : BOOL;
    P1.LowFeedAlarm AT %QX0.2 : BOOL;
    P1.HighProductAlarm AT %QX0.3 : BOOL;
    P1.BypassOpen AT %QX0.4 : BOOL;
END_VAR
END_CONFIGURATION
