(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheOutletFlowFollowsTheSquareRootOfTheLevel
(* v = √(2gh): 2.215 m/s at 0.25 m. With c = 0.020 m^(5/2)/s the outlet passes 0.010 m³/s at 0.25 m and
   0.020 · √0.5 = 0.01414 m³/s at 0.50 m: twice the level gives √2 times the flow, not twice. *)
ASSERT_NEAR(EXPECTED := LREAL#2.2147, ACTUAL := SQRT(LREAL#2.0 * LREAL#9.81 * LREAL#0.25), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#0.010, ACTUAL := GravityOutflow(Coefficient := LREAL#0.020, Level := LREAL#0.25),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.010 * SQRT(LREAL#2.0), ACTUAL := GravityOutflow(Coefficient := LREAL#0.020,
    Level := LREAL#0.50), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheEquilibriumFollowsTheSquareOfTheInflow
(* 0.010 m³/s balances the outlet at 0.25 m; 0.011 m³/s at (0.011/0.020)² = 0.3025 m. The step starts the rise at
   (0.011 − 0.010)/0.5 = 0.002 m/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := EquilibriumLevel(InletFlow := LREAL#0.010, Coefficient := LREAL#0.020),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.3025, ACTUAL := EquilibriumLevel(InletFlow := LREAL#0.011, Coefficient := LREAL#0.020),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.002, ACTUAL := (LREAL#0.011 - GravityOutflow(Coefficient := LREAL#0.020,
    Level := LREAL#0.25)) / LREAL#0.5, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
