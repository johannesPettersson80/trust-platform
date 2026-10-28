(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM TheIncrementOfTheExample
VAR
    Increment : LREAL;
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* e 0.10 → 0.096 rad, ω_f 0.20 → 0.22 rad/s, Kp 16, Ki 4, Kd 7, Δt 0.02 s, no tracking correction. *)
Increment := LREAL#16.0 * (LREAL#0.096 - LREAL#0.10) + LREAL#4.0 * LREAL#0.096 * LREAL#0.02
    - LREAL#7.0 * (LREAL#0.22 - LREAL#0.20);
ASSERT_NEAR(EXPECTED := LREAL#-0.19632, ACTUAL := Increment, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.00368, ACTUAL := LREAL#1.2 + Increment, DELTA := LREAL#1.0e-12);
(* Control.PID adds the same increment to its retained 1.2 N·m. The angle moves 0.004 rad and then 0.0044 rad
   per update (0.20 and 0.22 rad/s) while the target moves 0.0004 rad: the error goes from 0.10 to 0.096 rad. *)
Parameters.ProportionalGain := LREAL#16.0;
Parameters.IntegralGain := LREAL#4.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Controller.Configure(Parameters := Parameters);
State.Setpoint := LREAL#0.5;
State.ProcessValue := LREAL#0.4;
State.ProcessValueRate := LREAL#0.20;
State.NominalOutput := LREAL#1.2;
State.SaturatedOutput := LREAL#1.2;
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#0.5004;
Input.ProcessValue := LREAL#0.4044;
ASSERT_NEAR(EXPECTED := LREAL#1.00368, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheClampedUpdateRetainsTheCorrectedRequest
VAR
    Controller : Control.PID;
    RetainedApplied : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* The previous update requested 25 N·m against the 20 N·m limit. At rest on target the increment is zero, so
   this update requests 25 N·m again and the drive gets 20 N·m. *)
Parameters.ProportionalGain := LREAL#16.0;
Parameters.IntegralGain := LREAL#4.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Controller.Configure(Parameters := Parameters);
State.Setpoint := LREAL#0.5;
State.ProcessValue := LREAL#0.5;
State.NominalOutput := LREAL#25.0;
State.SaturatedOutput := LREAL#20.0;
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#0.5;
Input.ProcessValue := LREAL#0.5;
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
(* γ = (Δt/Tt)(u_a − u_r) = 0.02 · (20 − 25) = −0.1 N·m: the retained request is 24.9 N·m. *)
State := Controller.GetState();
ASSERT_NEAR(EXPECTED := LREAL#24.9, ACTUAL := State.NominalOutput, DELTA := LREAL#1.0e-9);
(* Tracking with Tt = Δt removes the whole excess: that controller retains the applied 20 N·m. *)
Parameters.TrackingTime := LREAL#0.02;
RetainedApplied.Configure(Parameters := Parameters);
State.NominalOutput := LREAL#25.0;
State.SaturatedOutput := LREAL#20.0;
RetainedApplied.Initialize(State := State);
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := RetainedApplied.Update(Input := Input), DELTA := LREAL#1.0e-12);
State := RetainedApplied.GetState();
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := State.NominalOutput, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM AGainRevisionAtConstantError
VAR
    Controller : Control.PID;
    Positional : PositionalPid;
    Unpreloaded : PositionalPid;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
    Count : INT;
    Before : LREAL;
    After : LREAL;
END_VAR
(* The axis rests 0.1 rad below its target. Two Manual updates hold 1.6 N·m; then one Auto update with
   Kp = 16, and one after the revision to Kp = 24. *)
Parameters.ProportionalGain := LREAL#16.0;
Parameters.IntegralGain := LREAL#4.0;
Parameters.DerivativeGain := LREAL#7.0;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
Parameters.OutputMinimum := LREAL#-20.0;
Parameters.OutputMaximum := LREAL#20.0;
Parameters.RateMinimum := LREAL#-10000.0;
Parameters.RateMaximum := LREAL#10000.0;
Parameters.TrackingTime := LREAL#1.0;
Parameters.AntiWindup := PIDAntiWindup#BackCalculation;
Controller.Configure(Parameters := Parameters);
State.Setpoint := LREAL#0.5;
State.ProcessValue := LREAL#0.4;
Controller.Initialize(State := State);
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#0.5;
Input.ProcessValue := LREAL#0.4;
Input.ManualOutput := LREAL#1.6;
Input.Mode := PIDMode#Manual;
FOR Count := 1 TO 2 DO
    Controller.Update(Input := Input);
    Positional(Parameters := Parameters, Input := Input);
    Unpreloaded(Parameters := Parameters, Input := Input, PreloadOnGainRevision := FALSE);
END_FOR;
Input.Mode := PIDMode#Auto;
Before := Controller.Update(Input := Input);
Positional(Parameters := Parameters, Input := Input);
Unpreloaded(Parameters := Parameters, Input := Input, PreloadOnGainRevision := FALSE);
(* 1.6 + Ki·e·Δt = 1.6 + 4 · 0.1 · 0.02 = 1.608 N·m in both forms. *)
ASSERT_NEAR(EXPECTED := LREAL#1.608, ACTUAL := Before, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := Before, ACTUAL := Positional.TorqueNm, DELTA := LREAL#1.0e-12);
Parameters.ProportionalGain := LREAL#24.0;
Controller.Configure(Parameters := Parameters);
After := Controller.Update(Input := Input);
Positional(Parameters := Parameters, Input := Input);
Unpreloaded(Parameters := Parameters, Input := Input, PreloadOnGainRevision := FALSE);
(* The error did not change, so Control.PID adds only the integral increment. *)
ASSERT_NEAR(EXPECTED := LREAL#1.616, ACTUAL := After, DELTA := LREAL#1.0e-12);
(* P rises by (24 − 16) · 0.1 = 0.8 N·m; the preloaded integral takes it over, so the positional torque agrees. *)
ASSERT_NEAR(EXPECTED := LREAL#2.4, ACTUAL := Positional.ProportionalNm, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := After, ACTUAL := Positional.TorqueNm, DELTA := LREAL#1.0e-12);
(* Without the preload the positional torque jumps by 0.8 N·m. *)
ASSERT_NEAR(EXPECTED := After + LREAL#0.8, ACTUAL := Unpreloaded.TorqueNm, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
