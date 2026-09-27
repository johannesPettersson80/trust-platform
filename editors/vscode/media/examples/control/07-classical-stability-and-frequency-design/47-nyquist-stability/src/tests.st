(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson47DelayPhaseAndNyquistCountPredictTheOvenCases
VAR
    Crossover : LREAL;
END_VAR
Crossover := Lesson47GainCrossover(
    ControllerGain := LREAL#2.0, PlantGain := LREAL#1.0,
    TimeConstantSeconds := LREAL#20.0);
ASSERT_NEAR(EXPECTED := LREAL#0.0866025403784439,
    ACTUAL := Crossover, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-1.48021025308882,
    ACTUAL := Lesson47CrossoverPhase(
        AngularFrequency := Crossover,
        TimeConstantSeconds := LREAL#20.0,
        DelaySeconds := LREAL#5.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#100.666666666667,
    ACTUAL := Lesson47ProportionalEquilibrium(
        BaselineTemperature := LREAL#100.0, Setpoint := LREAL#101.0,
        ControllerGain := LREAL#2.0, PlantGain := LREAL#1.0),
    DELTA := LREAL#1.0e-9);
ASSERT_EQUAL(EXPECTED := INT#0,
    ACTUAL := Lesson47ClosedLoopUnstablePoleCount(
        OpenLoopUnstablePoles := INT#0,
        CounterClockwiseEncirclements := INT#0));
ASSERT_EQUAL(EXPECTED := INT#2,
    ACTUAL := Lesson47ClosedLoopUnstablePoleCount(
        OpenLoopUnstablePoles := INT#0,
        CounterClockwiseEncirclements := INT#-2));
END_TEST_PROGRAM
