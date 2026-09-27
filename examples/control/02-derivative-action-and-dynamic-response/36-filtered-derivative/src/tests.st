(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM Lesson36SelectedFilterMovesOneThirdOfTheRemainingGap
VAR
    Filter : Control.LowPass;
    Parameters : Control.LowPassParameters;
    Output : LREAL;
END_VAR
Parameters.TimeConstant := LREAL#0.05;
Parameters.Order := FilterOrder#FirstOrder;
Filter.Configure(Parameters := Parameters);
Filter.Initialize(Input := LREAL#0.0);
Output := Filter.Update(Input := LREAL#0.003, CycleTime := LREAL#0.02);
ASSERT_NEAR(EXPECTED := LREAL#0.001, ACTUAL := Output, DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson36CandidatePasses(
    TorqueIncrementRms := LREAL#0.192292,
    PositiveLagArea := LREAL#0.017473
));
ASSERT_FALSE(IN := Lesson36CandidatePasses(
    TorqueIncrementRms := LREAL#0.097,
    PositiveLagArea := LREAL#0.031202
));
ASSERT_NEAR(EXPECTED := LREAL#0.02,
    ACTUAL := Lesson36FilterTimeConstantForTrial(TrialKind := 0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2,
    ACTUAL := Lesson36FilterTimeConstantForTrial(TrialKind := 2),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
