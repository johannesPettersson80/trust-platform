(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM PHoldsTheLevelWithAnOffsetSetByTheOutflow
(* Kp = 125 %/m: the outlet at 30 % leaves 0.24 m; at 40 %, 0.32 m, whatever the setpoint. *)
ASSERT_NEAR(EXPECTED := LREAL#0.24, ACTUAL := ProportionalLevelOffset(OutletCommandPct := LREAL#30.0,
    ProportionalGain := LREAL#125.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.32, ACTUAL := ProportionalLevelOffset(OutletCommandPct := LREAL#40.0,
    ProportionalGain := LREAL#125.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM SimcKeepsTheTwoIntegratorsCriticallyDamped
(* K' = 0.0008 m/(s·%), L = 2 s, the more robust lambda = 4L = 8 s: Kp = 125 %/m, Ti = 40 s, Kp·K'·Ti = 4. *)
ASSERT_NEAR(EXPECTED := LREAL#125.0, ACTUAL := IntegratingProportionalGain(RateGain := LREAL#0.0008, ResponseTimeS := LREAL#8.0,
    DelayS := LREAL#2.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := IntegratingIntegralTime(ResponseTimeS := LREAL#8.0, DelayS := LREAL#2.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := IntegratingProportionalGain(RateGain := LREAL#0.0008, ResponseTimeS := LREAL#8.0,
    DelayS := LREAL#2.0) * LREAL#0.0008 * IntegratingIntegralTime(ResponseTimeS := LREAL#8.0, DelayS := LREAL#2.0),
    DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
