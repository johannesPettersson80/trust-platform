(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
USING Control;

TEST_PROGRAM GainChangeRebasesTheIntegral
VAR
    OldP : LREAL;
    NewP : LREAL;
END_VAR
(* 2 °C error, Kp 2 → 3: P 4 → 6 points; I 71 → 69 % keeps the 75 % request. *)
OldP := WeightedProportional(ProportionalGain := LREAL#2.0, SetpointWeight := LREAL#1.0,
    Setpoint := LREAL#37.0, ProcessValue := LREAL#35.0, OperatingPoint := LREAL#35.0);
NewP := WeightedProportional(ProportionalGain := LREAL#3.0, SetpointWeight := LREAL#1.0,
    Setpoint := LREAL#37.0, ProcessValue := LREAL#35.0, OperatingPoint := LREAL#35.0);
ASSERT_NEAR(EXPECTED := LREAL#4.0, ACTUAL := OldP, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#6.0, ACTUAL := NewP, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#69.0, ACTUAL := LREAL#71.0 + OldP - NewP, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := NewP + LREAL#69.0, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM InvalidSetsAreRejected
VAR
    Candidate : Control.PIDParameters;
    Broken : Control.PIDParameters;
END_VAR
Candidate.ProportionalGain := LREAL#13.0;
Candidate.IntegralGain := LREAL#3.5;
Candidate.ProportionalSetpointWeight := LREAL#0.3;
Candidate.OutputMinimum := LREAL#0.0;
Candidate.OutputMaximum := LREAL#100.0;
Candidate.RateMinimum := LREAL#-10000.0;
Candidate.RateMaximum := LREAL#10000.0;
Candidate.TrackingTime := LREAL#1.0;
ASSERT_TRUE(ValidPidParameters(Candidate := Candidate, FilterTime := LREAL#1.0));
ASSERT_FALSE(ValidPidParameters(Candidate := Candidate, FilterTime := LREAL#-1.0));
Broken := Candidate;
Broken.OutputMinimum := LREAL#80.0;
Broken.OutputMaximum := LREAL#20.0;
ASSERT_FALSE(ValidPidParameters(Candidate := Broken, FilterTime := LREAL#1.0));
Broken := Candidate;
Broken.ProportionalGain := LREAL#-1.0;
ASSERT_FALSE(ValidPidParameters(Candidate := Broken, FilterTime := LREAL#1.0));
Broken := Candidate;
Broken.ProportionalSetpointWeight := LREAL#1.5;
ASSERT_FALSE(ValidPidParameters(Candidate := Broken, FilterTime := LREAL#1.0));
Broken := Candidate;
Broken.TrackingTime := LREAL#0.0;
ASSERT_FALSE(ValidPidParameters(Candidate := Broken, FilterTime := LREAL#1.0));
END_TEST_PROGRAM

TEST_PROGRAM IncrementalFormKeepsTheCommand
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
(* Holding 38 °C at 90 %. *)
State.Setpoint := LREAL#38.0;
State.ProcessValue := LREAL#38.0;
State.NominalOutput := LREAL#90.0;
State.SaturatedOutput := LREAL#90.0;
Controller.Initialize(State := State);
Input.Setpoint := LREAL#38.0;
Input.ProcessValue := LREAL#38.0;
Input.Mode := PIDMode#Auto;
Input.CycleTime := LREAL#0.1;
ASSERT_NEAR(EXPECTED := LREAL#90.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
(* Kp 13 → 9: the positional P would change by 8.4 points; the incremental form keeps 90 %. *)
Parameters.ProportionalGain := LREAL#9.0;
Controller.Configure(Parameters := Parameters);
ASSERT_NEAR(EXPECTED := LREAL#90.0, ACTUAL := Controller.Update(Input := Input), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
