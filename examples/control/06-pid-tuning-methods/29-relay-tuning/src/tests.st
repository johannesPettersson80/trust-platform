(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson29RelayPhysicsProducesOnlyAQualifiedUnappliedCandidate
VAR
    FundamentalAmplitude : LREAL;
    CriticalGain : LREAL;
    HistoricalKp : LREAL;
    HistoricalTi : LREAL;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#38.0, ACTUAL := Lesson29RelayOutput(
    Bias := LREAL#20.0, Amplitude := LREAL#18.0, HighState := TRUE
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Lesson29RelayOutput(
    Bias := LREAL#20.0, Amplitude := LREAL#18.0, HighState := FALSE
), DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson29RelayHighState(
    CurrentHighState := FALSE, MeasuredTemperature := LREAL#49.92,
    CenterTemperature := LREAL#50.0, HysteresisHalfWidth := LREAL#0.08));
ASSERT_FALSE(IN := Lesson29RelayHighState(
    CurrentHighState := TRUE, MeasuredTemperature := LREAL#50.08,
    CenterTemperature := LREAL#50.0, HysteresisHalfWidth := LREAL#0.08));
ASSERT_TRUE(IN := Lesson29RelayHighState(
    CurrentHighState := TRUE, MeasuredTemperature := LREAL#50.0,
    CenterTemperature := LREAL#50.0, HysteresisHalfWidth := LREAL#0.08));
FundamentalAmplitude := Lesson29FundamentalRelayAmplitude(
    RelayAmplitude := LREAL#18.0);
ASSERT_NEAR(EXPECTED := LREAL#22.9183118052329,
    ACTUAL := FundamentalAmplitude, DELTA := LREAL#1.0e-9);
CriticalGain := Lesson29CriticalGainEstimate(
    RelayAmplitude := LREAL#18.0,
    ProcessAmplitude := LREAL#1.292694524894
);
ASSERT_NEAR(EXPECTED := LREAL#17.729101008702,
    ACTUAL := CriticalGain, DELTA := LREAL#1.0e-8);
HistoricalKp := Lesson29HistoricalProportionalGain(
    UltimateGain := CriticalGain);
HistoricalTi := Lesson29HistoricalIntegralTime(
    UltimatePeriodSeconds := LREAL#11.938461538462);
ASSERT_NEAR(EXPECTED := LREAL#7.978095453916,
    ACTUAL := HistoricalKp, DELTA := LREAL#1.0e-8);
ASSERT_NEAR(EXPECTED := LREAL#9.948717948718,
    ACTUAL := HistoricalTi, DELTA := LREAL#1.0e-8);
ASSERT_NEAR(EXPECTED := LREAL#0.801921965729,
    ACTUAL := HistoricalKp / HistoricalTi, DELTA := LREAL#1.0e-8);
END_TEST_PROGRAM
