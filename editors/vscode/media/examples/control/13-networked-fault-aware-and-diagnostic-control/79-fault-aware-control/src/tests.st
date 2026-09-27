(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson79RedundantPressureIsolationUsesThreeNewSamples
VAR
    Supervisor : FaultAwarePumpSupervisor;
    Sample : INT;
    TimeSeconds : LREAL;
END_VAR
FOR Sample := INT#1 TO INT#3 DO
    TimeSeconds := INT_TO_LREAL(Sample) * LREAL#0.1;
    Supervisor(
        PressureASampleReceived := TRUE,
        PressureBSampleReceived := TRUE,
        LevelSampleReceived := TRUE,
        DriveFeedbackReceived := TRUE,
        PressureAQualityGood := TRUE,
        PressureBQualityGood := TRUE,
        LevelQualityGood := TRUE,
        DriveFeedbackQualityGood := TRUE,
        AcknowledgeRequest := FALSE,
        ResetApproved := FALSE,
        Setpoint := LREAL#6.0,
        LocalTimeSeconds := TimeSeconds,
        PressureASourceTime := TimeSeconds,
        PressureBSourceTime := TimeSeconds,
        LevelSourceTime := TimeSeconds,
        DriveFeedbackSourceTime := TimeSeconds,
        LevelRaw := LREAL#35.0,
        NaivePressureA := LREAL#7.0,
        PressureARaw := LREAL#7.0,
        PressureBRaw := LREAL#6.0,
        DriveFeedbackRaw := LREAL#50.0,
        NaiveEvidencePressure := LREAL#6.0);
END_FOR;
ASSERT_TRUE(IN := Supervisor.PressureAIsolated);
ASSERT_FALSE(IN := Supervisor.PressureBIsolated);
ASSERT_TRUE(IN := Supervisor.SelectedPressureSource = INT#2);
ASSERT_TRUE(IN := Supervisor.OperatingMode = INT#1);
ASSERT_NEAR(EXPECTED := LREAL#6.0,
    ACTUAL := Supervisor.SelectedPressure, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#35.0,
    ACTUAL := Supervisor.ConservativeLevel, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
