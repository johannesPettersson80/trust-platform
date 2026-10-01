(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheCompensatedLaw
(* At upright rest with the 0.1 rad target the law gives 16·0.1 = 1.6 N·m, whatever Ĝ. At the target at rest it gives only
   −Ĝ·sin 0.1: −0.3993 N·m with Ĝ = 4, which balances gravity's 4·sin 0.1, and 0 without compensation. *)
ASSERT_NEAR(EXPECTED := LREAL#1.6, ACTUAL := CompensatedTorqueNm(AngleRad := LREAL#0.0, SpeedRadPerS := LREAL#0.0, SetpointRad := LREAL#0.1, GravityEstimateNm := LREAL#4.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.3993336666, ACTUAL := CompensatedTorqueNm(AngleRad := LREAL#0.1, SpeedRadPerS := LREAL#0.0, SetpointRad := LREAL#0.1, GravityEstimateNm := LREAL#4.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := CompensatedTorqueNm(AngleRad := LREAL#0.1, SpeedRadPerS := LREAL#0.0, SetpointRad := LREAL#0.1, GravityEstimateNm := LREAL#0.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheBalanceWithoutCompensation
(* Without compensation the arm rests where the P term balances gravity, 16(0.1 − φ) = −4·sin φ: at φ = 0.13320 rad the
   law gives −0.53120 N·m and gravity 4·sin 0.13320 = 0.53120 N·m. *)
ASSERT_NEAR(EXPECTED := LREAL#-4.0 * SIN(LREAL#0.1332), ACTUAL := CompensatedTorqueNm(AngleRad := LREAL#0.1332, SpeedRadPerS := LREAL#0.0, SetpointRad := LREAL#0.1, GravityEstimateNm := LREAL#0.0), DELTA := LREAL#1.0e-4);
END_TEST_PROGRAM
