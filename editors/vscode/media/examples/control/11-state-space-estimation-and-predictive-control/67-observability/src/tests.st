(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheUpstreamFromTheDownstreamSlope
(* ẏ = 0.04x1 − 0.04y gives x1 = y + 25ẏ: y = 0 and ẏ = 0.0008 m/s mean x1 = 0.02 m. *)
ASSERT_NEAR(EXPECTED := LREAL#0.02, ACTUAL := UpstreamFromDownstream(DeviationM := LREAL#0.0, SlopeMPerS := LREAL#0.0008), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFirstReadingOnlyPrimes
(* No reconstruction from one reading. Readings 0.25 then 0.2508 m: Δy = 0.0008 m in one second, so
   x1 = 0.0008 + 25·0.0008 = 0.0208 m and tank 1 is reconstructed at 0.2708 m. *)
VAR
    Reconstruction : UpstreamReconstruction;
END_VAR
Reconstruction(MeasuredTank2M := LREAL#0.25);
ASSERT_FALSE(IN := Reconstruction.Ready);
ASSERT_NEAR(EXPECTED := LREAL#0.25, ACTUAL := Reconstruction.Tank1M, DELTA := LREAL#1.0e-12);
Reconstruction(MeasuredTank2M := LREAL#0.2508);
ASSERT_TRUE(IN := Reconstruction.Ready);
ASSERT_NEAR(EXPECTED := LREAL#0.2708, ACTUAL := Reconstruction.Tank1M, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheNoiseAmplification
(* A reading 5 mm low, then 5 mm high: Δy = 0.010 m in one second, so x1 = 0.005 + 25·0.010 = 0.255 m although both
   tanks are at rest. *)
VAR
    Reconstruction : UpstreamReconstruction;
END_VAR
Reconstruction(MeasuredTank2M := LREAL#0.245);
Reconstruction(MeasuredTank2M := LREAL#0.255);
ASSERT_NEAR(EXPECTED := LREAL#0.505, ACTUAL := Reconstruction.Tank1M, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheErrorStatistics
(* Errors of 3 mm and −4 mm: RMS √((0.003² + 0.004²)/2) = 3.5355 mm, peak 4 mm. *)
VAR
    Statistics : ErrorStatistics;
END_VAR
Statistics(ErrorM := LREAL#0.003);
Statistics(ErrorM := LREAL#-0.004);
ASSERT_NEAR(EXPECTED := LREAL#0.0035355339, ACTUAL := Statistics.RmsM, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.004, ACTUAL := Statistics.PeakM, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
