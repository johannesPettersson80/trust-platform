USING Control;

TEST_PROGRAM control_pid_manual_limits_output_but_retains_nominal_history
VAR
    Controller : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.IntegralGain := LREAL#1.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#10.0;
Parameters.RateMinimum := LREAL#-2.0;
Parameters.RateMaximum := LREAL#3.0;
Parameters.TrackingTime := LREAL#0.5;
InitialState.NominalOutput := LREAL#5.0;
InitialState.SaturatedOutput := LREAL#5.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);

Input.ManualOutput := LREAL#9.0;
Input.Mode := PIDMode#Manual;
Input.CycleTime := LREAL#0.5;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#6.5, ACTUAL := Output, DELTA := LREAL#6.5e-12);

Input.Mode := PIDMode#Auto;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := Output, DELTA := LREAL#8.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_track_preloads_only_the_incremental_form
VAR
    IntegralController : PID;
    PositionalController : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#1.0;
Parameters.ProportionalSetpointWeight := LREAL#0.5;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
IntegralController.Configure(Parameters := Parameters);
IntegralController.Initialize(State := InitialState);

Input.Setpoint := LREAL#1.0;
Input.TrackingOutput := LREAL#7.0;
Input.Mode := PIDMode#Track;
Input.CycleTime := LREAL#0.1;
Output := IntegralController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#8.1, ACTUAL := Output, DELTA := LREAL#8.1e-12);

Parameters.IntegralGain := LREAL#0.0;
Parameters.Bias := LREAL#1.0;
PositionalController.Configure(Parameters := Parameters);
PositionalController.Initialize(State := InitialState);
Output := PositionalController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);
END_TEST_PROGRAM
