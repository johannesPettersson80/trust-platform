(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM PLeavesTheHeaterOffset
(* A 3 °C step on the heater, K = 0.2 °C/%: P needs Kp = 145 %/°C for an offset of 0.1 °C. *)
ASSERT_NEAR(EXPECTED := LREAL#1.043115, ACTUAL := ProportionalOffset(ReferenceChange := LREAL#3.0, ProcessGain := LREAL#0.2,
    ProportionalGain := LREAL#9.38), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#0.1, ACTUAL := ProportionalOffset(ReferenceChange := LREAL#3.0, ProcessGain := LREAL#0.2,
    ProportionalGain := LREAL#145.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheShaftNeedsDerivativeDamping
(* J = 1 kg·m², B = 1 N·m·s/rad: P keeps the decay rate at 0.5 1/s, whatever Kp; Kd = 7.27 raises it to 4.135 1/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := ShaftDecayRate(Inertia := LREAL#1.0, Friction := LREAL#1.0,
    DerivativeGain := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.135, ACTUAL := ShaftDecayRate(Inertia := LREAL#1.0, Friction := LREAL#1.0,
    DerivativeGain := LREAL#7.27), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM PdHoldsTheWeightWithAnOffset
(* The 1.962 N·m weight: PD with Kp = 16 N·m/rad sags 0.1226 rad; 0.01 rad needs Kp = 196.2 N·m/rad. *)
ASSERT_NEAR(EXPECTED := LREAL#0.122625, ACTUAL := LoadOffset(LoadTorque := LREAL#1.962, ProportionalGain := LREAL#16.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.01, ACTUAL := LoadOffset(LoadTorque := LREAL#1.962, ProportionalGain := LREAL#196.2),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
