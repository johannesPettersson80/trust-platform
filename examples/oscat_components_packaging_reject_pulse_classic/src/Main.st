PROGRAM Main
VAR
    RejectPulse : GEN_PULSE;
    DefectDetected : BOOL := FALSE;
    RejectSolenoid : BOOL;
END_VAR

RejectPulse(ENQ := DefectDetected, PTH := T#150ms, PTL := T#500ms);
RejectSolenoid := RejectPulse.Q;
END_PROGRAM
