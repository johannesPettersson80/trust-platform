(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheBalanceGivesTheLevelRate
(* A = 0.5 m²: 0.012 m³/s in and 0.010 m³/s out raise the level at 0.002/0.5 = 0.004 m/s; equal flows hold it;
   the outlet alone lowers it at 0.02 m/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.004, ACTUAL := LevelRate(InletFlow := LREAL#0.012, OutletFlow := LREAL#0.010,
    Area := LREAL#0.5), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LevelRate(InletFlow := LREAL#0.010, OutletFlow := LREAL#0.010,
    Area := LREAL#0.5), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.02, ACTUAL := LevelRate(InletFlow := LREAL#0.0, OutletFlow := LREAL#0.010,
    Area := LREAL#0.5), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TenSecondsStoreTwentyLitres
VAR
    StoredM3 : LREAL;
END_VAR
(* ΔV = (q_in − q_out)·Δt = 0.002 · 10 = 0.020 m³ = 20 L, and Δh = ΔV/A = 0.04 m: 0.25 m becomes 0.29 m. *)
StoredM3 := (LREAL#0.012 - LREAL#0.010) * LREAL#10.0;
ASSERT_NEAR(EXPECTED := LREAL#0.020, ACTUAL := StoredM3, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.29, ACTUAL := LREAL#0.25 + StoredM3 / LREAL#0.5, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
