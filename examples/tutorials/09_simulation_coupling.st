PROGRAM SimulationCouplingDemo
VAR
    tank_level_raw AT %IW0 : WORD;
    high_level_alarm AT %QX0.0 : BOOL;
END_VAR

high_level_alarm := WORD_TO_UINT(tank_level_raw) >= 500;
END_PROGRAM
