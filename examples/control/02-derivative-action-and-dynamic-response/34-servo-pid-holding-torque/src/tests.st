(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM PdNeedsAnErrorToHoldTheDrum
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* A 2 kg weight on a 0.1 m drum: τL = m·g·r = 1.962 N·m. PD with Kp = 16 holds it at e = τL/Kp. *)
ASSERT_NEAR(EXPECTED := LREAL#1.962, ACTUAL := LREAL#2.0 * LREAL#9.81 * LREAL#0.1, DELTA := LREAL#1.0e-12);
Parameters.ProportionalGain := LREAL#16.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
(* At rest 0.122625 rad below the 0 rad target, P alone supplies the 1.962 N·m. *)
State.ProcessValue := LREAL#-0.122625;
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#0.0;
Input.ProcessValue := LREAL#-0.122625;
ASSERT_NEAR(EXPECTED := LREAL#1.962, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheIntegralHoldsTheDrumAtZeroError
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* The PID configuration: Kp = 16, Ki = 12 N·m/(rad·s), Kd = 7, β = 0.3. *)
Parameters.ProportionalGain := LREAL#16.0;
Parameters.IntegralGain := LREAL#12.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.ProportionalSetpointWeight := LREAL#0.3;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Controller.Configure(Parameters := Parameters);
(* The integral has learned the load: 1.962 N·m retained, the shaft at rest on its target. *)
State.NominalOutput := LREAL#1.962;
State.SaturatedOutput := LREAL#1.962;
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#0.0;
Input.ProcessValue := LREAL#0.0;
ASSERT_NEAR(EXPECTED := LREAL#1.962, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#1.962, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
