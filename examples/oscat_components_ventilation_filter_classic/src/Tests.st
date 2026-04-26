TEST_PROGRAM VentilationFilterClassicKeepsTwoFilterInstancesIndependent
VAR
    IntakeFilter : FT_PT1;
    ExhaustFilter : FT_PT1;
END_VAR

IntakeFilter(in := REAL#10.0, T := T#1s, K := REAL#1.0);
ExhaustFilter(in := REAL#10.0, T := T#1s, K := REAL#2.0);

ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := IntakeFilter.out, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := ExhaustFilter.out, DELTA := REAL#0.001);
END_TEST_PROGRAM
