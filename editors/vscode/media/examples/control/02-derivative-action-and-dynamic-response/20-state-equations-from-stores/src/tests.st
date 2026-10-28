(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheRatesWithWaterStoredUpstream
(* x = [0.02, 0], tank 1 at 0.27 m and tank 2 at 0.25 m: with v = 0 the levels move at [−0.0008, 0.0008] m/s, tank 2 rising
   although its own level is at rest; with v = 0.001 at [0.0012, 0.0008] m/s. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.0008, ACTUAL := CascadeUpstreamRate(StateM := LREAL#0.02, FlowDeviationM3PerS := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0008, ACTUAL := CascadeDownstreamRate(UpstreamStateM := LREAL#0.02, DownstreamStateM := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0012, ACTUAL := CascadeUpstreamRate(StateM := LREAL#0.02, FlowDeviationM3PerS := LREAL#0.001), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheNonlinearRates
(* The balances themselves at 0.27/0.25 m with 0.010 m³/s: (0.010 − 0.020·√0.27)/0.5 = −0.00078461 m/s and +0.00078461 m/s,
   within 2 % of the model's ∓0.0008. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.00078461, ACTUAL := UpstreamLevelRate(LevelM := LREAL#0.27, InflowM3PerS := LREAL#0.010), DELTA := LREAL#1.0e-8);
ASSERT_NEAR(EXPECTED := LREAL#0.00078461, ACTUAL := DownstreamLevelRate(UpstreamLevelM := LREAL#0.27, DownstreamLevelM := LREAL#0.25), DELTA := LREAL#1.0e-8);
END_TEST_PROGRAM

TEST_PROGRAM TheModelUsesBothOldStates
(* From 0.25/0.25 m with v = 0.001 m³/s: after 1 s [0.252, 0.25] m; after 2 s, from the old [0.002, 0],
   [0.252 + 0.00192, 0.25 + 0.00008] = [0.25392, 0.25008] m. *)
VAR
    Model : CascadeModel;
END_VAR
Model.Initialize(MeasuredTank1M := LREAL#0.25, MeasuredTank2M := LREAL#0.25);
Model(FlowDeviationM3PerS := LREAL#0.001);
ASSERT_NEAR(EXPECTED := LREAL#0.252, ACTUAL := Model.Tank1M, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := Model.Tank2M, DELTA := LREAL#1.0e-12);
Model(FlowDeviationM3PerS := LREAL#0.001);
ASSERT_NEAR(EXPECTED := LREAL#0.25392, ACTUAL := Model.Tank1M, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.25008, ACTUAL := Model.Tank2M, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
