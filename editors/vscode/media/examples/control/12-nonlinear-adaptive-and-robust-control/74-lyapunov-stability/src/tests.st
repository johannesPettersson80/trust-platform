(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson74FeedbackAndCertificateValuesAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#-0.4,
    ACTUAL := Lesson74TorqueRequest(
        AngleError := LREAL#0.1, AngularRate := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.006666666666666667,
    ACTUAL := Lesson74LyapunovValue(
        AngleError := LREAL#0.1, AngularRate := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson74LyapunovValue(
    AngleError := LREAL#0.1, AngularRate := LREAL#0.0) <= LREAL#0.025);
ASSERT_FALSE(IN := Lesson74LyapunovValue(
    AngleError := LREAL#0.25, AngularRate := LREAL#0.0) <= LREAL#0.025);
END_TEST_PROGRAM
