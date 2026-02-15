PROGRAM SimulationCouplingDemo
VAR
    tank_level_raw : WORD;
    high_level_alarm : BOOL;
END_VAR

high_level_alarm := WORD_TO_UINT(tank_level_raw) >= 500;
END_PROGRAM

CONFIGURATION SimulationCouplingConfig
TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : SimulationCouplingDemo;
VAR_CONFIG
    P1.tank_level_raw AT %IW0 : WORD;
    P1.high_level_alarm AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
