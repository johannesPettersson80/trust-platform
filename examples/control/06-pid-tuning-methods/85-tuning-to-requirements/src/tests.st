(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheSlowestResponseThatStillSettles
(* A 3 °C step into ±0.1 °C behind a 1 s delay: ln(30) = 3.4012 time constants are needed after the delay. *)
ASSERT_NEAR(EXPECTED := LREAL#8.526409, ACTUAL := SlowestResponseTimeS(SettlingTimeS := LREAL#30.0, DelayS := LREAL#1.0,
    ReferenceChange := LREAL#3.0, SettlingBand := LREAL#0.1), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#4.116197, ACTUAL := SlowestResponseTimeS(SettlingTimeS := LREAL#15.0, DelayS := LREAL#1.0,
    ReferenceChange := LREAL#3.0, SettlingBand := LREAL#0.1), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#1.176056, ACTUAL := SlowestResponseTimeS(SettlingTimeS := LREAL#5.0, DelayS := LREAL#1.0,
    ReferenceChange := LREAL#3.0, SettlingBand := LREAL#0.1), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM TheModelRuleGainsOfTheThreeResponses
(* The heater, K = 0.2 °C/%, tau = 5 s, L = 1 s; fastest lambda = max(L, tau/3) = 5/3 s, balanced twice it, more robust four times. *)
ASSERT_NEAR(EXPECTED := LREAL#9.375, ACTUAL := ModelRuleProportionalGain(ProcessGain := LREAL#0.2, TimeConstantS := LREAL#5.0,
    DelayS := LREAL#1.0, ResponseTimeS := LREAL#5.0 / LREAL#3.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#5.769231, ACTUAL := ModelRuleProportionalGain(ProcessGain := LREAL#0.2, TimeConstantS := LREAL#5.0,
    DelayS := LREAL#1.0, ResponseTimeS := LREAL#10.0 / LREAL#3.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#3.260870, ACTUAL := ModelRuleProportionalGain(ProcessGain := LREAL#0.2, TimeConstantS := LREAL#5.0,
    DelayS := LREAL#1.0, ResponseTimeS := LREAL#20.0 / LREAL#3.0), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
