(* For one directly measured quantity: the gain that minimises the corrected variance. *)
FUNCTION ScalarKalmanGain : LREAL
VAR_INPUT
    PredictionVariance : LREAL;
    MeasurementVariance : LREAL;
END_VAR
ScalarKalmanGain := PredictionVariance / (PredictionVariance + MeasurementVariance);
END_FUNCTION

(* The variance of x̂⁺ = x̂⁻ + K(z − x̂⁻) for independent unbiased errors: (1 − K)²P⁻ + K²R. *)
FUNCTION CorrectedVariance : LREAL
VAR_INPUT
    PredictionVariance : LREAL;
    MeasurementVariance : LREAL;
    Gain : LREAL;
END_VAR
CorrectedVariance := (LREAL#1.0 - Gain) * (LREAL#1.0 - Gain) * PredictionVariance + Gain * Gain * MeasurementVariance;
END_FUNCTION

(* The Kalman filter of the cascade, once per second from the old estimate and covariance: Ad = [[0.96, 0], [0.04, 0.96]],
   Bd = [2, 0] (Lesson 74's one-second step), C = [0, 1], Q = diag(1e−8, 1e−8) m². Start takes tank 2 from its reading and
   guesses tank 1 at rest, with P = diag(4e−4, 2.5e−5) m²: 20 mm and 5 mm standard deviations. It reads tank 2 and the
   applied flow only. The corrected covariance uses the Joseph form (I − KC)P⁻(I − KC)ᵀ + KRKᵀ. *)
FUNCTION_BLOCK TankKalmanFilter
VAR_INPUT
    ReadingM : LREAL;
    FlowDeviationM3PerS : LREAL;
    MeasurementVariance : LREAL;
END_VAR
VAR_OUTPUT
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    PredictedTank1M : LREAL := 0.25;
    Gain1 : LREAL := 0.0;
    Gain2 : LREAL := 0.0;
    InnovationM : LREAL := 0.0;
    Tank1SdM : LREAL := 0.0;
    PredictedVariance11 : LREAL := 0.0;
    PredictedCovariance12 : LREAL := 0.0;
    PredictedVariance22 : LREAL := 0.0;
END_VAR
VAR
    X1 : LREAL := 0.0;
    X2 : LREAL := 0.0;
    P11 : LREAL := 0.0;
    P12 : LREAL := 0.0;
    P22 : LREAL := 0.0;
    Predicted1 : LREAL;
    Predicted2 : LREAL;
    InnovationVariance : LREAL;
    A11 : LREAL;
    A12 : LREAL;
    A22 : LREAL;
END_VAR
METHOD PUBLIC Start
VAR_INPUT
    ReadingM : LREAL;
END_VAR
X1 := LREAL#0.0;
X2 := ReadingM - LREAL#0.25;
P11 := LREAL#4.0e-4;
P12 := LREAL#0.0;
P22 := LREAL#2.5e-5;
InnovationM := LREAL#0.0;
Tank1M := LREAL#0.25 + X1;
Tank2M := LREAL#0.25 + X2;
PredictedTank1M := Tank1M;
Tank1SdM := SQRT(P11);
END_METHOD
(* Prediction: x̂⁻ = Ad·x̂ + Bd·v and P⁻ = Ad·P·Adᵀ + Q, every entry from the old estimate and covariance. *)
Predicted1 := LREAL#0.96 * X1 + LREAL#2.0 * FlowDeviationM3PerS;
Predicted2 := LREAL#0.04 * X1 + LREAL#0.96 * X2;
PredictedVariance11 := LREAL#0.9216 * P11 + LREAL#1.0e-8;
PredictedCovariance12 := LREAL#0.96 * (LREAL#0.04 * P11 + LREAL#0.96 * P12);
PredictedVariance22 := LREAL#0.0016 * P11 + LREAL#0.0768 * P12 + LREAL#0.9216 * P22 + LREAL#1.0e-8;
(* Correction with the reading of tank 2: S = P⁻22 + R and K = [P⁻12, P⁻22]/S. *)
InnovationM := (ReadingM - LREAL#0.25) - Predicted2;
InnovationVariance := PredictedVariance22 + MeasurementVariance;
Gain1 := PredictedCovariance12 / InnovationVariance;
Gain2 := PredictedVariance22 / InnovationVariance;
X1 := Predicted1 + Gain1 * InnovationM;
X2 := Predicted2 + Gain2 * InnovationM;
(* Joseph form with I − KC = [[1, −K1], [0, 1 − K2]]. *)
A11 := PredictedVariance11 - LREAL#2.0 * Gain1 * PredictedCovariance12 + Gain1 * Gain1 * PredictedVariance22;
A12 := (LREAL#1.0 - Gain2) * (PredictedCovariance12 - Gain1 * PredictedVariance22);
A22 := (LREAL#1.0 - Gain2) * (LREAL#1.0 - Gain2) * PredictedVariance22;
P11 := A11 + Gain1 * Gain1 * MeasurementVariance;
P12 := A12 + Gain1 * Gain2 * MeasurementVariance;
P22 := A22 + Gain2 * Gain2 * MeasurementVariance;
Tank1M := LREAL#0.25 + X1;
Tank2M := LREAL#0.25 + X2;
PredictedTank1M := LREAL#0.25 + Predicted1;
Tank1SdM := SQRT(P11);
END_FUNCTION_BLOCK

(* Reading noise with a 5 mm standard deviation: x' = (8121·x + 28411) MOD 134456, uniform on ±√3·5 mm. Restart begins the
   sequence again. *)
FUNCTION_BLOCK ReadingNoise
VAR_INPUT
    Restart : BOOL;
END_VAR
VAR_OUTPUT
    NoiseM : LREAL := 0.0;
END_VAR
VAR
    State : DINT := 92429;
END_VAR
IF Restart THEN
    State := DINT#92429;
END_IF;
State := (DINT#8121 * State + DINT#28411) MOD DINT#134456;
NoiseM := (LREAL#2.0 * DINT_TO_LREAL(State) / LREAL#134456.0 - LREAL#1.0) * SQRT(LREAL#3.0) * LREAL#0.005;
END_FUNCTION_BLOCK

(* Root-mean-square and peak of the errors so far, as in Lesson 76. *)
FUNCTION_BLOCK ErrorStatistics
VAR_INPUT
    ErrorM : LREAL;
END_VAR
VAR_OUTPUT
    RmsM : LREAL := 0.0;
    PeakM : LREAL := 0.0;
END_VAR
VAR
    Count : LREAL := 0.0;
    SumOfSquares : LREAL := 0.0;
END_VAR
Count := Count + LREAL#1.0;
SumOfSquares := SumOfSquares + ErrorM * ErrorM;
RmsM := SQRT(SumOfSquares / Count);
PeakM := MAX(PeakM, ABS(ErrorM));
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* One cascade and one Kalman filter of its tank 1. AssumedReadingSdM, the filter's assumed standard deviation of the
       tank-2 reading, is a Control Studio experiment setting: R = AssumedReadingSdM². The project experiment restarts the
       reading noise, so every run reads the same noise, pours a slug into tank 1, 0.020 m³/s for one second, and starts
       the filter one update later. Tank 1 is read only to score the estimate, from 31 s after the start. ClaimedRmsM is
       the RMS error the filter claims over the same updates: the root mean square of its own standard deviation. *)
    Filter : TankKalmanFilter;
    Noise : ReadingNoise;
    Errors : ErrorStatistics;
    Claimed : ErrorStatistics;
    AssumedReadingSdM : LREAL := 0.005;
    PumpM3PerS : LREAL := 0.010;
    Tank1M : LREAL := 0.25;
    Tank2M : LREAL := 0.25;
    ReadingM : LREAL := 0.25;
    PredictedTank1M : LREAL := 0.25;
    CorrectedTank1M : LREAL := 0.25;
    CorrectedTank2M : LREAL := 0.25;
    FilterSdTank1M : LREAL := 0.0;
    RmsM : LREAL := 0.0;
    ClaimedRmsM : LREAL := 0.0;
    PeakM : LREAL := 0.0;
    GainTank1 : LREAL := 0.0;
    InnovationM : LREAL := 0.0;
    ElapsedS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    FaultLatched : BOOL := FALSE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
(* The prediction covers one second, so an interval outside 1 s ± 50 ms makes it stale, and a level outside 0.05–0.45 m
   is invalid: either latches zero flow and freezes the filter. *)
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR Tank1M < LREAL#0.05 OR Tank1M > LREAL#0.45 OR Tank2M < LREAL#0.05 OR Tank2M > LREAL#0.45 THEN
    FaultLatched := TRUE;
END_IF;
Noise(Restart := ExperimentEnabled AND NOT WasEnabled);
ReadingM := Tank2M + Noise.NoiseM;
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled AND NOT FaultLatched THEN
    ElapsedS := ElapsedS + LREAL#1.0;
    IF ElapsedS < LREAL#1.5 THEN
        Filter.Start(ReadingM := ReadingM);
    ELSE
        (* The applied flow deviation of the second that has passed: the pump runs 0.010 m³/s after the slug. *)
        Filter(ReadingM := ReadingM, FlowDeviationM3PerS := PumpM3PerS - LREAL#0.010,
            MeasurementVariance := AssumedReadingSdM * AssumedReadingSdM);
    END_IF;
    IF ElapsedS >= LREAL#30.5 THEN
        Errors(ErrorM := Filter.Tank1M - Tank1M);
        Claimed(ErrorM := Filter.Tank1SdM);
    END_IF;
END_IF;
PumpM3PerS := LREAL#0.010;
IF ExperimentEnabled AND ElapsedS < LREAL#0.5 THEN
    PumpM3PerS := LREAL#0.020;
END_IF;
IF FaultLatched THEN
    PumpM3PerS := LREAL#0.0;
END_IF;
PredictedTank1M := Filter.PredictedTank1M;
CorrectedTank1M := Filter.Tank1M;
CorrectedTank2M := Filter.Tank2M;
FilterSdTank1M := Filter.Tank1SdM;
GainTank1 := Filter.Gain1;
InnovationM := Filter.InnovationM;
RmsM := Errors.RmsM;
ClaimedRmsM := Claimed.RmsM;
PeakM := Errors.PeakM;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PumpM3PerS AT %QL0 : LREAL;
    P1.Tank1M AT %IL0 : LREAL;
    P1.Tank2M AT %IL8 : LREAL;
    P1.ReadingM AT %ML0 : LREAL;
    P1.PredictedTank1M AT %ML8 : LREAL;
    P1.CorrectedTank1M AT %ML16 : LREAL;
    P1.CorrectedTank2M AT %ML24 : LREAL;
    P1.FilterSdTank1M AT %ML32 : LREAL;
    P1.RmsM AT %ML40 : LREAL;
    P1.PeakM AT %ML48 : LREAL;
    P1.GainTank1 AT %ML56 : LREAL;
    P1.InnovationM AT %ML64 : LREAL;
    P1.ClaimedRmsM AT %ML80 : LREAL;
    P1.ExperimentEnabled AT %MX72.0 : BOOL;
    P1.ClockValid AT %MX72.1 : BOOL;
    P1.FaultLatched AT %MX72.2 : BOOL;
END_VAR
END_CONFIGURATION
