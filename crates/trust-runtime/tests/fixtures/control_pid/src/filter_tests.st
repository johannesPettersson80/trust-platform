USING Control;

TEST_PROGRAM control_low_pass_matches_listing_four_for_both_orders
VAR
    FirstOrderFilter : LowPass;
    SecondOrderFilter : LowPass;
    Parameters : LowPassParameters;
    Output : LREAL;
END_VAR

Parameters.TimeConstant := LREAL#1.5;
FirstOrderFilter.Configure(Parameters := Parameters);
FirstOrderFilter.Initialize(Input := LREAL#2.0);
Output := FirstOrderFilter.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);

Parameters.Order := FilterOrder#SecondOrder;
SecondOrderFilter.Configure(Parameters := Parameters);
SecondOrderFilter.Initialize(Input := LREAL#2.0);
Output := SecondOrderFilter.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := Output, DELTA := LREAL#2.5e-12);
Output := SecondOrderFilter.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);

Parameters.TimeConstant := LREAL#0.5;
Parameters.Order := FilterOrder#FirstOrder;
FirstOrderFilter.Configure(Parameters := Parameters);
FirstOrderFilter.Initialize(Input := LREAL#2.0);
Output := FirstOrderFilter.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := Output, DELTA := LREAL#4.0e-12);

Parameters.Order := FilterOrder#SecondOrder;
SecondOrderFilter.Configure(Parameters := Parameters);
SecondOrderFilter.Initialize(Input := LREAL#2.0);
Output := SecondOrderFilter.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := Output, DELTA := LREAL#4.0e-12);
END_TEST_PROGRAM
