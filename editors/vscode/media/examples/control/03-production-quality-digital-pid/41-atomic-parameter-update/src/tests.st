(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM Lesson41EveryScanUsesOneCompleteParameterRevision
ASSERT_NEAR(EXPECTED := LREAL#5.1,
    ACTUAL := Lesson41PiFeedforwardIncrement(
        ProportionalGain := LREAL#5.0,
        IntegralGain := LREAL#1.0,
        CycleTime := LREAL#0.1,
        CurrentError := LREAL#1.0,
        PreviousError := LREAL#0.0,
        FeedForwardChange := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.3,
    ACTUAL := Lesson41PiFeedforwardIncrement(
        ProportionalGain := LREAL#15.0,
        IntegralGain := LREAL#3.0,
        CycleTime := LREAL#0.1,
        CurrentError := LREAL#1.0,
        PreviousError := LREAL#0.0,
        FeedForwardChange := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.1,
    ACTUAL := Lesson41PiFeedforwardIncrement(
        ProportionalGain := LREAL#15.0,
        IntegralGain := LREAL#1.0,
        CycleTime := LREAL#0.1,
        CurrentError := LREAL#1.0,
        PreviousError := LREAL#0.0,
        FeedForwardChange := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM Lesson41RejectsInvalidPidParameterRecordsBeforePublication
VAR
    Candidate : Control.PIDParameters;
    Active : Control.PIDParameters;
    Controller : Control.PID;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    ActiveRevision : DINT;
    Accepted : BOOL;
END_VAR

Candidate.ProportionalGain := LREAL#5.0;
Candidate.IntegralGain := LREAL#1.0;
Candidate.DerivativeGain := LREAL#0.0;
Candidate.ProportionalSetpointWeight := LREAL#1.0;
Candidate.DerivativeSetpointWeight := LREAL#0.0;
Candidate.Bias := LREAL#0.0;
Candidate.OutputMinimum := LREAL#0.0;
Candidate.OutputMaximum := LREAL#100.0;
Candidate.RateMinimum := LREAL#-20.0;
Candidate.RateMaximum := LREAL#20.0;
Candidate.TrackingTime := LREAL#1.0;
Candidate.AntiWindup := PIDAntiWindup#Combined;
ASSERT_TRUE(IN := Lesson41PidParametersValid(Parameters := Candidate));

Candidate.OutputMinimum := LREAL#80.0;
Candidate.OutputMaximum := LREAL#20.0;
ASSERT_FALSE(IN := Lesson41PidParametersValid(Parameters := Candidate));

Candidate.OutputMinimum := LREAL#0.0;
Candidate.OutputMaximum := LREAL#100.0;
Candidate.ProportionalSetpointWeight := LREAL#1.5;
ASSERT_FALSE(IN := Lesson41PidParametersValid(Parameters := Candidate));

Candidate.ProportionalSetpointWeight := LREAL#1.0;
Candidate.TrackingTime := LREAL#0.0;
ASSERT_FALSE(IN := Lesson41PidParametersValid(Parameters := Candidate));

Candidate.TrackingTime := LREAL#1.0;
Candidate.ProportionalGain := LREAL#1.0e101;
ASSERT_FALSE(IN := Lesson41PidParametersValid(Parameters := Candidate));

Active := Candidate;
Active.ProportionalGain := LREAL#5.0;
Active.IntegralGain := LREAL#1.0;
Active.OutputMinimum := LREAL#0.0;
Active.OutputMaximum := LREAL#100.0;
Active.RateMinimum := LREAL#-20.0;
Active.RateMaximum := LREAL#20.0;
Active.ProportionalSetpointWeight := LREAL#1.0;
Active.TrackingTime := LREAL#1.0;
ActiveRevision := DINT#7;
Controller.Configure(Parameters := Active);
State.NominalOutput := LREAL#40.0;
State.SaturatedOutput := LREAL#40.0;
Controller.Initialize(State := State);
Input.Setpoint := LREAL#7.0;
Input.ProcessValue := LREAL#6.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
Candidate := Active;
Candidate.OutputMinimum := LREAL#80.0;
Candidate.OutputMaximum := LREAL#20.0;
Accepted := Lesson41PidParametersValid(Parameters := Candidate);
IF Accepted THEN
    Active := Candidate;
    ActiveRevision := ActiveRevision + DINT#1;
    Controller.Configure(Parameters := Active);
END_IF;
ASSERT_FALSE(IN := Accepted);
ASSERT_EQUAL(EXPECTED := DINT#7, ACTUAL := ActiveRevision);
ASSERT_NEAR(EXPECTED := LREAL#5.0,
    ACTUAL := Active.ProportionalGain, DELTA := LREAL#1.0e-12);

Candidate := Active;
Candidate.ProportionalGain := LREAL#6.0;
Accepted := Lesson41PidParametersValid(Parameters := Candidate);
IF Accepted THEN
    Active := Candidate;
    ActiveRevision := ActiveRevision + DINT#1;
    Controller.Configure(Parameters := Active);
END_IF;
ASSERT_TRUE(IN := Accepted);
ASSERT_EQUAL(EXPECTED := DINT#8, ACTUAL := ActiveRevision);
ASSERT_NEAR(EXPECTED := LREAL#6.0,
    ACTUAL := Active.ProportionalGain, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
