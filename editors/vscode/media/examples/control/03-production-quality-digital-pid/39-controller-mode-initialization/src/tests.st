(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM IntegralReproducesTheAppliedCommand
(* At SP = PV = 35 °C, P = 0, so I = 75 %. At SP 38 °C with beta 0.3, P = 13·0.3·3 = 11.7 %, so I = 63.3 %. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := WeightedProportional(ProportionalGain := LREAL#13.0,
    SetpointWeight := LREAL#0.3, Setpoint := LREAL#35.0, ProcessValue := LREAL#35.0, OperatingPoint := LREAL#35.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := InitialIntegral(AppliedOutput := LREAL#75.0, Proportional := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#11.7, ACTUAL := WeightedProportional(ProportionalGain := LREAL#13.0,
    SetpointWeight := LREAL#0.3, Setpoint := LREAL#38.0, ProcessValue := LREAL#35.0, OperatingPoint := LREAL#35.0),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#63.3, ACTUAL := InitialIntegral(AppliedOutput := LREAL#75.0, Proportional := LREAL#11.7),
    DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM FirstCommandComesFromTheStoredValues
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
Parameters.ProportionalGain := LREAL#13.0;
Parameters.IntegralGain := LREAL#3.5;
Parameters.ProportionalSetpointWeight := LREAL#0.3;
Parameters.OutputMinimum := LREAL#0.0;
Parameters.OutputMaximum := LREAL#100.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Controller.Configure(Parameters := Parameters);
Input.Setpoint := LREAL#35.0;
Input.ProcessValue := LREAL#35.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
(* Stored values from the running heater: the first command continues the applied 75 %. *)
State.Setpoint := LREAL#35.0;
State.ProcessValue := LREAL#35.0;
State.NominalOutput := LREAL#75.0;
State.SaturatedOutput := LREAL#75.0;
Controller.Initialize(State := State);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
(* Stored values all zero: dP = 13·(0.3·35 − 35) = −318.5 points, so the heater is switched off. *)
State.Setpoint := LREAL#0.0;
State.ProcessValue := LREAL#0.0;
State.NominalOutput := LREAL#0.0;
State.SaturatedOutput := LREAL#0.0;
Controller.Initialize(State := State);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
