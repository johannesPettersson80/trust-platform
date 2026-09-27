(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson34PhysicalLoadRequiresStoredHoldingTorque
VAR
    SelectedIntegralGain : LREAL;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#0.4, ACTUAL := Lesson34ApproximatePdPosition(
    ProportionalGain := LREAL#16.0,
    PeakGravityTorque := LREAL#4.0,
    Setpoint := LREAL#0.5
), DELTA := LREAL#1.0e-12);
SelectedIntegralGain := Lesson34IntegralGain(
    ProportionalGain := LREAL#16.0,
    IntegralTime := LREAL#4.0
);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := SelectedIntegralGain, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.917702154416812, ACTUAL := Lesson34GravityTorque(
    PeakGravityTorque := LREAL#4.0,
    Position := LREAL#0.5
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.479425538604203, ACTUAL := Lesson34RequiredIntegralState(
    PeakGravityTorque := LREAL#4.0,
    Setpoint := LREAL#0.5,
    IntegralGain := SelectedIntegralGain
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.962,
    ACTUAL := Lesson35DrumLoadTorque(
        MassKg := LREAL#2.0,
        GravityMPerS2 := LREAL#9.81,
        DrumRadiusM := LREAL#0.1
    ), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
