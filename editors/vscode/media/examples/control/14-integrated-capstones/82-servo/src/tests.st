(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM TheLoadFallsWhileTheDriveHasNoTorque
(* J = 1 kg·m², B = 1 N·m·s/rad, τL = 1.962 N·m: 0.16 s without torque drops the load 0.0238 rad, 0.76 s 0.447 rad. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.0238, ACTUAL := LoadDropRad(LoadTorque := LREAL#1.962, Inertia := LREAL#1.0,
    Friction := LREAL#1.0, Seconds := LREAL#0.16), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#-0.447, ACTUAL := LoadDropRad(LoadTorque := LREAL#1.962, Inertia := LREAL#1.0,
    Friction := LREAL#1.0, Seconds := LREAL#0.76), DELTA := LREAL#1.0e-3);
END_TEST_PROGRAM

TEST_PROGRAM StaleAndInvalidReadingsAreRejected
(* Fresh means at most two 20 ms updates old; the encoder reports −π to π rad. *)
ASSERT_TRUE(ReadingUsable(QualityGood := TRUE, Value := LREAL#1.0, Age := LREAL#0.04, MaximumAge := LREAL#0.04));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#1.0, Age := LREAL#0.06, MaximumAge := LREAL#0.04));
ASSERT_FALSE(ReadingUsable(QualityGood := FALSE, Value := LREAL#1.0, Age := LREAL#0.0, MaximumAge := LREAL#0.04));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#4.0, Age := LREAL#0.0, MaximumAge := LREAL#0.04));
END_TEST_PROGRAM

TEST_PROGRAM OnlyAWholeValidSetIsAdopted
VAR
    Candidate : Control.PIDParameters;
END_VAR
Candidate.ProportionalGain := LREAL#16.0;
Candidate.IntegralGain := LREAL#12.0;
Candidate.DerivativeGain := LREAL#7.0;
Candidate.ProportionalSetpointWeight := LREAL#0.3;
Candidate.OutputMinimum := LREAL#-20.0;
Candidate.OutputMaximum := LREAL#20.0;
Candidate.RateMinimum := LREAL#-10000.0;
Candidate.RateMaximum := LREAL#10000.0;
Candidate.TrackingTime := LREAL#1.0;
ASSERT_TRUE(ValidPidParameters(Candidate := Candidate, FilterTime := LREAL#0.1));
ASSERT_FALSE(ValidPidParameters(Candidate := Candidate, FilterTime := LREAL#-0.1));
Candidate.IntegralGain := LREAL#-1.0;
ASSERT_FALSE(ValidPidParameters(Candidate := Candidate, FilterTime := LREAL#0.1));
END_TEST_PROGRAM

TEST_PROGRAM TheFirstUpdateOfEachSet
VAR
    Installed : Control.PID;
    Candidate : Control.PID;
    Parameters : Control.PIDParameters;
    State : Control.PIDInitialState;
    Input : Control.PIDInput;
END_VAR
(* At rest, a 1 rad target change: the installed set adds Kp·β·Δr + Ki·e·Δt = 16 + 4 · 0.02 = 16.08 N·m,
   the candidate 16 · 0.3 + 12 · 0.02 = 5.04 N·m. *)
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
Installed.Configure(Parameters := Parameters);
Installed.Initialize(State := State);
Parameters.IntegralGain := LREAL#12.0;
Parameters.ProportionalSetpointWeight := LREAL#0.3;
Candidate.Configure(Parameters := Parameters);
Candidate.Initialize(State := State);
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.02;
Input.Setpoint := LREAL#1.0;
ASSERT_NEAR(EXPECTED := LREAL#16.08, ACTUAL := Installed.Update(Input := Input), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#5.04, ACTUAL := Candidate.Update(Input := Input), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
