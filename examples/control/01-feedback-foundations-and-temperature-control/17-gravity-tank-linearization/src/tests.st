(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheTangentAtTheOperatingLevel
VAR
    Slope : LREAL;
END_VAR
(* c = 0.020 m^(5/2)/s at h0 = 0.25 m: g = 0.020/(2 · 0.5) = 0.020 m²/s, K = 1/g = 50 s/m², τ = A/g = 25 s. *)
Slope := TangentSlope(Coefficient := LREAL#0.020, OperatingLevel := LREAL#0.25);
ASSERT_NEAR(EXPECTED := LREAL#0.020, ACTUAL := Slope, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#50.0, ACTUAL := LREAL#1.0 / Slope, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#25.0, ACTUAL := LREAL#0.5 / Slope, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheTangentErrorGrowsWithTheSquareOfTheStep
(* Tangent equilibria h0 + v/g: 0.3000 m for v = 0.001 and 0.5000 m for v = 0.005 m³/s. Exact equilibria
   (q/c)²: 0.3025 m and 0.5625 m. The difference is v²/c²: 0.0025 m and 0.0625 m, 25 times more for 5 times
   the step. A 0.005 m error allows |v| ≤ c·√0.005 = 0.00141 m³/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.3000, ACTUAL := LREAL#0.25 + LREAL#0.001 / LREAL#0.020, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5000, ACTUAL := LREAL#0.25 + LREAL#0.005 / LREAL#0.020, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.3025 - LREAL#0.3000, ACTUAL := (LREAL#0.001 / LREAL#0.020) * (LREAL#0.001 / LREAL#0.020),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5625 - LREAL#0.5000, ACTUAL := (LREAL#0.005 / LREAL#0.020) * (LREAL#0.005 / LREAL#0.020),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.001414, ACTUAL := LREAL#0.020 * SQRT(LREAL#0.005), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
