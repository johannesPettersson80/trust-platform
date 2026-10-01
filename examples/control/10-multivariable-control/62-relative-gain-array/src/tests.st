(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheCompensatedExperiment
(* Heater 1 +5 points with vessel 2 held: heater 2 must move by −(4/9)·5 = −2.2222 points, and vessel 1 rises by
   (1/9)·5 = 0.5556 °C instead of 0.6923 °C. *)
ASSERT_NEAR(EXPECTED := LREAL#-2.2222222, ACTUAL := CompensatingCommandPct(V1Pct := LREAL#5.0, C := LREAL#4.0 / LREAL#65.0,
    D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.1111111, ACTUAL := HeldOutputGain(A := LREAL#9.0 / LREAL#65.0, B := LREAL#4.0 / LREAL#65.0,
    C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.5555556, ACTUAL := LREAL#5.0 * HeldOutputGain(A := LREAL#9.0 / LREAL#65.0,
    B := LREAL#4.0 / LREAL#65.0, C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-7);
END_TEST_PROGRAM

TEST_PROGRAM TheRelativeGainArray
(* det K = 81/65² − 16/65² = 1/65. λ11 = (9/65)·9 = 81/65 = 1.2462, the isolated gain 9/65 over the held gain 1/9;
   λ12 = (4/65)·(−4) = −16/65. Each row sums to one. *)
ASSERT_NEAR(EXPECTED := LREAL#1.0 / LREAL#65.0, ACTUAL := Determinant(A := LREAL#9.0 / LREAL#65.0, B := LREAL#4.0 / LREAL#65.0,
    C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.2461538, ACTUAL := RelativeGain11(A := LREAL#9.0 / LREAL#65.0, B := LREAL#4.0 / LREAL#65.0,
    C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#-0.2461538, ACTUAL := RelativeGain12(A := LREAL#9.0 / LREAL#65.0, B := LREAL#4.0 / LREAL#65.0,
    C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := (LREAL#9.0 / LREAL#65.0) / (LREAL#1.0 / LREAL#9.0), ACTUAL := RelativeGain11(A := LREAL#9.0 / LREAL#65.0,
    B := LREAL#4.0 / LREAL#65.0, C := LREAL#4.0 / LREAL#65.0, D := LREAL#9.0 / LREAL#65.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheGuardedRatios
(* K = [[2, 1], [1, 1]] gives λ11 = 2·1/1 = 2. The singular K = [[1, 2], [2, 4]] has no inverse, and D = 0 means input 2
   cannot hold output 2: those results are 0, not a division by zero. *)
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := RelativeGain11(A := LREAL#2.0, B := LREAL#1.0, C := LREAL#1.0, D := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := RelativeGain11(A := LREAL#1.0, B := LREAL#2.0, C := LREAL#2.0, D := LREAL#4.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := HeldOutputGain(A := LREAL#1.0, B := LREAL#1.0, C := LREAL#1.0, D := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := CompensatingCommandPct(V1Pct := LREAL#5.0, C := LREAL#1.0, D := LREAL#0.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
