(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM TrackedIntegralReproducesTheManualCommand
(* P = 4 points and 60 % applied: I = 56 % gives P + I = 60 %. A stale I of 75 % would request 79 %. *)
ASSERT_NEAR(EXPECTED := LREAL#56.0, ACTUAL := InitialIntegral(AppliedOutput := LREAL#60.0, Proportional := LREAL#4.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#79.0, ACTUAL := LREAL#4.0 + LREAL#75.0, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TrackingRemovesTheTransferJump
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    Manual : LREAL;
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
(* Initialized at 35 °C and 75 %; the operator then holds 60 % and the water cools to 34.2 °C. *)
State.Setpoint := LREAL#35.0;
State.ProcessValue := LREAL#35.0;
State.NominalOutput := LREAL#75.0;
State.SaturatedOutput := LREAL#75.0;
Controller.Initialize(State := State);
Input.Setpoint := LREAL#35.0;
Input.ProcessValue := LREAL#34.2;
Input.CycleTime := LREAL#0.1;
Input.ManualOutput := LREAL#60.0;
(* Untracked: the first automatic update continues from 75 %: 75 + 13·0.8 + 3.5·0.8·0.1 = 85.68 %. *)
Input.Mode := PIDMode#Auto;
ASSERT_NEAR(EXPECTED := LREAL#85.68, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
(* Tracked: a Manual update aligns the stored values with the 60 % command, the setpoint and PV. *)
Controller.Initialize(State := State);
Input.Mode := PIDMode#Manual;
Manual := Controller.Update(Input := Input);
ASSERT_NEAR(EXPECTED := LREAL#60.0, ACTUAL := Manual, DELTA := LREAL#1.0e-12);
(* The next automatic update adds only its increments: 60 + 3.5·0.8·0.1 = 60.28 %. *)
Input.Mode := PIDMode#Auto;
ASSERT_NEAR(EXPECTED := LREAL#60.28, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
