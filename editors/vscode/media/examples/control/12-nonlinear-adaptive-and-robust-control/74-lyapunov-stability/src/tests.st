(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheEnergyFunction
(* V = ½ω² + 8φ² + 4(cos φ − 1): 0.0600167 J at 0.1 rad and rest, inside V ≤ 0.1; 0.2402663 J at 0.2 rad, outside; and
   0.1 J at upright with ω = √0.2 rad/s, the speed bound of the certificate. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0600166611, ACTUAL := ArmEnergyJ(AngleRad := LREAL#0.1, SpeedRadPerS := LREAL#0.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.2402663114, ACTUAL := ArmEnergyJ(AngleRad := LREAL#0.2, SpeedRadPerS := LREAL#0.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.1, ACTUAL := ArmEnergyJ(AngleRad := LREAL#0.0, SpeedRadPerS := SQRT(LREAL#0.2)), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheLaws
(* The tested law at 0.1 rad and rest gives −1.6 N·m; at the certificate's corner, 0.1291 rad and 0.4472 rad/s, it gives
   −5.196 N·m, inside the ±20 N·m limit. The hold at its target gives only −4·sin 0.1 = −0.3993 N·m, the torque that
   cancels gravity. *)
ASSERT_NEAR(EXPECTED := LREAL#-1.6, ACTUAL := PdTorqueNm(AngleRad := LREAL#0.1, SpeedRadPerS := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-5.196, ACTUAL := PdTorqueNm(AngleRad := LREAL#0.1291, SpeedRadPerS := LREAL#0.4472), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-0.3993336666, ACTUAL := HoldTorqueNm(AngleRad := LREAL#0.1, SpeedRadPerS := LREAL#0.0, TargetRad := LREAL#0.1), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
