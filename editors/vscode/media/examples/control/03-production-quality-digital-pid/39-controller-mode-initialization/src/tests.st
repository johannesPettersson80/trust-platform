(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM Lesson39StartupStateMustMatchTheRunningPump
VAR
    PressureFilter : Control.LowPass;
    FilterParameters : Control.LowPassParameters;
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    ZeroStateFilteredPressure : LREAL;
    MeasuredStateFilteredPressure : LREAL;
    FirstAutoRequest : LREAL;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#6.0, ACTUAL := Lesson39EquilibriumPressure(
    ZeroCommandPressure := LREAL#2.0,
    PressureGain := LREAL#0.1,
    ValveCommand := LREAL#40.0
), DELTA := LREAL#1.0e-12);
FilterParameters.TimeConstant := LREAL#0.5;
FilterParameters.Order := FilterOrder#SecondOrder;
PressureFilter.Configure(Parameters := FilterParameters);
PressureFilter.Initialize(Input := LREAL#0.0);
ZeroStateFilteredPressure := PressureFilter.Update(
    Input := LREAL#6.0,
    CycleTime := LREAL#0.1
);
ASSERT_NEAR(EXPECTED := LREAL#0.1983471074380165, ACTUAL := ZeroStateFilteredPressure, DELTA := LREAL#1.0e-12);
PressureFilter.Initialize(Input := LREAL#6.0);
MeasuredStateFilteredPressure := PressureFilter.Update(
    Input := LREAL#6.0,
    CycleTime := LREAL#0.1
);
ASSERT_NEAR(EXPECTED := LREAL#6.0, ACTUAL := MeasuredStateFilteredPressure, DELTA := LREAL#1.0e-12);

Parameters.ProportionalGain := LREAL#15.0;
Parameters.IntegralGain := LREAL#3.0;
Parameters.DerivativeGain := LREAL#0.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-100.0;
Parameters.RateMaximum := LREAL#100.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#Combined;
Controller.Configure(Parameters := Parameters);
InitialState.Setpoint := LREAL#6.0;
InitialState.ProcessValue := LREAL#6.0;
InitialState.NominalOutput := LREAL#40.0;
InitialState.SaturatedOutput := LREAL#40.0;
Controller.Initialize(State := InitialState);
Input.Setpoint := LREAL#6.0;
Input.ProcessValue := LREAL#6.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
FirstAutoRequest := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := FirstAutoRequest, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
