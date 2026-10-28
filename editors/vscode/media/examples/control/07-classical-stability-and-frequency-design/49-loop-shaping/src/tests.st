(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheLeadElement
(* Tz = 10 s, Tp = 2 s: |F| = √2/√1.04 = 1.38675 at 33.69° for 0.1 rad/s, √101/√5 = 4.49444 at 20.85° for 1 rad/s,
   and towards Tz/Tp = 5 at high frequency. *)
ASSERT_NEAR(EXPECTED := LREAL#1.38675, ACTUAL := LeadMagnitude(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.1), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.588003, ACTUAL := LeadPhaseRad(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.1), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#4.49444, ACTUAL := LeadMagnitude(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#1.0), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.363978, ACTUAL := LeadPhaseRad(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#1.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#5.0, ACTUAL := LeadMagnitude(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#1000.0), DELTA := LREAL#1.0e-4);
(* Tz = 3 s: at most 1.5 times at high frequency. *)
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := LeadMagnitude(ZeroTimeS := LREAL#3.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#1000.0), DELTA := LREAL#1.0e-4);
END_TEST_PROGRAM

TEST_PROGRAM TheFirstShapedRequest
VAR
    Controller : ShapedPi;
END_VAR
(* A 3 °C error at the first update: P = 15, I = 75.3, w = 15.3. Tz = 10 s requests 75 + 5·15.3 = 151.5 %, is limited
   to 100 % and tracks I to 75.3 + 0.1·(100 − 151.5)/5 = 74.27 %. *)
Controller(Enable := TRUE, SetpointC := LREAL#38.0, MeasuredC := LREAL#35.0, ZeroTimeS := LREAL#10.0);
ASSERT_NEAR(EXPECTED := LREAL#151.5, ACTUAL := Controller.RequestPct, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#100.0, ACTUAL := Controller.CommandPct, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#74.27, ACTUAL := Controller.IntegralPct, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheCompleteLoop
(* L = C·F·G = 0.2·F(s)·e^(−s)/s, so |L| = 0.2·|F|/ω and ∠L = −90° − ω·180°/π + ∠F. At the crossover |L| = 1, with the
   phase margin 180° + ∠L: F = 1 at 0.2 rad/s with 78.54°, Tz = 3 s at 0.219232 rad/s with 87.10°, Tz = 10 s at
   0.873558 rad/s with 63.20°. *)
VAR
    DegreesPerRad : LREAL := 57.29577951308232;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := LREAL#0.2 * LeadMagnitude(ZeroTimeS := LREAL#2.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.2) / LREAL#0.2, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#78.5408, ACTUAL := LREAL#90.0 - DegreesPerRad * LREAL#0.2 + DegreesPerRad * LeadPhaseRad(ZeroTimeS := LREAL#2.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := LREAL#0.2 * LeadMagnitude(ZeroTimeS := LREAL#3.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.219232) / LREAL#0.219232, DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#87.096, ACTUAL := LREAL#90.0 - DegreesPerRad * LREAL#0.219232 + DegreesPerRad * LeadPhaseRad(ZeroTimeS := LREAL#3.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.219232), DELTA := LREAL#1.0e-3);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := LREAL#0.2 * LeadMagnitude(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.873558) / LREAL#0.873558, DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#63.204, ACTUAL := LREAL#90.0 - DegreesPerRad * LREAL#0.873558 + DegreesPerRad * LeadPhaseRad(ZeroTimeS := LREAL#10.0, PoleTimeS := LREAL#2.0, OmegaRadS := LREAL#0.873558), DELTA := LREAL#1.0e-3);
END_TEST_PROGRAM
