TEST_PROGRAM ConveyorPulseComponentMatchesClassicDisabledState
VAR
    BeltPulse : PulseGenerator;
    PulseObject : IPulseGenerator;
    ClassicPulse : GEN_PULSE;
END_VAR

PulseObject := BeltPulse;
PulseObject.Initialize();
PulseObject.Configure(HighTime := T#500ms, LowTime := T#500ms);
PulseObject.SetEnabled(Enabled := FALSE);
ClassicPulse(ENQ := FALSE, PTH := T#500ms, PTL := T#500ms);

ASSERT_TRUE(IN := PulseObject.Update() = ClassicPulse.Q);
ASSERT_FALSE(IN := PulseObject.Output);
END_TEST_PROGRAM
