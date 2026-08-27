USING Control;

TEST_PROGRAM control_pid_feedforward_is_positional_incremental_and_manual_history_aware
VAR
    PositionalController : PID;
    IncrementalController : PID;
    ManualController : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;

PositionalController.Configure(Parameters := Parameters);
PositionalController.Initialize(State := InitialState);
Input.FeedForward := LREAL#2.0;
Input.CycleTime := LREAL#0.1;
Output := PositionalController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Output, DELTA := LREAL#2.0e-12);

Parameters.IntegralGain := LREAL#1.0;
InitialState.FeedForward := LREAL#1.0;
InitialState.NominalOutput := LREAL#10.0;
InitialState.SaturatedOutput := LREAL#10.0;
IncrementalController.Configure(Parameters := Parameters);
IncrementalController.Initialize(State := InitialState);
Input.FeedForward := LREAL#3.0;
Output := IncrementalController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := Output, DELTA := LREAL#12.0e-12);

InitialState.FeedForward := LREAL#1.0;
InitialState.NominalOutput := LREAL#0.0;
InitialState.SaturatedOutput := LREAL#0.0;
ManualController.Configure(Parameters := Parameters);
ManualController.Initialize(State := InitialState);
Input.FeedForward := LREAL#5.0;
Input.ManualOutput := LREAL#7.0;
Input.Mode := PIDMode#Manual;
Output := ManualController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#7.0, ACTUAL := Output, DELTA := LREAL#7.0e-12);
Input.Mode := PIDMode#Auto;
Output := ManualController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#7.0, ACTUAL := Output, DELTA := LREAL#7.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_setpoint_weights_affect_only_their_paper_terms
VAR
    ControllerA : PID;
    ControllerB : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#1.0;
Parameters.ProportionalSetpointWeight := LREAL#0.25;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
ControllerA.Configure(Parameters := Parameters);
ControllerA.Initialize(State := InitialState);
Input.Setpoint := LREAL#2.0;
Input.ProcessValue := LREAL#0.5;
Input.CycleTime := LREAL#0.1;
Output := ControllerA.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.15, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.ProportionalSetpointWeight := LREAL#0.75;
ControllerB.Configure(Parameters := Parameters);
ControllerB.Initialize(State := InitialState);
Output := ControllerB.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.15, ACTUAL := Output, DELTA := LREAL#2.15e-12);

Parameters.IntegralGain := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#0.0;
ControllerA.Configure(Parameters := Parameters);
ControllerA.Initialize(State := InitialState);
Output := ControllerA.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);

Parameters.ProportionalGain := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.5;
Parameters.DerivativeSetpointWeight := LREAL#0.25;
ControllerA.Configure(Parameters := Parameters);
ControllerA.Initialize(State := InitialState);
Output := ControllerA.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.DerivativeSetpointWeight := LREAL#0.0;
ControllerB.Configure(Parameters := Parameters);
ControllerB.Initialize(State := InitialState);
Output := ControllerB.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-2.5, ACTUAL := Output, DELTA := LREAL#2.5e-12);

Parameters.IntegralGain := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.25;
ControllerA.Configure(Parameters := Parameters);
ControllerA.Initialize(State := InitialState);
Output := ControllerA.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.15, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.DerivativeSetpointWeight := LREAL#0.0;
ControllerB.Configure(Parameters := Parameters);
ControllerB.Initialize(State := InitialState);
Output := ControllerB.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-2.35, ACTUAL := Output, DELTA := LREAL#2.35e-12);
END_TEST_PROGRAM
