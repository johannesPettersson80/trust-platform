USING Control;

TEST_PROGRAM control_smith_predictor_models_delay_and_anchors_prediction_to_measurement
VAR
    Predictor : SmithPredictorFOPDT;
    Parameters : SmithPredictorParameters;
    State : SmithPredictorInitialState;
    Input : SmithPredictorInput;
    Output : SmithPredictorOutput;
    Accepted : BOOL;
END_VAR
Parameters.ProcessGain := LREAL#2.0;
Parameters.TimeConstant := LREAL#1.0;
Parameters.DeadTime := LREAL#1.0;
Parameters.OperatingOutput := LREAL#10.0;
Parameters.MaximumCorrection := LREAL#0.5;
Accepted := Predictor.Configure(Parameters := Parameters);
ASSERT_TRUE(Accepted);
State.ModelOutput := LREAL#10.0;
State.ProcessValue := LREAL#10.0;
Predictor.Initialize(State := State);
Input.ManipulatedValue := LREAL#1.0;
Input.ProcessValue := LREAL#10.0;
Input.CycleTime := LREAL#0.5;
Input.Enabled := TRUE;
Output := Predictor.Update(Input := Input);
ASSERT_TRUE(Output.Active);
ASSERT_TRUE(Output.HistoryComplete);
ASSERT_NEAR(EXPECTED := LREAL#10.8, ACTUAL := Output.DelayFreeModel, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := Output.DelayedModel, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.8, ACTUAL := Output.PredictedProcessValue, DELTA := LREAL#1.0e-12);

Input.ProcessValue := LREAL#12.0;
Output := Predictor.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := Output.Correction, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#11.78, ACTUAL := Output.PredictedProcessValue, DELTA := LREAL#1.0e-12);

Input.Enabled := FALSE;
Input.ProcessValue := LREAL#9.25;
Output := Predictor.Update(Input := Input);
ASSERT_FALSE(Output.Active);
ASSERT_NEAR(EXPECTED := LREAL#9.25, ACTUAL := Output.PredictedProcessValue, DELTA := LREAL#1.0e-12);

Parameters.TimeConstant := LREAL#0.0;
Accepted := Predictor.Configure(Parameters := Parameters);
ASSERT_FALSE(Accepted);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_form_conversion_is_explicit_reversible_and_fail_closed
VAR
    Input : PIDFormParameters;
    Result : PIDFormConversionResult;
    Parallel : PIDFormParameters;
END_VAR

Input.Form := PIDParameterForm#Ideal;
Input.Proportional := LREAL#2.0;
Input.Integral := LREAL#10.0;
Input.Derivative := LREAL#0.5;
Result := ConvertPIDForm(Input := Input, Target := PIDParameterForm#Parallel);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#Valid);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Result.Parameters.Proportional, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2, ACTUAL := Result.Parameters.Integral, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Result.Parameters.Derivative, DELTA := LREAL#1.0e-12);

Input.Form := PIDParameterForm#Series;
Result := ConvertPIDForm(Input := Input, Target := PIDParameterForm#Parallel);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#Valid);
ASSERT_NEAR(EXPECTED := LREAL#2.1, ACTUAL := Result.Parameters.Proportional, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2, ACTUAL := Result.Parameters.Integral, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Result.Parameters.Derivative, DELTA := LREAL#1.0e-12);
Parallel := Result.Parameters;
Result := ConvertPIDForm(Input := Parallel, Target := PIDParameterForm#Series);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#Valid);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Result.Parameters.Proportional, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := Result.Parameters.Integral, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := Result.Parameters.Derivative, DELTA := LREAL#1.0e-12);

Parallel.Form := PIDParameterForm#Parallel;
Parallel.Proportional := LREAL#-2.0;
Parallel.Integral := LREAL#-0.2;
Parallel.Derivative := LREAL#-1.0;
Result := ConvertPIDForm(Input := Parallel, Target := PIDParameterForm#Ideal);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#Valid);
ASSERT_NEAR(EXPECTED := LREAL#-2.0, ACTUAL := Result.Parameters.Proportional, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := Result.Parameters.Integral, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := Result.Parameters.Derivative, DELTA := LREAL#1.0e-12);

Result := ConvertPIDForm(Input := Parallel, Target := PIDParameterForm#Parallel);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#Valid);
ASSERT_NEAR(EXPECTED := Parallel.Proportional, ACTUAL := Result.Parameters.Proportional, DELTA := LREAL#1.0e-12);

Input.Form := PIDParameterForm#Ideal;
Input.Proportional := LREAL#2.0;
Input.Integral := LREAL#0.0;
Input.Derivative := LREAL#0.5;
Result := ConvertPIDForm(Input := Input, Target := PIDParameterForm#Parallel);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#InvalidDomain);

Parallel.Form := PIDParameterForm#Parallel;
Parallel.Proportional := LREAL#1.0;
Parallel.Integral := LREAL#1.0;
Parallel.Derivative := LREAL#1.0;
Result := ConvertPIDForm(Input := Parallel, Target := PIDParameterForm#Series);
ASSERT_TRUE(Result.Status = PIDFormConversionStatus#NonRepresentable);
END_TEST_PROGRAM

TEST_PROGRAM control_smith_predictor_initialization_sets_the_correction_from_the_measurement
VAR
    Predictor : SmithPredictorFOPDT;
    State : SmithPredictorInitialState;
    Input : SmithPredictorInput;
    Output : SmithPredictorOutput;
END_VAR
(* Spec 37 §2.3: Initialize sets the initial correction from ProcessValue - ModelOutput. Before
   configuration no model interval runs, so the update reports that correction while it returns
   the measurement. *)
State.ModelOutput := LREAL#10.0;
State.ProcessValue := LREAL#12.0;
Predictor.Initialize(State := State);
Input.ProcessValue := LREAL#12.0;
Input.CycleTime := LREAL#0.5;
Input.Enabled := TRUE;
Output := Predictor.Update(Input := Input);
ASSERT_FALSE(Output.Active);
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := Output.PredictedProcessValue, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Output.Correction, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_smith_predictor_zero_delay_reduces_to_measurement
VAR
    Predictor : SmithPredictorFOPDT;
    Parameters : SmithPredictorParameters;
    State : SmithPredictorInitialState;
    Input : SmithPredictorInput;
    Output : SmithPredictorOutput;
END_VAR
Parameters.ProcessGain := LREAL#1.0;
Parameters.TimeConstant := LREAL#2.0;
Parameters.DeadTime := LREAL#0.0;
ASSERT_TRUE(Predictor.Configure(Parameters := Parameters));
Predictor.Initialize(State := State);
Input.ManipulatedValue := LREAL#4.0;
Input.ProcessValue := LREAL#7.25;
Input.CycleTime := LREAL#0.1;
Input.Enabled := TRUE;
Output := Predictor.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#7.25, ACTUAL := Output.PredictedProcessValue, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_gain_scheduler_interpolates_tuning_and_preserves_common_policy
VAR
    Scheduler : PIDGainScheduler;
    Parameters : PIDGainScheduleParameters;
    Output : PIDGainScheduleOutput;
    Accepted : BOOL;
END_VAR
Parameters.PointCount := DINT#3;
Parameters.Points[0].OperatingPoint := LREAL#0.0;
Parameters.Points[0].Tuning.ProportionalGain := LREAL#1.0;
Parameters.Points[0].Tuning.IntegralGain := LREAL#0.1;
Parameters.Points[0].Tuning.DerivativeGain := LREAL#0.2;
Parameters.Points[0].Tuning.ProportionalSetpointWeight := LREAL#0.3;
Parameters.Points[0].Tuning.DerivativeSetpointWeight := LREAL#0.4;
Parameters.Points[0].Tuning.TrackingTime := LREAL#1.0;
Parameters.Points[1].OperatingPoint := LREAL#10.0;
Parameters.Points[1].Tuning.ProportionalGain := LREAL#2.0;
Parameters.Points[1].Tuning.IntegralGain := LREAL#0.2;
Parameters.Points[1].Tuning.DerivativeGain := LREAL#0.4;
Parameters.Points[1].Tuning.ProportionalSetpointWeight := LREAL#0.5;
Parameters.Points[1].Tuning.DerivativeSetpointWeight := LREAL#0.6;
Parameters.Points[1].Tuning.TrackingTime := LREAL#2.0;
Parameters.Points[2].OperatingPoint := LREAL#20.0;
Parameters.Points[2].Tuning.ProportionalGain := LREAL#4.0;
Parameters.Points[2].Tuning.IntegralGain := LREAL#0.4;
Parameters.Points[2].Tuning.DerivativeGain := LREAL#0.8;
Parameters.Points[2].Tuning.ProportionalSetpointWeight := LREAL#0.7;
Parameters.Points[2].Tuning.DerivativeSetpointWeight := LREAL#0.8;
Parameters.Points[2].Tuning.TrackingTime := LREAL#4.0;
Parameters.CommonParameters.Bias := LREAL#12.0;
Parameters.CommonParameters.OutputMinimum := LREAL#0.0;
Parameters.CommonParameters.OutputMaximum := LREAL#100.0;
Parameters.CommonParameters.RateMinimum := LREAL#-5.0;
Parameters.CommonParameters.RateMaximum := LREAL#7.0;
Parameters.CommonParameters.AntiWindup := PIDAntiWindup#Combined;
Accepted := Scheduler.Configure(Parameters := Parameters);
ASSERT_TRUE(Accepted);
Scheduler.Initialize(OperatingPoint := LREAL#5.0);
Output := Scheduler.Update(OperatingPoint := LREAL#5.0, CycleTime := LREAL#0.1);
ASSERT_TRUE(Output.Valid);
ASSERT_FALSE(Output.Clamped);
ASSERT_TRUE(Output.LowerIndex = DINT#0);
ASSERT_TRUE(Output.UpperIndex = DINT#1);
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := Output.Fraction, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := Output.Parameters.ProportionalGain, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.15, ACTUAL := Output.Parameters.IntegralGain, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.3, ACTUAL := Output.Parameters.DerivativeGain, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.4, ACTUAL := Output.Parameters.ProportionalSetpointWeight, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := Output.Parameters.DerivativeSetpointWeight, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := Output.Parameters.TrackingTime, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := Output.Parameters.Bias, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#100.0, ACTUAL := Output.Parameters.OutputMaximum, DELTA := LREAL#1.0e-12);
ASSERT_TRUE(Output.Parameters.AntiWindup = PIDAntiWindup#Combined);

Output := Scheduler.Update(OperatingPoint := LREAL#-5.0, CycleTime := LREAL#0.1);
ASSERT_TRUE(Output.Clamped);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Output.Parameters.ProportionalGain, DELTA := LREAL#1.0e-12);

Parameters.Points[1].OperatingPoint := LREAL#0.0;
Accepted := Scheduler.Configure(Parameters := Parameters);
ASSERT_FALSE(Accepted);
Output := Scheduler.Update(OperatingPoint := LREAL#15.0, CycleTime := LREAL#0.1);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output.Parameters.ProportionalGain, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_gain_scheduler_filters_the_operating_point
VAR
    Scheduler : PIDGainScheduler;
    Parameters : PIDGainScheduleParameters;
    Output : PIDGainScheduleOutput;
END_VAR
Parameters.PointCount := DINT#2;
Parameters.Points[0].OperatingPoint := LREAL#0.0;
Parameters.Points[0].Tuning.ProportionalGain := LREAL#1.0;
Parameters.Points[0].Tuning.TrackingTime := LREAL#1.0;
Parameters.Points[1].OperatingPoint := LREAL#10.0;
Parameters.Points[1].Tuning.ProportionalGain := LREAL#3.0;
Parameters.Points[1].Tuning.TrackingTime := LREAL#1.0;
Parameters.CommonParameters.OutputMinimum := LREAL#-100.0;
Parameters.CommonParameters.OutputMaximum := LREAL#100.0;
Parameters.CommonParameters.RateMinimum := LREAL#-100.0;
Parameters.CommonParameters.RateMaximum := LREAL#100.0;
Parameters.FilterTimeConstant := LREAL#1.0;
ASSERT_TRUE(Scheduler.Configure(Parameters := Parameters));
Scheduler.Initialize(OperatingPoint := LREAL#0.0);
Output := Scheduler.Update(OperatingPoint := LREAL#10.0, CycleTime := LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#5.0, ACTUAL := Output.FilteredOperatingPoint, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Output.Parameters.ProportionalGain, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_static_decoupler_maps_and_recovers_two_channel_commands
VAR
    Decoupler : StaticDecoupler2x2;
    Parameters : StaticDecoupler2x2Parameters;
    VirtualCommand : ControlVector2;
    ActuatorCommand : ControlVector2;
    Recovered : ControlVector2;
    Status : StaticDecoupler2x2Status;
    Accepted : BOOL;
END_VAR
VirtualCommand.First := LREAL#3.0;
ActuatorCommand := Decoupler.ToActuators(VirtualCommand := VirtualCommand);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := ActuatorCommand.First, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := ActuatorCommand.Second, DELTA := LREAL#1.0e-12);

Parameters.GainMatrix.G11 := LREAL#2.0;
Parameters.GainMatrix.G12 := LREAL#1.0;
Parameters.GainMatrix.G21 := LREAL#1.0;
Parameters.GainMatrix.G22 := LREAL#2.0;
Parameters.ActuatorBias.First := LREAL#10.0;
Parameters.ActuatorBias.Second := LREAL#20.0;
Parameters.SingularityTolerance := LREAL#1.0e-9;
Accepted := Decoupler.Configure(Parameters := Parameters);
ASSERT_TRUE(Accepted);
Status := Decoupler.Snapshot();
ASSERT_TRUE(Status.Configured);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Status.Determinant, DELTA := LREAL#1.0e-12);
ActuatorCommand := Decoupler.ToActuators(VirtualCommand := VirtualCommand);
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := ActuatorCommand.First, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#19.0, ACTUAL := ActuatorCommand.Second, DELTA := LREAL#1.0e-12);
Recovered := Decoupler.ToVirtual(ActuatorCommand := ActuatorCommand);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Recovered.First, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Recovered.Second, DELTA := LREAL#1.0e-12);

Parameters.GainMatrix.G11 := LREAL#1000000.0;
Parameters.GainMatrix.G12 := LREAL#1000000.0;
Parameters.GainMatrix.G21 := LREAL#1000000.0;
Parameters.GainMatrix.G22 := LREAL#1000000.01;
Parameters.SingularityTolerance := LREAL#1.0e-6;
Accepted := Decoupler.Configure(Parameters := Parameters);
ASSERT_FALSE(Accepted);
ActuatorCommand := Decoupler.ToActuators(VirtualCommand := VirtualCommand);
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := ActuatorCommand.First, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#19.0, ACTUAL := ActuatorCommand.Second, DELTA := LREAL#1.0e-12);

Parameters.GainMatrix.G11 := LREAL#1.0;
Parameters.GainMatrix.G12 := LREAL#2.0;
Parameters.GainMatrix.G21 := LREAL#2.0;
Parameters.GainMatrix.G22 := LREAL#4.0;
Accepted := Decoupler.Configure(Parameters := Parameters);
ASSERT_FALSE(Accepted);
ActuatorCommand := Decoupler.ToActuators(VirtualCommand := VirtualCommand);
ASSERT_NEAR(EXPECTED := LREAL#12.0, ACTUAL := ActuatorCommand.First, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#19.0, ACTUAL := ActuatorCommand.Second, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
