(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM TheBenchmarkUpdate
VAR
    ErrorRad : LREAL;
    ProportionalNm : LREAL;
    TrialIntegralNm : LREAL;
    DerivativeNm : LREAL;
    RequestNm : LREAL;
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* Target 0.50 rad, angle 0.42 rad after 0.40 rad, retained integral 1.962 N·m, Kp 16, Ki 4, Kd 7,
   dt 0.02 s, filter bypassed. *)
ErrorRad := LREAL#0.50 - LREAL#0.42;
ProportionalNm := LREAL#16.0 * ErrorRad;
TrialIntegralNm := LREAL#1.962 + LREAL#4.0 * ErrorRad * LREAL#0.02;
DerivativeNm := LREAL#-7.0 * (LREAL#0.42 - LREAL#0.40) / LREAL#0.02;
RequestNm := ProportionalNm + TrialIntegralNm + DerivativeNm;
ASSERT_NEAR(EXPECTED := LREAL#1.28, ACTUAL := ProportionalNm, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.9684, ACTUAL := TrialIntegralNm, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-7.0, ACTUAL := DerivativeNm, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-3.7516, ACTUAL := RequestNm, DELTA := LREAL#1.0e-9);
(* Control.PID from the same retained state: the previous update stood at 0.40 rad at rest, so it retained
   P + I + D = 16 · 0.10 + 1.962 + 0 = 3.562 N·m. Its next update gives the same request. *)
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
State.Setpoint := LREAL#0.50;
State.ProcessValue := LREAL#0.40;
State.NominalOutput := LREAL#3.562;
State.SaturatedOutput := LREAL#3.562;
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#0.50;
Input.ProcessValue := LREAL#0.42;
ASSERT_NEAR(EXPECTED := LREAL#-3.7516, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM ClampAndTracking
VAR
    Controller : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* A 1.5 rad target step from rest requests P + I* = 16 · 1.5 + 4 · 1.5 · 0.02 = 24.12 N·m: the drive gets
   20 N·m, and tracking corrects the retained request by (dt/Tt)(u_a − u_r) = 0.02 · (20 − 24.12) N·m. *)
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
Controller.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#1.5;
Input.ProcessValue := LREAL#0.0;
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-9);
State := Controller.GetState();
ASSERT_NEAR(EXPECTED := LREAL#24.12 + LREAL#0.02 * (LREAL#20.0 - LREAL#24.12), ACTUAL := State.NominalOutput, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
