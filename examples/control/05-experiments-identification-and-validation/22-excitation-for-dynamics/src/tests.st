(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson22PatternMatchesRequiredDynamics
VAR
    Frequency : LREAL;
    Value : LREAL;
END_VAR
Frequency := Lesson22NaturalFrequencyHz(
    NaturalFrequencyRadiansPerSecond := LREAL#6.0
);
Value := Lesson22Multisine(ElapsedSeconds := LREAL#0.25);
ASSERT_NEAR(Frequency, LREAL#0.9549296586, LREAL#0.000000001);
ASSERT_TRUE(Lesson22PatternAllowed(PlantSelection := 0, PatternSelection := 1));
ASSERT_TRUE(Lesson22PatternAllowed(PlantSelection := 1, PatternSelection := 5));
ASSERT_FALSE(Lesson22PatternAllowed(PlantSelection := 0, PatternSelection := 5));
ASSERT_TRUE(Value <= LREAL#1.0);
ASSERT_TRUE(Value >= LREAL#-1.0);
END_TEST_PROGRAM
