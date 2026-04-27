TEST_PROGRAM ClassicCipCaseAdvancesThroughSequence
VAR
    Cip : ClassicCipController;
END_VAR

Cip.Initialize();
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
END_TEST_PROGRAM
