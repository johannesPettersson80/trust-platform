TEST_PROGRAM control_pid_positional_pd_uses_actual_cycle_time
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.5;
Parameters.ProportionalSetpointWeight := LREAL#0.0;
Parameters.DerivativeSetpointWeight := LREAL#0.25;
Parameters.Bias := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;

Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);

Input.Setpoint := LREAL#1.0;
Input.ProcessValue := LREAL#0.2;
Input.FeedForward := LREAL#0.3;
Input.CycleTime := LREAL#0.1;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.15, ACTUAL := Output, DELTA := LREAL#3.15e-12);

Input.ProcessValue := LREAL#0.4;
Input.CycleTime := LREAL#0.2;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Output, DELTA := LREAL#2.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_switches_forms_without_resetting_nominal_history
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#0.5;
Parameters.ProportionalSetpointWeight := LREAL#0.5;
Parameters.Bias := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);

Input.Setpoint := LREAL#1.0;
Input.CycleTime := LREAL#0.1;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.05, ACTUAL := Output, DELTA := LREAL#1.05e-12);

Parameters.IntegralGain := LREAL#0.0;
Controller.Configure(Parameters := Parameters);
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);

Parameters.IntegralGain := LREAL#0.5;
Controller.Configure(Parameters := Parameters);
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.05, ACTUAL := Output, DELTA := LREAL#3.05e-12);

Parameters.ProportionalGain := LREAL#4.0;
Controller.Configure(Parameters := Parameters);
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.10, ACTUAL := Output, DELTA := LREAL#3.10e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_incremental_pi_and_pid_use_signal_differences
VAR
    PidController : Control.PID;
    PiController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#0.5;
Parameters.DerivativeGain := LREAL#0.1;
Parameters.ProportionalSetpointWeight := LREAL#0.5;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;

PidController.Configure(Parameters := Parameters);
PidController.Initialize(State := InitialState);

Input.Setpoint := LREAL#1.0;
Input.ProcessValue := LREAL#0.0;
Input.FeedForward := LREAL#0.2;
Input.CycleTime := LREAL#0.1;
Output := PidController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.25, ACTUAL := Output, DELTA := LREAL#1.25e-12);

Input.ProcessValue := LREAL#0.2;
Input.FeedForward := LREAL#0.3;
Output := PidController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.79, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.DerivativeGain := LREAL#0.0;
PiController.Configure(Parameters := Parameters);
PiController.Initialize(State := InitialState);

Input.ProcessValue := LREAL#0.0;
Input.FeedForward := LREAL#0.2;
Output := PiController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.25, ACTUAL := Output, DELTA := LREAL#1.25e-12);

Input.ProcessValue := LREAL#0.2;
Input.FeedForward := LREAL#0.3;
Output := PiController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.99, ACTUAL := Output, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
