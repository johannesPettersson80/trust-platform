TEST_PROGRAM VentilationFilterComponentsKeepTwoInstancesIndependent
VAR
    IntakeFilter : Pt1Filter;
    ExhaustFilter : Pt1Filter;
    IntakeObject : IPt1Filter;
    ExhaustObject : IPt1Filter;
END_VAR

IntakeObject := IntakeFilter;
ExhaustObject := ExhaustFilter;
IntakeObject.Initialize();
ExhaustObject.Initialize();
IntakeObject.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
ExhaustObject.Configure(TimeConstant := T#1s, Gain := REAL#2.0);

ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := IntakeObject.Update(Sample := REAL#10.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := ExhaustObject.Update(Sample := REAL#10.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := IntakeObject.Output, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := ExhaustObject.Output, DELTA := REAL#0.001);
END_TEST_PROGRAM
