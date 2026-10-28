(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheObserverGains
(* det(sI − A + LC) = s² + (0.08 + l2)s + 0.0016 + 0.04(l1 + l2). Poles −0.16 and −0.20 s⁻¹ need s² + 0.36s + 0.032:
   l2 = 0.28 and l1 = 0.48 s⁻¹. Twice as fast, −0.32 and −0.40 s⁻¹: l2 = 0.64 and l1 = 2.52 s⁻¹. *)
ASSERT_NEAR(EXPECTED := LREAL#0.48, ACTUAL := ObserverGain1(Pole1 := LREAL#0.16, Pole2 := LREAL#0.20), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.28, ACTUAL := ObserverGain2(Pole1 := LREAL#0.16, Pole2 := LREAL#0.20), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.52, ACTUAL := ObserverGain1(Pole1 := LREAL#0.32, Pole2 := LREAL#0.40), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.64, ACTUAL := ObserverGain2(Pole1 := LREAL#0.32, Pole2 := LREAL#0.40), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheStartHasNoInnovation
(* The observer starts with tank 1 guessed at rest and tank 2 from its reading: the innovation is zero. *)
VAR
    Observer : TankObserver;
END_VAR
Observer.Start(ReadingM := LREAL#0.2504);
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := Observer.Tank1M, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2504, ACTUAL := Observer.Tank2M, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Observer.InnovationM, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheObserverUsesBothOldEstimates
(* From x̂ = [0, 0] with L = [0.48, 0.28] and v = 0: a reading 1 mm high gives the innovation 0.001 m and
   x̂ = [0.00048, 0.00028]. A reading 2 mm high then gives the innovation 0.002 − 0.00028 = 0.00172 m and, from the old
   [0.00048, 0.00028], x̂ = [0.00048 − 0.0000192 + 0.0008256, 0.00028 + 0.0000192 − 0.0000112 + 0.0004816]. *)
VAR
    Observer : TankObserver;
END_VAR
Observer.Start(ReadingM := LREAL#0.25);
Observer(ReadingM := LREAL#0.251, FlowDeviationM3PerS := LREAL#0.0, L1 := LREAL#0.48, L2 := LREAL#0.28);
ASSERT_NEAR(EXPECTED := LREAL#0.001, ACTUAL := Observer.InnovationM, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.25048, ACTUAL := Observer.Tank1M, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.25028, ACTUAL := Observer.Tank2M, DELTA := LREAL#1.0e-12);
Observer(ReadingM := LREAL#0.252, FlowDeviationM3PerS := LREAL#0.0, L1 := LREAL#0.48, L2 := LREAL#0.28);
ASSERT_NEAR(EXPECTED := LREAL#0.00172, ACTUAL := Observer.InnovationM, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2512864, ACTUAL := Observer.Tank1M, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2507696, ACTUAL := Observer.Tank2M, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheAppliedFlowEntersTheModel
(* With a flow deviation v the model part moves tank 1 at 2v: v = 0.001 m³/s and no innovation give x̂1 = 0.002 m. *)
VAR
    Observer : TankObserver;
END_VAR
Observer.Start(ReadingM := LREAL#0.25);
Observer(ReadingM := LREAL#0.25, FlowDeviationM3PerS := LREAL#0.001, L1 := LREAL#0.48, L2 := LREAL#0.28);
ASSERT_NEAR(EXPECTED := LREAL#0.252, ACTUAL := Observer.Tank1M, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
