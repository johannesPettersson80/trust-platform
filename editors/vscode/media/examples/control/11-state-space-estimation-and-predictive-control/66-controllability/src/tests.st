(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheCounterPulse
(* After 20 s at +0.001 m³/s tank 1 stands 0.05·(1 − e^(−0.8)) = 0.02753 m high. At −0.001 m³/s it falls towards
   −0.05 m and passes zero after ln(2 − e^(−0.8))/0.04 = 10.967 s. *)
ASSERT_NEAR(EXPECTED := LREAL#10.967194, ACTUAL := CounterPulseSeconds(PulseSeconds := LREAL#20.0), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM TheSteeringFlows
(* +0.001 m³/s in the first 20 updates, −0.001 m³/s in the next 11, then nominal flow. *)
ASSERT_NEAR(EXPECTED := LREAL#0.001, ACTUAL := SteeringFlowDeviationM3PerS(ElapsedS := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.001, ACTUAL := SteeringFlowDeviationM3PerS(ElapsedS := LREAL#19.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.001, ACTUAL := SteeringFlowDeviationM3PerS(ElapsedS := LREAL#20.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.001, ACTUAL := SteeringFlowDeviationM3PerS(ElapsedS := LREAL#30.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := SteeringFlowDeviationM3PerS(ElapsedS := LREAL#31.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheModelReachesTheSteeredState
(* Lesson 74's one-second model driven by the two pulses: after 31 updates tank 1 is 0.3 mm below rest and tank 2
   10.8 mm above it. *)
VAR
    Model : CascadeModel;
    Count : INT;
END_VAR
Model.Initialize(MeasuredTank1M := LREAL#0.25, MeasuredTank2M := LREAL#0.25);
FOR Count := 0 TO 30 DO
    Model(FlowDeviationM3PerS := SteeringFlowDeviationM3PerS(ElapsedS := INT_TO_LREAL(Count)));
END_FOR;
ASSERT_NEAR(EXPECTED := LREAL#0.2497187662, ACTUAL := Model.Tank1M, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.2607522283, ACTUAL := Model.Tank2M, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
