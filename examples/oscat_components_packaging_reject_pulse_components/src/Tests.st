TEST_PROGRAM PackagingRejectPulseComponentStaysLowWithoutDefect
VAR
    RejectPulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    ClassicPulse : GEN_PULSE;
END_VAR

PulseObject := RejectPulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#150ms, LowTime := T#500ms);
PulseObject.SetEnabled(Enabled := FALSE);
ClassicPulse(ENQ := FALSE, PTH := T#150ms, PTL := T#500ms);

ASSERT_TRUE(IN := PulseObject.Update() = ClassicPulse.Q);
END_TEST_PROGRAM
