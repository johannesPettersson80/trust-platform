(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM ARawDerivativeAmplifiesNoise
(* A 0.002 rad measurement change within 0.02 s reads as 0.1 rad/s; Kd = 7 turns it into 0.7 N·m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.1, ACTUAL := LREAL#0.002 / LREAL#0.02, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.7, ACTUAL := LREAL#7.0 * LREAL#0.002 / LREAL#0.02, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM FilteringTheAngleFiltersTheVelocity
VAR
    Alpha : LREAL;
    FilteredAngle : LREAL;
END_VAR
(* Tf = 0.1 s at 0.02 s: alpha = 0.02/(0.1 + 0.02) = 1/6. *)
Alpha := FilterFraction(CycleTime := LREAL#0.02, FilterTime := LREAL#0.1);
ASSERT_NEAR(EXPECTED := LREAL#1.0 / LREAL#6.0, ACTUAL := Alpha, DELTA := LREAL#1.0e-12);
(* The same 0.002 rad step in the angle moves the filtered angle 1/6 of the way. Its difference is the filtered
   velocity: omega_f = alpha · 0.1 rad/s, and D = −7 · omega_f is 1/6 of the raw 0.7 N·m. *)
FilteredAngle := FilterUpdate(PreviousFilteredValue := LREAL#0.0, RawValue := LREAL#0.002, FilterFraction := Alpha);
ASSERT_NEAR(EXPECTED := Alpha * LREAL#0.1, ACTUAL := FilteredAngle / LREAL#0.02, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.7 / LREAL#6.0, ACTUAL := LREAL#-7.0 * FilteredAngle / LREAL#0.02, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
