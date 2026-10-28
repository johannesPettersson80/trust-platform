(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheFeedbackRow
(* x = [0.02, 0] with K = [0.06, 0.04] m²/s and the setpoint at the operating level: v = −0.06·0.02 = −0.0012 m³/s,
   so q = 0.010 − 0.0012 = 0.0088 m³/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0088, ACTUAL := FeedbackFlowM3PerS(Tank1M := LREAL#0.27, Tank2M := LREAL#0.25, SetpointM := LREAL#0.25, K1 := LREAL#0.06, K2 := LREAL#0.04, N := LREAL#0.12), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheReferenceTerm
(* Both levels at a 0.26 m setpoint with N = 0.02 + K1 + K2: q = 0.010 + 0.01·(N − K1 − K2) = 0.0102 m³/s, the flow that
   holds both tanks 1 cm higher in the local model. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0102, ACTUAL := FeedbackFlowM3PerS(Tank1M := LREAL#0.26, Tank2M := LREAL#0.26, SetpointM := LREAL#0.26, K1 := LREAL#0.06, K2 := LREAL#0.04, N := LREAL#0.12), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFlowLimits
(* The pump delivers 0–0.020 m³/s; a request outside that range is limited, a request inside is passed on. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#-0.005), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.020, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#0.025), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.0088, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#0.0088), DELTA := LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM TheSetpointGuard
(* A setpoint outside the 0.05–0.45 m levels, such as an unwritten 0, is replaced by the operating level 0.25 m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := ValidSetpointM(SetpointM := LREAL#0.0), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.26, ACTUAL := ValidSetpointM(SetpointM := LREAL#0.26), DELTA := LREAL#0.0);
END_TEST_PROGRAM
