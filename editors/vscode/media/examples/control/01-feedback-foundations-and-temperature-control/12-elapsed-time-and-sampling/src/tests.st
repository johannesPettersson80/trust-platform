(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson12UsesScriptedElapsedTime
ASSERT_NEAR(EXPECTED := LREAL#0.05, ACTUAL := Lesson12AccumulatedChange(
    ChangeRate := LREAL#0.5,
    ElapsedTime := LREAL#0.1
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.10, ACTUAL := Lesson12AccumulatedChange(
    ChangeRate := LREAL#0.5,
    ElapsedTime := LREAL#0.2
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM Lesson12DifferentWavesCanHaveEqualSamples
VAR
    SampleNumber : DINT;
    SlowSample : LREAL;
    FastSample : LREAL;
END_VAR
FOR SampleNumber := 0 TO 9 DO
    SlowSample := Lesson12SampledWave(
        Frequency := LREAL#2.0,
        SampleInterval := LREAL#0.1,
        SampleNumber := SampleNumber,
        ReverseSign := FALSE
    );
    FastSample := Lesson12SampledWave(
        Frequency := LREAL#8.0,
        SampleInterval := LREAL#0.1,
        SampleNumber := SampleNumber,
        ReverseSign := TRUE
    );
    ASSERT_NEAR(EXPECTED := SlowSample, ACTUAL := FastSample, DELTA := LREAL#1.0e-12);
END_FOR;
END_TEST_PROGRAM
