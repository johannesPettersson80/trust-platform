PROGRAM SimulationCouplingDemo
VAR
    TankLevelRaw : WORD;
    HighLevelAlarm : BOOL;
END_VAR

HighLevelAlarm := WORD_TO_UINT(TankLevelRaw) >= 500;
END_PROGRAM

CONFIGURATION SimulationCouplingConfig
RESOURCE SimulationRes ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : SimulationCouplingDemo;
END_RESOURCE
VAR_CONFIG
    P1.TankLevelRaw AT %IW0 : WORD;
    P1.HighLevelAlarm AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
