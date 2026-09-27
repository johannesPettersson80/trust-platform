(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM Lesson40TrackingAlignsTheFirstAutoRequest
VAR
    TrackedController : Control.PID;
    UntrackedController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    TrackRequest : LREAL;
    TrackedAutoRequest : LREAL;
    UntrackedAutoRequest : LREAL;
END_VAR
Parameters.ProportionalGain := LREAL#15.0;
Parameters.IntegralGain := LREAL#3.0;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#Combined;
TrackedController.Configure(Parameters := Parameters);
UntrackedController.Configure(Parameters := Parameters);
InitialState.Setpoint := LREAL#6.0;
InitialState.ProcessValue := LREAL#6.0;
InitialState.NominalOutput := LREAL#40.0;
InitialState.SaturatedOutput := LREAL#40.0;
TrackedController.Initialize(State := InitialState);
UntrackedController.Initialize(State := InitialState);
Input.Setpoint := LREAL#6.0;
Input.ProcessValue := LREAL#6.0;
Input.CycleTime := LREAL#0.1;
Input.Mode := PIDMode#Track;
Input.TrackingOutput := LREAL#60.0;
TrackRequest := TrackedController.Update(Input := Input);
Input.Mode := PIDMode#Auto;
TrackedAutoRequest := TrackedController.Update(Input := Input);
UntrackedAutoRequest := UntrackedController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#60.0, ACTUAL := TrackRequest, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#60.0, ACTUAL := TrackedAutoRequest, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := UntrackedAutoRequest, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Lesson40TransferStep(
    FirstNewRequest := TrackedAutoRequest,
    LastAppliedOutput := LREAL#60.0
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Lesson40TransferStep(
    FirstNewRequest := UntrackedAutoRequest,
    LastAppliedOutput := LREAL#60.0
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
