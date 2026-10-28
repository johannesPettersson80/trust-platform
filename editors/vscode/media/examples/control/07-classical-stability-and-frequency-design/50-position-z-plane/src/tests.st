(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheHeldShaftUpdate
(* a = e^(−h), b = d = 1 − e^(−h), c = h − 1 + e^(−h) for h = 0.02 s and 0.6 s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.9801986733, ACTUAL := HeldVelocityFactor(PeriodS := LREAL#0.02), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.0198013267, ACTUAL := HeldVelocityGain(PeriodS := LREAL#0.02), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.0001986733, ACTUAL := HeldAngleGain(PeriodS := LREAL#0.02), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.5488116361, ACTUAL := HeldVelocityFactor(PeriodS := LREAL#0.6), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.4511883639, ACTUAL := HeldVelocityGain(PeriodS := LREAL#0.6), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.1488116361, ACTUAL := HeldAngleGain(PeriodS := LREAL#0.6), DELTA := LREAL#1.0e-10);
END_TEST_PROGRAM

TEST_PROGRAM TheSampledLoopPolynomial
(* h = 0.02 s: the cubic is −0.1354516352 at z = 0 and changes sign between 0 and 0.2, 0.8 and 0.9, 0.9 and 1: three
   real roots inside the unit circle. h = 0.6 s: it is −1.2078964313 at z = 0, so the product of the roots is
   1.2078964313 > 1 and at least one root lies outside the unit circle. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.1354516352, ACTUAL := SampledLoopPolynomial(PeriodS := LREAL#0.02, ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0, Z := LREAL#0.0), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.0177377645, ACTUAL := SampledLoopPolynomial(PeriodS := LREAL#0.02, ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0, Z := LREAL#0.2), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.0068106003, ACTUAL := SampledLoopPolynomial(PeriodS := LREAL#0.02, ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0, Z := LREAL#0.8), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#-0.0006245008, ACTUAL := SampledLoopPolynomial(PeriodS := LREAL#0.02, ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0, Z := LREAL#0.9), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#0.0063364245, ACTUAL := SampledLoopPolynomial(PeriodS := LREAL#0.02, ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0, Z := LREAL#1.0), DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#-1.2078964313, ACTUAL := SampledLoopPolynomial(PeriodS := LREAL#0.6, ProportionalGain := LREAL#16.0, VelocityGain := LREAL#7.0, Z := LREAL#0.0), DELTA := LREAL#1.0e-10);
END_TEST_PROGRAM
