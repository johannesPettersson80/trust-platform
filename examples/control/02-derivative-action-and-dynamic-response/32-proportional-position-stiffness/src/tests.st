(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson32ProportionalGainCreatesPositionStiffness
ASSERT_NEAR(EXPECTED := LREAL#4.0,
    ACTUAL := Lesson32ProportionalTorque(
        ProportionalGain := LREAL#4.0,
        Setpoint := LREAL#1.0,
        Position := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0,
    ACTUAL := Lesson32NaturalFrequency(
        ProportionalGain := LREAL#4.0,
        Inertia := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.25,
    ACTUAL := Lesson32DampingRatio(
        ViscousDamping := LREAL#1.0,
        Inertia := LREAL#1.0,
        ProportionalGain := LREAL#4.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.125,
    ACTUAL := Lesson32DampingRatio(
        ViscousDamping := LREAL#1.0,
        Inertia := LREAL#1.0,
        ProportionalGain := LREAL#16.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
