(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheEstimatorStep
(* Old estimate x̂ = [0.01, 0.002] m, previous command 0.009 m³/s (v = −0.001), reading 0.2525 m, L = [0.2393, 0.1056]:
   predicted tank 2 = 0.04·0.01 + 0.96·0.002 = 0.00232 m, innovation = 0.0025 − 0.00232 = 0.00018 m,
   x̂1 = 0.96·0.01 + 2·(−0.001) + 0.2393·0.00018 = 0.007643074 m and x̂2 = 0.00232 + 0.1056·0.00018 = 0.002339008 m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.00018, ACTUAL := Innovation(ReadingM := LREAL#0.2525, X1Hat := LREAL#0.01, X2Hat := LREAL#0.002), DELTA := LREAL#1.0e-15);
ASSERT_NEAR(EXPECTED := LREAL#0.007643074, ACTUAL := CorrectedTank1(X1Hat := LREAL#0.01, CommandDeviationM3PerS := LREAL#-0.001, InnovationM := LREAL#0.00018, L1 := LREAL#0.2393), DELTA := LREAL#1.0e-15);
ASSERT_NEAR(EXPECTED := LREAL#0.002339008, ACTUAL := CorrectedTank2(X1Hat := LREAL#0.01, X2Hat := LREAL#0.002, InnovationM := LREAL#0.00018, L2 := LREAL#0.1056), DELTA := LREAL#1.0e-15);
END_TEST_PROGRAM

TEST_PROGRAM TheFeedbackRow
(* Lesson 80's 20-step gain K0 = [0.07874026, 0.02387430] m²/s at x = [0.02, 0] with the setpoint at the operating level:
   q = 0.010 − 0.07874026·0.02 = 0.0084251948 m³/s, whether x is measured or estimated. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0084251948, ACTUAL := FeedbackFlowM3PerS(Deviation1M := LREAL#0.02, Deviation2M := LREAL#0.0, SetpointM := LREAL#0.25, K1 := LREAL#0.07874026, K2 := LREAL#0.02387430, N := LREAL#0.12261456), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheCosts
(* Lesson 80's scales: x = [0.02, 0] costs 400·0.02² = 0.16 and a correction of 0.005 m³/s costs 1. *)
ASSERT_NEAR(EXPECTED := LREAL#0.16, ACTUAL := LevelCost(Tank1M := LREAL#0.27, Tank2M := LREAL#0.25), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := EffortCost(FlowM3PerS := LREAL#0.015), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFlowLimitsAndTheSetpointGuard
(* The pump delivers 0–0.020 m³/s; a setpoint outside 0.05–0.45 m, such as an unwritten 0, means 0.25 m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#-0.005), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.020, ACTUAL := LimitedFlowM3PerS(RequestedM3PerS := LREAL#0.025), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := ValidSetpointM(SetpointM := LREAL#0.0), DELTA := LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM TheReadingNoise
(* Lesson 79's sequence: 0.005629397 m, then −0.0061484094 m, and again 0.005629397 m after a restart. *)
VAR
    Noise : ReadingNoise;
END_VAR
Noise();
ASSERT_NEAR(EXPECTED := LREAL#0.005629397, ACTUAL := Noise.NoiseM, DELTA := LREAL#1.0e-9);
Noise();
ASSERT_NEAR(EXPECTED := LREAL#-0.0061484094, ACTUAL := Noise.NoiseM, DELTA := LREAL#1.0e-9);
Noise(Restart := TRUE);
ASSERT_NEAR(EXPECTED := LREAL#0.005629397, ACTUAL := Noise.NoiseM, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
