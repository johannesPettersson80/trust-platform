PROGRAM Main
VAR
    OxygenDemand : HYST;
    AerationPulse : GEN_PULSE;
    DissolvedOxygen : REAL := REAL#1.5;
    BlowerCommand : BOOL;
END_VAR

OxygenDemand(In := DissolvedOxygen, HIGH_LIMIT := REAL#2.0, LOW_LIMIT := REAL#1.0);
AerationPulse(ENQ := OxygenDemand.Q, PTH := T#2s, PTL := T#3s);
BlowerCommand := AerationPulse.Q;
END_PROGRAM
