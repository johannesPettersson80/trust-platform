TEST_PROGRAM ConveyorPulseClassicRemainsLowWhenDisabled
VAR
    BeltPulse : GEN_PULSE;
END_VAR

BeltPulse(ENQ := FALSE, PTH := T#500ms, PTL := T#500ms);

ASSERT_FALSE(IN := BeltPulse.Q);
END_TEST_PROGRAM
