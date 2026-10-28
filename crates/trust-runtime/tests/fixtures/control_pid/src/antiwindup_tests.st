USING Control;

TEST_PROGRAM control_pid_back_calculation_and_clamping_retain_distinct_nominal_states
VAR
    BackCalculationController : PID;
    ClampingController : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.IntegralGain := LREAL#4.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#3.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#2.0;
BackCalculationController.Configure(Parameters := Parameters);
BackCalculationController.Initialize(State := InitialState);

Input.Setpoint := LREAL#1.0;
Input.CycleTime := LREAL#1.0;
Output := BackCalculationController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);
Input.Setpoint := LREAL#-0.125;
Output := BackCalculationController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);

Parameters.TrackingTime := LREAL#1.0;
ClampingController.Configure(Parameters := Parameters);
ClampingController.Initialize(State := InitialState);
Input.Setpoint := LREAL#1.0;
Output := ClampingController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);
Input.Setpoint := LREAL#-0.125;
Output := ClampingController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := Output, DELTA := LREAL#2.5e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_combined_anti_windup_allows_integration_away_from_saturation
VAR
    Controller : PID;
    Parameters : PIDParameters;
    InitialState : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.IntegralGain := LREAL#1.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#3.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#2.0;
Parameters.AntiWindup := PIDAntiWindup#Combined;
InitialState.NominalOutput := LREAL#4.0;
InitialState.SaturatedOutput := LREAL#3.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);

Input.Setpoint := LREAL#1.0;
Input.CycleTime := LREAL#1.0;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#3.0e-12);

Input.Setpoint := LREAL#-0.75;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.75, ACTUAL := Output, DELTA := LREAL#2.75e-12);
END_TEST_PROGRAM
