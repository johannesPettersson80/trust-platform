(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson46ComplexResponsePredictsAllThreePhysicalProbes
ASSERT_NEAR(EXPECTED := LREAL#0.266571902389806,
    ACTUAL := Lesson46PlantMagnitude(
        AngularFrequency := LREAL#0.5), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-1.52752544221293,
    ACTUAL := Lesson46RadiansToDegrees(AngleRadians :=
        Lesson46PlantPhaseRadians(AngularFrequency := LREAL#0.5)),
    DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := Lesson46PlantMagnitude(
    AngularFrequency := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-90.0,
    ACTUAL := Lesson46RadiansToDegrees(AngleRadians :=
        Lesson46PlantPhaseRadians(AngularFrequency := LREAL#2.0)),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.0166607438993628,
    ACTUAL := Lesson46PlantMagnitude(AngularFrequency := LREAL#8.0),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-178.472474557787,
    ACTUAL := Lesson46RadiansToDegrees(AngleRadians :=
        Lesson46PlantPhaseRadians(AngularFrequency := LREAL#8.0)),
    DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Lesson46ProbeRequest(
    Amplitude := LREAL#1.0,
    AngularFrequency := LREAL#2.0,
    ElapsedSeconds := LREAL#0.785398163397448),
    DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
