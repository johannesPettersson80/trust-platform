(* @trustlsp:runtime-ignore — worked model calculations, not measurements.
   Run with ST: Run All Tests in the created lesson project. *)
TEST_PROGRAM Lesson25EarlyAndLateSeparation
VAR
    CandidateAC : LREAL;
    CandidateBC : LREAL;
    SeparationC : LREAL;
    ClearlySeparated : BOOL;
    MeasurementErrorBoundC : LREAL := 0.05;
    TimeAfterDelayS : LREAL;
    LateObservationS : LREAL := 10.0;
    ObservationIndex : INT;
END_VAR
(* These are the two fixed first-order hypotheses from this lesson,
   separate from Control Studio's first-order/additional-lag fit comparison. *)
FOR ObservationIndex := 1 TO 2 DO
    IF ObservationIndex = 1 THEN TimeAfterDelayS := LREAL#1.0;
    ELSE TimeAfterDelayS := LateObservationS;
    END_IF;
    CandidateAC := LREAL#35.0 + LREAL#2.0
        * (LREAL#1.0 - EXP(-TimeAfterDelayS / LREAL#5.0));
    CandidateBC := LREAL#35.0 + LREAL#4.0
        * (LREAL#1.0 - EXP(-TimeAfterDelayS / LREAL#10.0));
    SeparationC := ABS(CandidateAC - CandidateBC);
    ClearlySeparated := SeparationC > LREAL#2.0 * MeasurementErrorBoundC;
    IF ObservationIndex = 1 THEN
        ASSERT_NEAR(SeparationC, LREAL#0.0181118, LREAL#0.000001);
        ASSERT_FALSE(ClearlySeparated);
    ELSE
        ASSERT_NEAR(SeparationC, LREAL#0.7991528, LREAL#0.000001);
        ASSERT_TRUE(ClearlySeparated);
    END_IF;
END_FOR;
END_TEST_PROGRAM

TEST_PROGRAM Lesson25SwappedLagsHaveTheSameMeasuredResponse
VAR
    ProcessTimeA : LREAL := 5.0;
    SensorTimeA : LREAL := 2.0;
    ProcessTimeB : LREAL := 2.0;
    SensorTimeB : LREAL := 5.0;
    TimeAfterDelayS : LREAL := 5.0;
    MeasuredRiseA : LREAL;
    MeasuredRiseB : LREAL;
END_VAR
(* Both deviations start at zero, with equal gain and delayed input.
   Equal differential-equation coefficients explain the ambiguity for
   arbitrary inputs; the held-step prediction below illustrates it. *)
ASSERT_NEAR(ProcessTimeA * SensorTimeA, LREAL#10.0, LREAL#0.000001);
ASSERT_NEAR(ProcessTimeB * SensorTimeB, LREAL#10.0, LREAL#0.000001);
ASSERT_NEAR(ProcessTimeA + SensorTimeA, LREAL#7.0, LREAL#0.000001);
ASSERT_NEAR(ProcessTimeB + SensorTimeB, LREAL#7.0, LREAL#0.000001);
MeasuredRiseA := LREAL#2.0 * (LREAL#1.0
    - (ProcessTimeA * EXP(-TimeAfterDelayS / ProcessTimeA)
       - SensorTimeA * EXP(-TimeAfterDelayS / SensorTimeA))
       / (ProcessTimeA - SensorTimeA));
MeasuredRiseB := LREAL#2.0 * (LREAL#1.0
    - (ProcessTimeB * EXP(-TimeAfterDelayS / ProcessTimeB)
       - SensorTimeB * EXP(-TimeAfterDelayS / SensorTimeB))
       / (ProcessTimeB - SensorTimeB));
ASSERT_NEAR(MeasuredRiseA, LREAL#0.8831819, LREAL#0.000001);
ASSERT_NEAR(MeasuredRiseB, MeasuredRiseA, LREAL#0.000001);
END_TEST_PROGRAM
