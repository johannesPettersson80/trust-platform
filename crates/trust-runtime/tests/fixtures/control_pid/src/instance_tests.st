USING Control;

TEST_PROGRAM control_pid_and_filter_histories_are_instance_local
VAR
    PidA : PID;
    PidB : PID;
    PidAAlone : PID;
    PidBAlone : PID;
    PidParametersA : PIDParameters;
    PidParametersB : PIDParameters;
    PidStateA : PIDInitialState;
    PidStateB : PIDInitialState;
    PidInputA : PIDInput;
    PidInputB : PIDInput;
    A1 : LREAL;
    A2 : LREAL;
    B1 : LREAL;
    B2 : LREAL;
    FilterA : LowPass;
    FilterB : LowPass;
    FilterAAlone : LowPass;
    FilterBAlone : LowPass;
    FilterParametersA : LowPassParameters;
    FilterParametersB : LowPassParameters;
    FA1 : LREAL;
    FA2 : LREAL;
    FB1 : LREAL;
    FB2 : LREAL;
END_VAR

PidParametersA.ProportionalGain := LREAL#1.0;
PidParametersA.IntegralGain := LREAL#1.0;
PidParametersA.ProportionalSetpointWeight := LREAL#0.5;
PidParametersA.OutputMinimum := LREAL#-100.0;
PidParametersA.OutputMaximum := LREAL#100.0;
PidParametersA.RateMinimum := LREAL#-1000.0;
PidParametersA.RateMaximum := LREAL#1000.0;
PidParametersA.TrackingTime := LREAL#1.0;
PidParametersB := PidParametersA;
PidParametersB.ProportionalGain := LREAL#2.0;
PidParametersB.IntegralGain := LREAL#0.5;
PidStateB.NominalOutput := LREAL#5.0;
PidStateB.SaturatedOutput := LREAL#5.0;

PidA.Configure(Parameters := PidParametersA);
PidA.Initialize(State := PidStateA);
PidAAlone.Configure(Parameters := PidParametersA);
PidAAlone.Initialize(State := PidStateA);
PidB.Configure(Parameters := PidParametersB);
PidB.Initialize(State := PidStateB);
PidBAlone.Configure(Parameters := PidParametersB);
PidBAlone.Initialize(State := PidStateB);

PidInputA.Setpoint := LREAL#1.0;
PidInputA.CycleTime := LREAL#0.1;
PidInputB.Setpoint := LREAL#-2.0;
PidInputB.ProcessValue := LREAL#0.5;
PidInputB.CycleTime := LREAL#0.2;
A1 := PidA.Update(Input := PidInputA);
B1 := PidB.Update(Input := PidInputB);
PidInputA.ProcessValue := LREAL#0.2;
A2 := PidA.Update(Input := PidInputA);
PidInputB.ProcessValue := LREAL#0.0;
B2 := PidB.Update(Input := PidInputB);

PidInputA.ProcessValue := LREAL#0.0;
ASSERT_NEAR(EXPECTED := A1, ACTUAL := PidAAlone.Update(Input := PidInputA), DELTA := LREAL#1.0e-12);
PidInputA.ProcessValue := LREAL#0.2;
ASSERT_NEAR(EXPECTED := A2, ACTUAL := PidAAlone.Update(Input := PidInputA), DELTA := LREAL#1.0e-12);

PidInputB.ProcessValue := LREAL#0.5;
ASSERT_NEAR(EXPECTED := B1, ACTUAL := PidBAlone.Update(Input := PidInputB), DELTA := LREAL#1.0e-12);
PidInputB.ProcessValue := LREAL#0.0;
ASSERT_NEAR(EXPECTED := B2, ACTUAL := PidBAlone.Update(Input := PidInputB), DELTA := LREAL#1.0e-12);

FilterParametersA.TimeConstant := LREAL#1.5;
FilterParametersB.TimeConstant := LREAL#2.5;
FilterParametersB.Order := FilterOrder#SecondOrder;
FilterA.Configure(Parameters := FilterParametersA);
FilterA.Initialize(Input := LREAL#0.0);
FilterAAlone.Configure(Parameters := FilterParametersA);
FilterAAlone.Initialize(Input := LREAL#0.0);
FilterB.Configure(Parameters := FilterParametersB);
FilterB.Initialize(Input := LREAL#10.0);
FilterBAlone.Configure(Parameters := FilterParametersB);
FilterBAlone.Initialize(Input := LREAL#10.0);

FA1 := FilterA.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0);
FB1 := FilterB.Update(Input := LREAL#2.0, CycleTime := LREAL#0.5);
FA2 := FilterA.Update(Input := LREAL#6.0, CycleTime := LREAL#1.0);
FB2 := FilterB.Update(Input := LREAL#4.0, CycleTime := LREAL#0.5);

ASSERT_NEAR(EXPECTED := FA1, ACTUAL := FilterAAlone.Update(Input := LREAL#4.0, CycleTime := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := FA2, ACTUAL := FilterAAlone.Update(Input := LREAL#6.0, CycleTime := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := FB1, ACTUAL := FilterBAlone.Update(Input := LREAL#2.0, CycleTime := LREAL#0.5), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := FB2, ACTUAL := FilterBAlone.Update(Input := LREAL#4.0, CycleTime := LREAL#0.5), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
