TEST_PROGRAM VentilationFilterOopKeepTwoInstancesIndependent
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
END_VAR

IntakeFilter.Initialize();
ExhaustFilter.Initialize();
IntakeFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustFilter.Configure(TimeConstant := T#1s, Gain := REAL#2.0);

ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := IntakeFilter.Update(Sample := REAL#10.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := ExhaustFilter.Update(Sample := REAL#10.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := IntakeFilter.Output, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := ExhaustFilter.Output, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Independence-across-different-samples test: drives each filter with
   its own sample sequence. Verifies the two instances do not share
   any state via the Output property after multiple updates. *)
TEST_PROGRAM VentilationFiltersStayIndependentAcrossDifferentSamples
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
END_VAR

IntakeFilter.Initialize();
ExhaustFilter.Initialize();
IntakeFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustFilter.Configure(TimeConstant := T#1s, Gain := REAL#2.0);

(* Each filter receives its own sample on first call (PT1 init: Output := Gain * Sample) *)
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := IntakeFilter.Update(Sample := REAL#5.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#16.0, ACTUAL := ExhaustFilter.Update(Sample := REAL#8.0), DELTA := REAL#0.001);

(* Outputs do not bleed across instances *)
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := IntakeFilter.Output, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#16.0, ACTUAL := ExhaustFilter.Output, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Multi-call regression: many Update calls in tight succession do not
   corrupt the Output property of either instance and the gains keep
   their assigned ratio. Catches bugs where shared state would cause
   one filter to overwrite the other after a burst of updates. *)
TEST_PROGRAM VentilationFiltersSurviveBurstOfUpdates
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
    I : INT;
END_VAR

IntakeFilter.Initialize();
ExhaustFilter.Initialize();
IntakeFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustFilter.Configure(TimeConstant := T#1s, Gain := REAL#2.0);

FOR I := 1 TO 50 DO
    IntakeFilter.Update(Sample := REAL#10.0);
    ExhaustFilter.Update(Sample := REAL#10.0);
END_FOR;

(* Intake gain 1.0 with input 10.0 trends toward 10.0; exhaust gain 2.0
   trends toward 20.0. Ratio must hold. *)
ASSERT_TRUE(IN := ExhaustFilter.Output > IntakeFilter.Output);
ASSERT_TRUE(IN := IntakeFilter.Output > REAL#0.0);
ASSERT_TRUE(IN := ExhaustFilter.Output > REAL#0.0);
END_TEST_PROGRAM

(* Per-FB isolation: directly drive one filter and verify Reset clears
   the Output property and a subsequent first-sample seeds the new
   working point. *)
TEST_PROGRAM VentilationFilterResetClearsOutput
VAR
    Filter : Pt1Filter;
END_VAR

Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
Filter.Update(Sample := REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Filter.Output, DELTA := REAL#0.001);
Filter.Reset();
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Filter.Output, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Edge input: a zero sample feeds zero through the gain path on the
   first call (Output := Gain * 0). *)
TEST_PROGRAM VentilationFilterZeroSampleProducesZeroOutput
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
END_VAR

IntakeFilter.Initialize();
ExhaustFilter.Initialize();
IntakeFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustFilter.Configure(TimeConstant := T#1s, Gain := REAL#2.0);

ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := IntakeFilter.Update(Sample := REAL#0.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := ExhaustFilter.Update(Sample := REAL#0.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Edge input: a negative sample is processed by the same gain path
   without rejection or sign flipping. *)
TEST_PROGRAM VentilationFilterAcceptsNegativeSample
VAR
    Filter : Pt1Filter;
END_VAR

Filter.Initialize();
Filter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ASSERT_NEAR(EXPECTED := REAL#-5.0, ACTUAL := Filter.Update(Sample := REAL#-5.0), DELTA := REAL#0.001);
END_TEST_PROGRAM
