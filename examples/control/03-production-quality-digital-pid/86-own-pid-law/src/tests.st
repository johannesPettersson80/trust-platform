(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM AnEmptyIntegralTermDropsTheHoldingTorque
VAR
    Law : PositionalPid;
    Settings : Control.PIDParameters := (ProportionalGain := LREAL#16.0, IntegralGain := LREAL#4.0,
        DerivativeGain := LREAL#7.0, OutputMinimum := LREAL#-20.0, OutputMaximum := LREAL#20.0, TrackingTime := LREAL#1.0);
    Sample : Control.PIDInput := (Setpoint := LREAL#0.0, ProcessValue := LREAL#0.0, CycleTime := LREAL#0.02,
        Mode := PIDMode#Auto, ManualOutput := LREAL#1.962);
END_VAR
(* Started in Auto at the target, P and D are zero: the torque is the integral term, still zero. *)
Law(Parameters := Settings, Input := Sample);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Law.TorqueNm, DELTA := LREAL#1.0e-12);
(* One Manual update with the holding torque sets the integral term; the next Auto update continues from it. *)
Sample.Mode := PIDMode#Manual;
Law(Parameters := Settings, Input := Sample);
Sample.Mode := PIDMode#Auto;
Law(Parameters := Settings, Input := Sample);
ASSERT_NEAR(EXPECTED := LREAL#1.962, ACTUAL := Law.TorqueNm, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM AStalePreviousAngleLooksLikeMotion
VAR
    Law : PositionalPid;
    Settings : Control.PIDParameters := (ProportionalGain := LREAL#16.0, IntegralGain := LREAL#4.0,
        DerivativeGain := LREAL#7.0, OutputMinimum := LREAL#-20.0, OutputMaximum := LREAL#20.0, TrackingTime := LREAL#1.0);
    Sample : Control.PIDInput := (Setpoint := LREAL#0.5, ProcessValue := LREAL#0.5, CycleTime := LREAL#0.02,
        Mode := PIDMode#Auto);
END_VAR
(* The axis rests at 0.5 rad, but the previous angle is still 0: D sees 0.5 / 0.02 = 25 rad/s. *)
Law(Parameters := Settings, Input := Sample);
ASSERT_NEAR(EXPECTED := LREAL#-175.0, ACTUAL := Law.DerivativeNm, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-20.0, ACTUAL := Law.TorqueNm, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
