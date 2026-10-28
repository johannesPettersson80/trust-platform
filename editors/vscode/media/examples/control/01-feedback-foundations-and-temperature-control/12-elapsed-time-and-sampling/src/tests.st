(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM IntegralUsesTheElapsedInterval
ASSERT_NEAR(EXPECTED := LREAL#0.05, ACTUAL := IntegralChange(
    IntegralGain := LREAL#0.5, Error := LREAL#1.0, ElapsedTime := LREAL#0.1), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.10, ACTUAL := IntegralChange(
    IntegralGain := LREAL#0.5, Error := LREAL#1.0, ElapsedTime := LREAL#0.2), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM FastSineAliasesToSlowSine
VAR
    SampleNumber : DINT;
END_VAR
(* Sampled every 0.1 s, an 8 Hz sine gives exactly the samples of an inverted 2 Hz sine. *)
FOR SampleNumber := 0 TO 9 DO
    ASSERT_NEAR(EXPECTED := -SampledSine(Frequency := LREAL#2.0, SampleInterval := LREAL#0.1, SampleNumber := SampleNumber),
        ACTUAL := SampledSine(Frequency := LREAL#8.0, SampleInterval := LREAL#0.1, SampleNumber := SampleNumber),
        DELTA := LREAL#1.0e-9);
END_FOR;
END_TEST_PROGRAM

TEST_PROGRAM OneSamplePerCycleLooksConstant
VAR
    SampleNumber : DINT;
END_VAR
(* Sampled once per cycle, a 10 Hz sine is zero at every sample. *)
FOR SampleNumber := 0 TO 9 DO
    ASSERT_NEAR(EXPECTED := LREAL#0.0,
        ACTUAL := SampledSine(Frequency := LREAL#10.0, SampleInterval := LREAL#0.1, SampleNumber := SampleNumber),
        DELTA := LREAL#1.0e-9);
END_FOR;
END_TEST_PROGRAM

TEST_PROGRAM UnusableIntervalsAreRejected
(* No elapsed time, a clock that went back, and a gap longer than half the 4.42 s ultimate period are unusable. *)
ASSERT_FALSE(IntervalUsable(ElapsedTime := LREAL#0.0, MaximumInterval := LREAL#2.2));
ASSERT_FALSE(IntervalUsable(ElapsedTime := LREAL#-0.1, MaximumInterval := LREAL#2.2));
ASSERT_TRUE(IntervalUsable(ElapsedTime := LREAL#0.1, MaximumInterval := LREAL#2.2));
ASSERT_TRUE(IntervalUsable(ElapsedTime := LREAL#1.0, MaximumInterval := LREAL#2.2));
ASSERT_FALSE(IntervalUsable(ElapsedTime := LREAL#3.1, MaximumInterval := LREAL#2.2));
END_TEST_PROGRAM
