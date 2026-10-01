(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheWorkedPlans
(* Heater 1 at 28 °C, still receiving 40 points, setpoint 31 °C: holding 40 points costs 3·3² = 27; 55 points predicts
   28, 28.6 and 29.08 °C and costs 9 + 5.76 + 3.6864 + 0.002·15² = 18.8964; 60 points costs 17.0736. *)
VAR
    Plan : HeaterPlan;
END_VAR
Plan(TemperatureC := LREAL#28.0, PreviousPct := LREAL#40.0, UPct := LREAL#40.0, SetpointC := LREAL#31.0, LoadKw := LREAL#0.0, MoveWeight := LREAL#0.002, MinimumC := LREAL#26.0, MaximumC := LREAL#33.0);
ASSERT_NEAR(EXPECTED := LREAL#27.0, ACTUAL := Plan.Cost, DELTA := LREAL#1.0e-9);
Plan(UPct := LREAL#55.0);
ASSERT_NEAR(EXPECTED := LREAL#28.0, ACTUAL := Plan.FirstC, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#28.6, ACTUAL := Plan.SecondC, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#29.08, ACTUAL := Plan.EndC, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#18.8964, ACTUAL := Plan.Cost, DELTA := LREAL#1.0e-9);
Plan(UPct := LREAL#60.0);
ASSERT_NEAR(EXPECTED := LREAL#17.0736, ACTUAL := Plan.Cost, DELTA := LREAL#1.0e-9);
ASSERT_TRUE(Plan.Feasible);
END_TEST_PROGRAM

TEST_PROGRAM TheLimits
(* Heater 2 at 28 °C with no command predicts 28, 26.4 and 25.12 °C: below 26 °C, so the plan is rejected. A command outside
   0–100 points is rejected too. *)
VAR
    Plan : HeaterPlan;
END_VAR
Plan(TemperatureC := LREAL#28.0, PreviousPct := LREAL#40.0, UPct := LREAL#0.0, SetpointC := LREAL#28.0, LoadKw := LREAL#0.0, MoveWeight := LREAL#0.002, MinimumC := LREAL#26.0, MaximumC := LREAL#33.0);
ASSERT_NEAR(EXPECTED := LREAL#25.12, ACTUAL := Plan.EndC, DELTA := LREAL#1.0e-12);
ASSERT_FALSE(Plan.Feasible);
Plan(UPct := LREAL#105.0);
ASSERT_FALSE(Plan.Feasible);
END_TEST_PROGRAM

TEST_PROGRAM TheSteadyCommands
(* At steady state 2u = 10(T − 20) + D: 31 °C needs 55 points; 28 °C needs 40 points, or 55 with 30 kW more taken. *)
VAR
    Plan : HeaterPlan;
END_VAR
Plan(TemperatureC := LREAL#31.0, PreviousPct := LREAL#55.0, UPct := LREAL#55.0, SetpointC := LREAL#31.0, LoadKw := LREAL#0.0, MoveWeight := LREAL#0.002, MinimumC := LREAL#26.0, MaximumC := LREAL#33.0);
ASSERT_NEAR(EXPECTED := LREAL#31.0, ACTUAL := Plan.EndC, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Plan.Cost, DELTA := LREAL#1.0e-12);
Plan(TemperatureC := LREAL#28.0, PreviousPct := LREAL#55.0, UPct := LREAL#55.0, SetpointC := LREAL#28.0, LoadKw := LREAL#30.0);
ASSERT_NEAR(EXPECTED := LREAL#28.0, ACTUAL := Plan.EndC, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
