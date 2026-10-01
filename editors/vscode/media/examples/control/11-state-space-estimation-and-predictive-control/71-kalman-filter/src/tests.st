(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheScalarUpdate
(* Prediction 10 cm with variance 4 cm², measurement 12 cm with variance 1 cm²: K = 4/(4 + 1) = 0.8 and the corrected
   estimate is 10 + 0.8·(12 − 10) = 11.6 cm. *)
ASSERT_NEAR(EXPECTED := LREAL#0.8, ACTUAL := ScalarKalmanGain(PredictionVariance := LREAL#4.0, MeasurementVariance := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#11.6, ACTUAL := LREAL#10.0 + ScalarKalmanGain(PredictionVariance := LREAL#4.0, MeasurementVariance := LREAL#1.0) * (LREAL#12.0 - LREAL#10.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheCorrectedVarianceIsSmallestAtTheKalmanGain
(* (1 − K)²P + K²R with P = 4 and R = 1: 0.8 at K = 0.8, more at K = 0.5 and at K = 1. *)
ASSERT_NEAR(EXPECTED := LREAL#0.8, ACTUAL := CorrectedVariance(PredictionVariance := LREAL#4.0, MeasurementVariance := LREAL#1.0, Gain := LREAL#0.8), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.25, ACTUAL := CorrectedVariance(PredictionVariance := LREAL#4.0, MeasurementVariance := LREAL#1.0, Gain := LREAL#0.5), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := CorrectedVariance(PredictionVariance := LREAL#4.0, MeasurementVariance := LREAL#1.0, Gain := LREAL#1.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFirstPredictionAndCorrection
(* Start at a 0.25 m reading: x̂ = [0, 0], P = diag(4e−4, 2.5e−5) m². One update with Q = diag(1e−8, 1e−8) m², R = 2.5e−5 m²
   and a 0.251 m reading: P− = Ad·P·Adᵀ + Q = [[3.6865e−4, 1.536e−5], [1.536e−5, 2.369e−5]], S = 4.869e−5,
   K = [0.3154652, 0.4865475] and x̂+ = K·0.001 m. *)
VAR
    Filter : TankKalmanFilter;
END_VAR
Filter.Start(ReadingM := LREAL#0.25);
Filter(ReadingM := LREAL#0.251, FlowDeviationM3PerS := LREAL#0.0, MeasurementVariance := LREAL#2.5e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.00036865, ACTUAL := Filter.PredictedVariance11, DELTA := LREAL#1.0e-15);
ASSERT_NEAR(EXPECTED := LREAL#1.536e-5, ACTUAL := Filter.PredictedCovariance12, DELTA := LREAL#1.0e-15);
ASSERT_NEAR(EXPECTED := LREAL#2.369e-5, ACTUAL := Filter.PredictedVariance22, DELTA := LREAL#1.0e-15);
ASSERT_NEAR(EXPECTED := LREAL#0.3154651879, ACTUAL := Filter.Gain1, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.4865475457, ACTUAL := Filter.Gain2, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.2503154652, ACTUAL := Filter.Tank1M, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.2504865475, ACTUAL := Filter.Tank2M, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.0190736587, ACTUAL := Filter.Tank1SdM, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheReadingNoise
(* x' = (8121·x + 28411) MOD 134456 from 92429 gives 110928, then 19499; the noise (2x/134456 − 1)·√3·0.005 m has a standard
   deviation of 5 mm. A restart begins the sequence again. *)
VAR
    Noise : ReadingNoise;
END_VAR
Noise();
ASSERT_NEAR(EXPECTED := LREAL#0.005629397, ACTUAL := Noise.NoiseM, DELTA := LREAL#1.0e-9);
Noise();
ASSERT_NEAR(EXPECTED := LREAL#-0.0061484094, ACTUAL := Noise.NoiseM, DELTA := LREAL#1.0e-9);
Noise(Restart := TRUE);
ASSERT_NEAR(EXPECTED := LREAL#0.005629397, ACTUAL := Noise.NoiseM, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
