(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson33DerivativeTorqueOpposesMotionAndAddsDamping
ASSERT_NEAR(EXPECTED := LREAL#7.0,
    ACTUAL := Lesson33CriticalDerivativeGain(
        Inertia := LREAL#1.0,
        ProportionalGain := LREAL#16.0,
        PhysicalDamping := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#9.0,
    ACTUAL := Lesson33PdTorque(
        ProportionalGain := LREAL#16.0,
        DerivativeGain := LREAL#7.0,
        Setpoint := LREAL#1.0,
        Position := LREAL#0.0,
        Velocity := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#23.0,
    ACTUAL := Lesson33PdTorque(
        ProportionalGain := LREAL#16.0,
        DerivativeGain := LREAL#7.0,
        Setpoint := LREAL#1.0,
        Position := LREAL#0.0,
        Velocity := LREAL#-1.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
