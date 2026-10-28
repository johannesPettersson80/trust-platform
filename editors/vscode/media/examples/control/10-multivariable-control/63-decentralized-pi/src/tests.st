(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheHoldingCommands
(* Vessel 1 at 37 °C with vessel 2 held at 35 °C: 2·v1 = 36 and 2·v2 = −16, so 93 % and 67 %. A 10 kW load on vessel 1 at
   35/35 °C: 80 % and 75 %. *)
ASSERT_NEAR(EXPECTED := LREAL#93.0, ACTUAL := HoldingCommand1Pct(X1C := LREAL#2.0, X2C := LREAL#0.0, D1Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#67.0, ACTUAL := HoldingCommand2Pct(X1C := LREAL#2.0, X2C := LREAL#0.0, D2Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := HoldingCommand1Pct(X1C := LREAL#0.0, X2C := LREAL#0.0, D1Kw := LREAL#10.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := HoldingCommand2Pct(X1C := LREAL#0.0, X2C := LREAL#0.0, D2Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheGainWithTheOtherLoopClosed
(* Lesson 69's K = [[9/65, 4/65], [4/65, 9/65]] °C per point: with the other loop holding its vessel, heater 1 moves vessel
   1 by 9/65 − (4/65)·(4/65)/(9/65) = 1/9 °C per point, which is K11/λ11 = (9/65)/(81/65). *)
ASSERT_NEAR(EXPECTED := LREAL#1.0 / LREAL#9.0, ACTUAL := OtherLoopClosedGain(A := LREAL#9.0 / LREAL#65.0, B := LREAL#4.0 / LREAL#65.0, C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := OtherLoopClosedGain(A := LREAL#1.0, B := LREAL#1.0, C := LREAL#1.0, D := LREAL#0.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
