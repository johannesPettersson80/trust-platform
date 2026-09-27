(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson81EnergyBalanceAndSIMCGainsAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#75.0,
    ACTUAL := Lesson81HoldingPower(
        Setpoint := LREAL#35.0, ZeroPowerTemperature := LREAL#20.0,
        ProcessGain := LREAL#0.2), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#3.260869565075614,
    ACTUAL := Lesson81SIMCProportionalGain(
        ProcessGain := LREAL#0.2, TimeConstant := LREAL#5.0,
        DeadTime := LREAL#1.0, ClosedLoopTime := LREAL#6.666666667),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.652173913015123,
    ACTUAL := Lesson81SIMCIntegralGain(
        ProcessGain := LREAL#0.2, TimeConstant := LREAL#5.0,
        DeadTime := LREAL#1.0, ClosedLoopTime := LREAL#6.666666667),
    DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
