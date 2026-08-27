USING Control;

TEST_PROGRAM control_pid_replays_paper_example_1_manual_to_auto
VAR
    Controller : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#0.6666666666666667;
Parameters.IntegralGain := LREAL#0.6666666666666667;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-1.0e100;
Parameters.OutputMaximum := LREAL#1.0e100;
Parameters.RateMinimum := LREAL#-1.0e100;
Parameters.RateMaximum := LREAL#1.0e100;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := State);

Input.Setpoint := LREAL#3.0;
Input.CycleTime := LREAL#0.01;
Input.Mode := PIDMode#Manual;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Input.ManualOutput := LREAL#1.0;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Output, DELTA := LREAL#1.0e-12);

(* Auto resumes from the nominal manual command retained by Listing 3. *)
Input.ProcessValue := LREAL#1.0;
Input.Mode := PIDMode#Auto;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.3466666666666667, ACTUAL := Output, DELTA := LREAL#1.0e-12);

State.Setpoint := LREAL#3.0;
State.ProcessValue := LREAL#3.0;
State.NominalOutput := LREAL#3.0;
State.SaturatedOutput := LREAL#3.0;
Controller.Initialize(State := State);
Input.ProcessValue := LREAL#3.0;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := Output, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_replays_paper_example_2_form_switch_and_rate_limit
VAR
    Controller : PID;
    LimitedController : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#0.6666666666666667;
Parameters.IntegralGain := LREAL#0.6666666666666667;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#-1.0e100;
Parameters.OutputMaximum := LREAL#1.0e100;
Parameters.RateMinimum := LREAL#-1.0e100;
Parameters.RateMaximum := LREAL#1.0e100;
Parameters.TrackingTime := LREAL#1.0;
State.Setpoint := LREAL#1.0;
State.ProcessValue := LREAL#1.0;
State.NominalOutput := LREAL#1.0;
State.SaturatedOutput := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Controller.Initialize(State := State);

Input.Setpoint := LREAL#3.0;
Input.ProcessValue := LREAL#1.0;
Input.CycleTime := LREAL#0.1;
Input.Mode := PIDMode#Auto;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.4666666666666663, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.IntegralGain := LREAL#0.0;
Parameters.Bias := LREAL#2.0;
Controller.Configure(Parameters := Parameters);
Input.ProcessValue := LREAL#2.9;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.066666666666667, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.IntegralGain := LREAL#0.6666666666666667;
Controller.Configure(Parameters := Parameters);
Input.Setpoint := LREAL#1.0;
Input.ProcessValue := LREAL#1.6;
Output := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.56, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Parameters.Bias := LREAL#0.0;
Parameters.RateMinimum := LREAL#-0.5;
Parameters.RateMaximum := LREAL#0.5;
LimitedController.Configure(Parameters := Parameters);
LimitedController.Initialize(State := State);
Input.Setpoint := LREAL#3.0;
Input.ProcessValue := LREAL#1.0;
Output := LimitedController.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.05, ACTUAL := Output, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_replays_paper_example_3_setpoint_weighting
VAR
    FullWeight : PID;
    HalfWeight : PID;
    ZeroWeight : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#0.6666666666666667;
Parameters.IntegralGain := LREAL#0.6666666666666667;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-1.0e100;
Parameters.OutputMaximum := LREAL#1.0e100;
Parameters.RateMinimum := LREAL#-1.0e100;
Parameters.RateMaximum := LREAL#1.0e100;
Parameters.TrackingTime := LREAL#1.0;
FullWeight.Configure(Parameters := Parameters);
FullWeight.Initialize(State := State);
Parameters.ProportionalSetpointWeight := LREAL#0.5;
HalfWeight.Configure(Parameters := Parameters);
HalfWeight.Initialize(State := State);
Parameters.ProportionalSetpointWeight := LREAL#0.0;
ZeroWeight.Configure(Parameters := Parameters);
ZeroWeight.Initialize(State := State);

Input.Setpoint := LREAL#3.0;
Input.CycleTime := LREAL#0.01;
Input.Mode := PIDMode#Auto;
Output := FullWeight.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#2.02, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := HalfWeight.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.02, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := ZeroWeight.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.02, ACTUAL := Output, DELTA := LREAL#1.0e-12);

(* The same load disturbance changes the process-value term equally. *)
Input.ProcessValue := LREAL#1.0;
Output := FullWeight.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#1.3666666666666667, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := HalfWeight.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.3666666666666667, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := ZeroWeight.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-0.6333333333333333, ACTUAL := Output, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_replays_paper_example_4_feedforward_and_anti_windup
VAR
    Unconstrained : PID;
    Clamping : PID;
    BackCalculation : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    Output : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#2.727272727272727;
Parameters.IntegralGain := LREAL#0.9090909090909091;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-1.0e100;
Parameters.OutputMaximum := LREAL#1.0e100;
Parameters.RateMinimum := LREAL#-1.0e100;
Parameters.RateMaximum := LREAL#1.0e100;
Parameters.TrackingTime := LREAL#3.0;
Unconstrained.Configure(Parameters := Parameters);
Unconstrained.Initialize(State := State);
Parameters.OutputMinimum := LREAL#-3.5;
Parameters.OutputMaximum := LREAL#3.5;
Parameters.TrackingTime := LREAL#0.01;
Clamping.Configure(Parameters := Parameters);
Clamping.Initialize(State := State);
Parameters.TrackingTime := LREAL#3.0;
BackCalculation.Configure(Parameters := Parameters);
BackCalculation.Initialize(State := State);

Input.Setpoint := LREAL#4.5;
Input.CycleTime := LREAL#0.01;
Input.Mode := PIDMode#Auto;
Output := Unconstrained.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#12.313636363636363, ACTUAL := Output, DELTA := LREAL#1.0e-11);
Output := Clamping.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.5, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := BackCalculation.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#3.5, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Input.Setpoint := LREAL#0.0;
Output := Clamping.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-3.5, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := BackCalculation.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#0.011530303030303557, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Input.FeedForward := LREAL#-8.970083166928498;
Output := Unconstrained.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-8.929174076019407, ACTUAL := Output, DELTA := LREAL#1.0e-11);
Output := Clamping.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-3.5, ACTUAL := Output, DELTA := LREAL#1.0e-12);
Output := BackCalculation.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#-3.5, ACTUAL := Output, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_replays_paper_example_5_measurement_filters
VAR
    SlowFirst : LowPass;
    SlowSecond : LowPass;
    FastFirst : LowPass;
    FastSecond : LowPass;
    Parameters : LowPassParameters;
    Output : LREAL;
END_VAR

Parameters.TimeConstant := LREAL#0.1;
Parameters.Order := FilterOrder#FirstOrder;
SlowFirst.Configure(Parameters := Parameters);
SlowFirst.Initialize(Input := LREAL#0.0);
Parameters.Order := FilterOrder#SecondOrder;
SlowSecond.Configure(Parameters := Parameters);
SlowSecond.Initialize(Input := LREAL#0.0);
Parameters.TimeConstant := LREAL#0.01;
Parameters.Order := FilterOrder#FirstOrder;
FastFirst.Configure(Parameters := Parameters);
FastFirst.Initialize(Input := LREAL#0.0);
Parameters.Order := FilterOrder#SecondOrder;
FastSecond.Configure(Parameters := Parameters);
FastSecond.Initialize(Input := LREAL#0.0);

(* These are selected released noisy measurements after the process delay. *)
Output := SlowFirst.Update(Input := LREAL#0.0775918043205658, CycleTime := LREAL#0.01);
Output := SlowFirst.Update(Input := LREAL#0.14700588752798377, CycleTime := LREAL#0.01);
Output := SlowFirst.Update(Input := LREAL#0.26877691165790607, CycleTime := LREAL#0.01);
ASSERT_NEAR(EXPECTED := LREAL#0.04431413638365756, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Output := SlowSecond.Update(Input := LREAL#0.0775918043205658, CycleTime := LREAL#0.01);
Output := SlowSecond.Update(Input := LREAL#0.14700588752798377, CycleTime := LREAL#0.01);
Output := SlowSecond.Update(Input := LREAL#0.26877691165790607, CycleTime := LREAL#0.01);
ASSERT_NEAR(EXPECTED := LREAL#0.0065790126617539055, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Output := FastFirst.Update(Input := LREAL#0.0775918043205658, CycleTime := LREAL#0.01);
Output := FastFirst.Update(Input := LREAL#0.14700588752798377, CycleTime := LREAL#0.01);
Output := FastFirst.Update(Input := LREAL#0.26877691165790607, CycleTime := LREAL#0.01);
ASSERT_NEAR(EXPECTED := LREAL#0.21760012383893867, ACTUAL := Output, DELTA := LREAL#1.0e-12);

Output := FastSecond.Update(Input := LREAL#0.0775918043205658, CycleTime := LREAL#0.01);
Output := FastSecond.Update(Input := LREAL#0.14700588752798377, CycleTime := LREAL#0.01);
Output := FastSecond.Update(Input := LREAL#0.26877691165790607, CycleTime := LREAL#0.01);
ASSERT_NEAR(EXPECTED := LREAL#0.17450878731114844, ACTUAL := Output, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_replays_paper_example_6_gain_scheduling_with_tracking
VAR
    LowController : PID;
    MiddleController : PID;
    HighController : PID;
    Parameters : PIDParameters;
    State : PIDInitialState;
    Input : PIDInput;
    TankArea : LREAL;
    OrificeArea : LREAL;
    Gravity : LREAL;
    Time1 : LREAL;
    Time2 : LREAL;
    Time3 : LREAL;
    Gain1 : LREAL;
    Gain2 : LREAL;
    Gain3 : LREAL;
    CommonTime : LREAL;
    InitialFlow : LREAL;
    LowOutput : LREAL;
    MiddleOutput : LREAL;
    HighOutput : LREAL;
    SelectedOutput : LREAL;
END_VAR

TankArea := LREAL#390.0;
OrificeArea := LREAL#2.15;
Gravity := LREAL#983.0;
Time1 := TankArea / OrificeArea * SQRT(LREAL#8.0 / Gravity);
Time2 := TankArea / OrificeArea * SQRT(LREAL#24.0 / Gravity);
Time3 := TankArea / OrificeArea * SQRT(LREAL#40.0 / Gravity);
Gain1 := Time1 / TankArea;
Gain2 := Time2 / TankArea;
Gain3 := Time3 / TankArea;
CommonTime := (Time1 + Time2 + Time3) / LREAL#3.0;
InitialFlow := OrificeArea * SQRT(LREAL#8.0 * Gravity);

Parameters.ProportionalGain := Time1 / (Gain1 * CommonTime);
Parameters.IntegralGain := Parameters.ProportionalGain / Time1;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-1.0e100;
Parameters.OutputMaximum := LREAL#1.0e100;
Parameters.RateMinimum := LREAL#-1.0e100;
Parameters.RateMaximum := LREAL#1.0e100;
Parameters.TrackingTime := Time1;
State.Setpoint := LREAL#4.0;
State.ProcessValue := LREAL#4.0;
State.NominalOutput := InitialFlow;
State.SaturatedOutput := InitialFlow;
LowController.Configure(Parameters := Parameters);
LowController.Initialize(State := State);
Parameters.ProportionalGain := Time2 / (Gain2 * CommonTime);
Parameters.IntegralGain := Parameters.ProportionalGain / Time2;
Parameters.TrackingTime := Time2;
MiddleController.Configure(Parameters := Parameters);
MiddleController.Initialize(State := State);
Parameters.ProportionalGain := Time3 / (Gain3 * CommonTime);
Parameters.IntegralGain := Parameters.ProportionalGain / Time3;
Parameters.TrackingTime := Time3;
HighController.Configure(Parameters := Parameters);
HighController.Initialize(State := State);

Input.Setpoint := LREAL#22.0;
Input.ProcessValue := LREAL#4.0;
Input.CycleTime := LREAL#0.01;
Input.Mode := PIDMode#Auto;
LowOutput := LowController.Update(Input := Input);
Input.Mode := PIDMode#Track;
Input.TrackingOutput := InitialFlow;
MiddleOutput := MiddleController.Update(Input := Input);
HighOutput := HighController.Update(Input := Input);
SelectedOutput := LowOutput;
ASSERT_NEAR(EXPECTED := LREAL#449.8616638937913, ACTUAL := SelectedOutput, DELTA := LREAL#1.0e-8);

Input.ProcessValue := LREAL#12.1;
Input.TrackingOutput := SelectedOutput;
Input.Mode := PIDMode#Track;
LowOutput := LowController.Update(Input := Input);
Input.Mode := PIDMode#Auto;
MiddleOutput := MiddleController.Update(Input := Input);
Input.Mode := PIDMode#Track;
HighOutput := HighController.Update(Input := Input);
SelectedOutput := MiddleOutput;
ASSERT_NEAR(EXPECTED := LREAL#333.2756344812609, ACTUAL := SelectedOutput, DELTA := LREAL#1.0e-8);

Input.ProcessValue := LREAL#20.1;
Input.TrackingOutput := SelectedOutput;
Input.Mode := PIDMode#Track;
LowOutput := LowController.Update(Input := Input);
MiddleOutput := MiddleController.Update(Input := Input);
Input.Mode := PIDMode#Auto;
HighOutput := HighController.Update(Input := Input);
SelectedOutput := HighOutput;
ASSERT_NEAR(EXPECTED := LREAL#218.20841899630443, ACTUAL := SelectedOutput, DELTA := LREAL#1.0e-8);
END_TEST_PROGRAM

TEST_PROGRAM control_pid_replays_paper_example_7_selector_with_tracking
VAR
    FirstController : PID;
    SecondController : PID;
    Parameters : PIDParameters;
    FirstState : PIDInitialState;
    SecondState : PIDInitialState;
    FirstInput : PIDInput;
    SecondInput : PIDInput;
    FirstOutput : LREAL;
    SecondOutput : LREAL;
    SelectedOutput : LREAL;
END_VAR

Parameters.ProportionalGain := LREAL#0.6666666666666667;
Parameters.IntegralGain := LREAL#0.6666666666666667;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-1.0e100;
Parameters.OutputMaximum := LREAL#1.0e100;
Parameters.RateMinimum := LREAL#-1.0e100;
Parameters.RateMaximum := LREAL#1.0e100;
Parameters.TrackingTime := LREAL#1.0;
FirstState.Setpoint := LREAL#0.3;
FirstState.ProcessValue := LREAL#0.3;
SecondState.Setpoint := LREAL#0.5;
SecondState.ProcessValue := LREAL#0.5;
FirstController.Configure(Parameters := Parameters);
FirstController.Initialize(State := FirstState);
SecondController.Configure(Parameters := Parameters);
SecondController.Initialize(State := SecondState);

FirstInput.Setpoint := LREAL#0.3;
FirstInput.ProcessValue := LREAL#0.2871535891077072;
FirstInput.CycleTime := LREAL#0.01;
FirstInput.Mode := PIDMode#Auto;
SecondInput.Setpoint := LREAL#0.5;
SecondInput.ProcessValue := LREAL#0.49502491687458405;
SecondInput.CycleTime := LREAL#0.01;
SecondInput.Mode := PIDMode#Track;
FirstOutput := FirstController.Update(Input := FirstInput);
SecondOutput := SecondController.Update(Input := SecondInput);
SelectedOutput := MIN(FirstOutput, SecondOutput);
ASSERT_NEAR(EXPECTED := LREAL#0.003349889304446737, ACTUAL := SelectedOutput, DELTA := LREAL#1.0e-12);

FirstInput.ProcessValue := LREAL#0.2785272123191362;
FirstInput.TrackingOutput := SelectedOutput;
FirstInput.Mode := PIDMode#Track;
SecondInput.ProcessValue := LREAL#0.5;
SecondInput.TrackingOutput := SelectedOutput;
SecondInput.Mode := PIDMode#Auto;
FirstOutput := FirstController.Update(Input := FirstInput);
SecondOutput := SecondController.Update(Input := SecondInput);
SelectedOutput := MIN(FirstOutput, SecondOutput);
ASSERT_NEAR(EXPECTED := LREAL#0.00003316722083610609, ACTUAL := SelectedOutput, DELTA := LREAL#1.0e-12);

FirstInput.ProcessValue := LREAL#0.2682;
FirstInput.TrackingOutput := SelectedOutput;
SecondInput.ProcessValue := LREAL#0.447019;
SecondInput.TrackingOutput := SelectedOutput;
FirstOutput := FirstController.Update(Input := FirstInput);
SecondOutput := SecondController.Update(Input := SecondInput);
SelectedOutput := MIN(FirstOutput, SecondOutput);
ASSERT_NEAR(EXPECTED := LREAL#0.0071299754335935675, ACTUAL := SelectedOutput, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
