(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheLocalModelGivesTheGains
(* λ = 10 s, A = 0.5 m², c = 0.020 m^(5/2)/s: Kp = A/λ = 0.05 m²/s at every level; Ki = g/λ = c/(2λ√h) falls
   from 0.0025 m²/s² at 0.16 m through 0.002 at 0.25 m to 0.001667 at 0.36 m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.05, ACTUAL := LREAL#0.5 / LREAL#10.0, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0025, ACTUAL := OutflowSlope(Coefficient := LREAL#0.020, Level := LREAL#0.16) / LREAL#10.0,
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.002, ACTUAL := OutflowSlope(Coefficient := LREAL#0.020, Level := LREAL#0.25) / LREAL#10.0,
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.001667, ACTUAL := OutflowSlope(Coefficient := LREAL#0.020, Level := LREAL#0.36) / LREAL#10.0,
    DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM TheExperimentLevels
(* The comparison runs at 0.09, 0.25 and 0.49 m (√h = 0.3, 0.5, 0.7): holding flows 0.006, 0.010 and 0.014 m³/s,
   scheduled Ki 0.00333, 0.002 and 0.00143 m²/s². *)
ASSERT_NEAR(EXPECTED := LREAL#0.006, ACTUAL := LREAL#0.020 * SQRT(LREAL#0.09), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.014, ACTUAL := LREAL#0.020 * SQRT(LREAL#0.49), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0033333, ACTUAL := OutflowSlope(Coefficient := LREAL#0.020, Level := LREAL#0.09) / LREAL#10.0,
    DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.0014286, ACTUAL := OutflowSlope(Coefficient := LREAL#0.020, Level := LREAL#0.49) / LREAL#10.0,
    DELTA := LREAL#1.0e-7);
END_TEST_PROGRAM
