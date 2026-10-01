(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM TorqueIsProportionalToThePositionError
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    Input : Control.PIDInput;
END_VAR
(* The P configuration of Control.PID: Kp = 16 N·m/rad, zero integral and derivative gains. *)
Parameters.ProportionalGain := LREAL#16.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#1.0;
(* 0.1 rad short of the target: 1.6 N·m. *)
Input.ProcessValue := LREAL#0.9;
ASSERT_NEAR(EXPECTED := LREAL#1.6, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
(* At the target: zero torque, whatever the velocity. *)
Input.ProcessValue := LREAL#1.0;
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
(* 0.2 rad past it: the torque reverses. *)
Input.ProcessValue := LREAL#1.2;
ASSERT_NEAR(EXPECTED := LREAL#-3.2, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheDriveLimitClipsTheRequest
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    Input : Control.PIDInput;
END_VAR
Parameters.ProportionalGain := LREAL#16.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Controller.Configure(Parameters := Parameters);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
(* A 1.5 rad error requests 24 N·m; the drive delivers at most 20 N·m. *)
Input.Setpoint := LREAL#1.5;
Input.ProcessValue := LREAL#0.0;
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
