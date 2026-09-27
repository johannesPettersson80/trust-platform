(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM Lesson38IntegralPresenceSelectsTheValidRealization
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    BeforeGainChange : LREAL;
    AfterGainChange : LREAL;
END_VAR
Parameters.ProportionalGain := LREAL#4.0;
Parameters.IntegralGain := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#-10.0;
Parameters.OutputMaximum := LREAL#10.0;
Parameters.RateMinimum := LREAL#-500.0;
Parameters.RateMaximum := LREAL#500.0;
Parameters.TrackingTime := LREAL#0.2;
Parameters.AntiWindup := PIDAntiWindup#Combined;
Controller.Configure(Parameters := Parameters);
InitialState.Setpoint := LREAL#0.5;
InitialState.ProcessValue := LREAL#0.0;
Controller.Initialize(State := InitialState);
Input.Setpoint := LREAL#0.5;
Input.ProcessValue := LREAL#0.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
BeforeGainChange := Controller.Update(Input := Input);
Parameters.ProportionalGain := LREAL#8.0;
Controller.Configure(Parameters := Parameters);
AfterGainChange := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := BeforeGainChange, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := AfterGainChange, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Lesson38RejectedIncrementalPOutput(
    PreviousOutput := LREAL#1.0,
    ProportionalGain := LREAL#8.0,
    CurrentError := LREAL#0.5,
    PreviousError := LREAL#0.5
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.24, ACTUAL := Lesson38BackwardIntegralIncrement(
    IntegralGain := LREAL#0.8,
    SampleInterval := LREAL#0.02,
    CurrentError := LREAL#15.0
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
