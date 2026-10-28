(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheFeedforwardSuppliesTheTorqueOfThePlan
(* J = 1 kg·m², B = 1 N·m·s/rad: 3 N·m at the end of the acceleration, 1 N·m cruising, −1 N·m as braking starts. *)
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := FeedforwardTorque(Inertia := LREAL#1.0, Friction := LREAL#1.0,
    AccelerationRadS2 := LREAL#2.0, VelocityRadS := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := FeedforwardTorque(Inertia := LREAL#1.0, Friction := LREAL#1.0,
    AccelerationRadS2 := LREAL#0.0, VelocityRadS := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-1.0, ACTUAL := FeedforwardTorque(Inertia := LREAL#1.0, Friction := LREAL#1.0,
    AccelerationRadS2 := LREAL#-2.0, VelocityRadS := LREAL#1.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM AMoveTakesItsAccelerationCruiseAndBraking
(* 1 rad at 1 rad/s and 2 rad/s²: 0.5 s accelerating, 0.5 s cruising, 0.5 s braking. *)
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := MoveDurationS(DistanceRad := LREAL#1.0, MaxVelocityRadS := LREAL#1.0,
    AccelerationRadS2 := LREAL#2.0), DELTA := LREAL#1.0e-12);
(* 0.2 rad is shorter than V²/A = 0.5 rad: it accelerates for √(0.2/2) s and brakes as long. *)
ASSERT_NEAR(EXPECTED := LREAL#0.6324555320336759, ACTUAL := MoveDurationS(DistanceRad := LREAL#0.2, MaxVelocityRadS := LREAL#1.0,
    AccelerationRadS2 := LREAL#2.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM DerivativeOnTheMeasurementBrakesTheMove
ASSERT_NEAR(EXPECTED := LREAL#7.27, ACTUAL := DerivativeBrakeTorque(DerivativeGain := LREAL#7.27, VelocityRadS := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
