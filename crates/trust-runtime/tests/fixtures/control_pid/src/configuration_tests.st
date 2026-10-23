(* Specification 45 sections 2.4/2.5 and 3.2/3.3: complete value-copy configuration,
   retained histories, and initialization without an update. *)
USING Control;

TEST_PROGRAM control_pid_configuration_copies_parameters_and_preserves_every_history
VAR
    Controller : PID;
    Parameters : PIDParameters;
    AcceptedParameters : PIDParameters;
    InitialState : PIDInitialState;
    ZeroState : PIDInitialState;
    Observed : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

InitialState.Setpoint := LREAL#10.0;
InitialState.SetpointRate := LREAL#2.0;
InitialState.ProcessValue := LREAL#8.0;
InitialState.ProcessValueRate := LREAL#-1.0;
InitialState.FeedForward := LREAL#3.0;
InitialState.NominalOutput := LREAL#20.0;
InitialState.SaturatedOutput := LREAL#19.0;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := InitialState);
Observed := Controller.GetState();
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := Observed.Setpoint, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Observed.SetpointRate, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := Observed.ProcessValue, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-1.0, ACTUAL := Observed.ProcessValueRate, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Observed.FeedForward, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Observed.NominalOutput, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#19.0, ACTUAL := Observed.SaturatedOutput, DELTA := LREAL#1.0e-12);

Parameters.ProportionalGain := LREAL#2.0;
Parameters.IntegralGain := LREAL#0.5;
Parameters.DerivativeGain := LREAL#0.25;
Parameters.ProportionalSetpointWeight := LREAL#0.4;
Parameters.DerivativeSetpointWeight := LREAL#0.3;
Parameters.Bias := LREAL#7.0;
Parameters.OutputMinimum := LREAL#-30.0;
Parameters.OutputMaximum := LREAL#40.0;
Parameters.RateMinimum := LREAL#-4.0;
Parameters.RateMaximum := LREAL#6.0;
Parameters.TrackingTime := LREAL#2.0;
Parameters.AntiWindup := PIDAntiWindup#Combined;
Controller.Configure(Parameters := Parameters);
Observed := Controller.GetState();
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := Observed.Setpoint, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Observed.SetpointRate, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := Observed.ProcessValue, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-1.0, ACTUAL := Observed.ProcessValueRate, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Observed.FeedForward, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Observed.NominalOutput, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#19.0, ACTUAL := Observed.SaturatedOutput, DELTA := LREAL#1.0e-12);

(* Mutating the caller's record after Configure cannot alter the accepted copy. *)
AcceptedParameters := Parameters;
Parameters.ProportionalGain := LREAL#100.0;
Parameters.IntegralGain := LREAL#0.0;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#1.0;
Parameters.Bias := LREAL#-10.0;
Parameters.OutputMinimum := LREAL#-100.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-1000.0;
Parameters.RateMaximum := LREAL#1000.0;
Parameters.TrackingTime := LREAL#10.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;

Input.Setpoint := LREAL#12.0;
Input.ProcessValue := LREAL#9.0;
Input.FeedForward := LREAL#5.0;
Input.CycleTime := LREAL#0.5;
Output := Controller.Update(Input := Input);
(* 20 - 0.4 + 0.75 - 0.6 + 2 = 21.75; rate interval is [17,22]. *)
ASSERT_NEAR(EXPECTED := LREAL#21.75, ACTUAL := Output, DELTA := LREAL#1.0e-10);
Input.Setpoint := LREAL#13.0;
Input.ProcessValue := LREAL#11.0;
Input.FeedForward := LREAL#6.0;
Input.CycleTime := LREAL#0.25;
Output := Controller.Update(Input := Input);
Observed := Controller.GetState();
ASSERT_NEAR(EXPECTED := LREAL#20.75, ACTUAL := Output, DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#18.60625, ACTUAL := Observed.NominalOutput, DELTA := LREAL#1.0e-10);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := Observed.SetpointRate, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#8.0, ACTUAL := Observed.ProcessValueRate, DELTA := LREAL#1.0e-12);

Input.Mode := PIDMode#Manual;
Input.ManualOutput := LREAL#100.0;
Input.CycleTime := LREAL#10.0;
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
Input.ManualOutput := LREAL#-100.0;
Input.CycleTime := LREAL#20.0;
ASSERT_NEAR(EXPECTED := LREAL#-30.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
Input.ManualOutput := LREAL#100.0;
Input.CycleTime := LREAL#0.5;
ASSERT_NEAR(EXPECTED := LREAL#-27.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);

Controller.Initialize(State := ZeroState);
Input.Mode := PIDMode#Auto;
Input.Setpoint := LREAL#100.0;
Input.ProcessValue := LREAL#0.0;
Input.FeedForward := LREAL#0.0;
Input.CycleTime := LREAL#1.0;
Output := Controller.Update(Input := Input);
Observed := Controller.GetState();
(* Combined removes 50 of the 131.5 saturation error, then back-calculates
   with coefficient 1/2: 137.5 - 50 - (87.5 - 6)/2 = 46.75. *)
ASSERT_NEAR(EXPECTED := LREAL#6.0, ACTUAL := Output, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#46.75, ACTUAL := Observed.NominalOutput, DELTA := LREAL#1.0e-10);

(* The same complete record's Bias takes effect when integral action is zero. *)
Parameters := AcceptedParameters;
Parameters.IntegralGain := LREAL#0.0;
Controller.Configure(Parameters := Parameters);
Parameters.Bias := LREAL#700.0;
Controller.Initialize(State := ZeroState);
Input.Setpoint := LREAL#0.0;
Input.CycleTime := LREAL#2.0;
ASSERT_NEAR(EXPECTED := LREAL#7.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

(* Specification 45 sections 4.2/4.3 and 6: separate filter histories continue
   across reconfiguration, using each call's own elapsed interval. *)
TEST_PROGRAM control_low_pass_reconfiguration_preserves_both_stages_and_actual_intervals
VAR
    Filter : LowPass;
    Parameters : LowPassParameters;
END_VAR
Parameters.TimeConstant := LREAL#1.5;
Parameters.Order := FilterOrder#SecondOrder;
Filter.Configure(Parameters := Parameters);
Filter.Initialize(Input := LREAL#2.0);
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := Filter.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0), DELTA := LREAL#1.0e-12);

Parameters.TimeConstant := LREAL#0.75;
Filter.Configure(Parameters := Parameters);
Parameters.TimeConstant := LREAL#100.0;
Parameters.Order := FilterOrder#FirstOrder;
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Filter.Update(Input := LREAL#4.0, CycleTime := LREAL#0.5), DELTA := LREAL#1.0e-12);

Parameters.TimeConstant := LREAL#0.75;
Filter.Configure(Parameters := Parameters);
ASSERT_NEAR(EXPECTED := LREAL#4.25, ACTUAL := Filter.Update(Input := LREAL#5.0, CycleTime := LREAL#0.5), DELTA := LREAL#1.0e-12);
Parameters.Order := FilterOrder#SecondOrder;
Filter.Configure(Parameters := Parameters);
ASSERT_NEAR(EXPECTED := LREAL#3.8125, ACTUAL := Filter.Update(Input := LREAL#5.0, CycleTime := LREAL#0.5), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
