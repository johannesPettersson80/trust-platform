(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM DerivativeOnMeasurementBrakesTheMotion
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* The PD configuration: Kp = 16 N·m/rad, Kd = 7 N·m·s/rad, D on the measured angle. *)
Parameters.ProportionalGain := LREAL#16.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
(* The shaft rests at its 1 rad target; the controller starts from that measurement. *)
State.Setpoint := LREAL#1.0;
State.ProcessValue := LREAL#1.0;
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#1.0;
Input.ProcessValue := LREAL#1.0;
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
(* At the target, moving at 0.004 rad per 20 ms = 0.2 rad/s: P is zero and D brakes with −7 · 0.2 N·m. *)
Input.Setpoint := LREAL#1.004;
Input.ProcessValue := LREAL#1.004;
ASSERT_NEAR(EXPECTED := LREAL#-1.4, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
(* At rest, D contributes nothing. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM DerivativeOnErrorKicksAtATargetStep
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    Input : Control.PIDInput;
END_VAR
Parameters.ProportionalGain := LREAL#16.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.ProcessValue := LREAL#0.0;
(* D on the measurement: a 0 → 1 rad target step requests only Kp · 1 rad. *)
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Controller.Configure(Parameters := Parameters);
Input.Setpoint := LREAL#0.0;
Controller.Update(Input := Input);
Input.Setpoint := LREAL#1.0;
ASSERT_NEAR(EXPECTED := LREAL#16.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
(* D on the error: the step adds Kd · 1 rad / 0.02 s = 350 N·m; the drive limit clips the request at 20 N·m. *)
Parameters.DerivativeSetpointWeight := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Input.Setpoint := LREAL#0.0;
Controller.Update(Input := Input);
Input.Setpoint := LREAL#1.0;
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
