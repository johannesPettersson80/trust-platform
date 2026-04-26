TEST_PROGRAM WastewaterAerationClassicGatesPulseWithDemand
VAR
    OxygenDemand : HYST;
    AerationPulse : GEN_PULSE;
    ExpectedPulse : GEN_PULSE;
END_VAR

OxygenDemand(In := REAL#2.5, HIGH_LIMIT := REAL#2.0, LOW_LIMIT := REAL#1.0);
AerationPulse(ENQ := OxygenDemand.Q, PTH := T#2s, PTL := T#3s);
ExpectedPulse(ENQ := TRUE, PTH := T#2s, PTL := T#3s);

ASSERT_TRUE(IN := OxygenDemand.Q);
ASSERT_TRUE(IN := AerationPulse.Q = ExpectedPulse.Q);
END_TEST_PROGRAM
