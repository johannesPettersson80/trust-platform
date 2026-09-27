(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson27SelectionsFollowTheDeclaredPhysicalBalances
ASSERT_NEAR(EXPECTED := LREAL#50.0, ACTUAL := Lesson27HeaterEquilibriumCommand(
    Setpoint := LREAL#30.0,
    AmbientEquivalentTemperature := LREAL#20.0,
    EquilibriumGain := LREAL#0.2
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#65.0, ACTUAL := Lesson27HeaterEquilibriumCommand(
    Setpoint := LREAL#30.0,
    AmbientEquivalentTemperature := LREAL#17.0,
    EquilibriumGain := LREAL#0.2
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := Lesson27TankClosedLoopTimeConstant(
    IntegratingGain := LREAL#0.2,
    ProportionalGain := LREAL#2.0
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#3.365883939231586, ACTUAL := Lesson27GravityHoldingTorque(
    PeakGravityTorque := LREAL#4.0,
    Position := LREAL#1.0
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
