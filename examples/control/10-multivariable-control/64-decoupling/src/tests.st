(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheDecouplerMap
(* M = K⁻¹·(1/9)I = [[1, −4/9], [−4/9, 1]]: requests of 5 and 0 points around 50 % give heaters of 55 % and 47.78 %. Control
   Studio's D = K⁻¹·diag(K) = (81/65)·M gives 56.23 % and 47.23 %; beyond 100 % a heater is limited. *)
ASSERT_NEAR(EXPECTED := LREAL#55.0, ACTUAL := DecoupledHeaterPct(OwnEntry := LREAL#1.0, CrossEntry := LREAL#-4.0 / LREAL#9.0, OwnRequestPct := LREAL#55.0, OtherRequestPct := LREAL#50.0) + LREAL#50.0 * LREAL#4.0 / LREAL#9.0, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#47.7777778, ACTUAL := DecoupledHeaterPct(OwnEntry := LREAL#1.0, CrossEntry := LREAL#-4.0 / LREAL#9.0, OwnRequestPct := LREAL#50.0, OtherRequestPct := LREAL#55.0) + LREAL#50.0 * LREAL#4.0 / LREAL#9.0, DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#100.0, ACTUAL := DecoupledHeaterPct(OwnEntry := LREAL#81.0 / LREAL#65.0, CrossEntry := LREAL#-36.0 / LREAL#65.0, OwnRequestPct := LREAL#120.0, OtherRequestPct := LREAL#0.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheStartingRequests
(* With Studio's D, the controllers start at v = D⁻¹·u = u·(1 + 4/9): 108.33 % for heaters at 75 %, outside 0–100 %, and
   72.22 % at 50 %. Through D, 72.22 % gives back 50 %. *)
ASSERT_NEAR(EXPECTED := LREAL#108.3333333, ACTUAL := StartingRequest1Pct(D11 := LREAL#81.0 / LREAL#65.0, D12 := LREAL#-36.0 / LREAL#65.0, D21 := LREAL#-36.0 / LREAL#65.0, D22 := LREAL#81.0 / LREAL#65.0, Heater1Pct := LREAL#75.0, Heater2Pct := LREAL#75.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#72.2222222, ACTUAL := StartingRequest1Pct(D11 := LREAL#81.0 / LREAL#65.0, D12 := LREAL#-36.0 / LREAL#65.0, D21 := LREAL#-36.0 / LREAL#65.0, D22 := LREAL#81.0 / LREAL#65.0, Heater1Pct := LREAL#50.0, Heater2Pct := LREAL#50.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#50.0, ACTUAL := DecoupledHeaterPct(OwnEntry := LREAL#81.0 / LREAL#65.0, CrossEntry := LREAL#-36.0 / LREAL#65.0, OwnRequestPct := LREAL#650.0 / LREAL#9.0, OtherRequestPct := LREAL#650.0 / LREAL#9.0), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
