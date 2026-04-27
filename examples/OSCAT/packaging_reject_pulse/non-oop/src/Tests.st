TEST_PROGRAM PackagingRejectPulseClassicStaysLowWithoutDefect
VAR
    RejectPulse : GEN_PULSE;
END_VAR

RejectPulse(ENQ := FALSE, PTH := T#150ms, PTL := T#500ms);

ASSERT_FALSE(IN := RejectPulse.Q);
END_TEST_PROGRAM
