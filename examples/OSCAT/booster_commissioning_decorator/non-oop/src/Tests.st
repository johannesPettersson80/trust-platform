TEST_PROGRAM ClassicCommissioningUsesInlineBranches
VAR
    Cell : ClassicCommissioningBooster;
    Speed : REAL;
END_VAR

Cell.Initialize();
Speed := Cell.Update(RealPressureBar := REAL#9.0, CommissioningMode := TRUE, LoopbackOnly := TRUE, PressureSetpoint := REAL#4.5);
ASSERT_FALSE(IN := Cell.RealPumpRunning);
ASSERT_TRUE(IN := Cell.LastCommandedSpeed >= REAL#0.0);
END_TEST_PROGRAM
