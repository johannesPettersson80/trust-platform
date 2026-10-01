(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheFeedbackRow
(* x = [0.02, 0] with the 20-step gain K0 = [0.07874026, 0.02387430] m²/s and the setpoint at the operating level:
   v = −0.07874026·0.02 = −0.0015748052 m³/s, so q = 0.0084251948 m³/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0084251948, ACTUAL := FeedbackFlowM3PerS(Tank1M := LREAL#0.27, Tank2M := LREAL#0.25, SetpointM := LREAL#0.25, K1 := LREAL#0.07874026, K2 := LREAL#0.02387430, N := LREAL#0.12261456), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheCostsUseTheDesignScales
(* A level deviation of 0.05 m or a pump deviation of 0.005 m³/s costs 1: x = [0.02, 0] costs 400·0.02² = 0.16, and the
   correction −0.0015748052 m³/s costs 40000·0.0015748052² = 0.0992005. *)
ASSERT_NEAR(EXPECTED := LREAL#0.16, ACTUAL := LevelCost(Tank1M := LREAL#0.27, Tank2M := LREAL#0.25), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := LevelCost(Tank1M := LREAL#0.25, Tank2M := LREAL#0.20), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0992004567, ACTUAL := EffortCost(FlowM3PerS := LREAL#0.0084251948), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := EffortCost(FlowM3PerS := LREAL#0.015), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFlowLimits
(* The pump delivers 0–0.020 m³/s; a request outside that range is limited, a request inside is passed on. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#-0.005), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.020, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#0.025), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.0084, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#0.0084), DELTA := LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM TheSetpointGuard
(* A setpoint outside the 0.05–0.45 m levels, such as an unwritten 0, is replaced by the operating level 0.25 m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := ValidSetpointM(SetpointM := LREAL#0.0), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.26, ACTUAL := ValidSetpointM(SetpointM := LREAL#0.26), DELTA := LREAL#0.0);
END_TEST_PROGRAM
