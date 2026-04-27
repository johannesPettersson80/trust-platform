CONFIGURATION CipWashStateOopConfiguration
RESOURCE Cpu0 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Cip WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    Cip.StartCommand AT %IX0.0 : BOOL;
    Cip.StepComplete AT %IX0.1 : BOOL;
    Cip.EStopHealthy AT %IX0.2 : BOOL;
    Cip.FlowOk AT %IX0.3 : BOOL;
    Cip.ConductivityRaw AT %IW0 : INT;
    Cip.CausticTempRaw AT %IW2 : INT;
    Cip.PumpEnableOut AT %QX0.0 : BOOL;
    Cip.DrainValveOut AT %QX0.1 : BOOL;
    Cip.AlarmOut AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
