(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson73PhysicalEquilibriumAndOwnedScheduleAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#80.0,
    ACTUAL := Lesson73EquilibriumHeat(
        IndoorTemperature := LREAL#21.0,
        OutdoorTemperature := LREAL#5.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#5.0,
    ACTUAL := Lesson73OutsideTemperature(
        ExperimentElapsedSeconds := LREAL#300.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0,
    ACTUAL := Lesson73OutsideTemperature(
        ExperimentElapsedSeconds := LREAL#360.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson73OccupancyGain(
        ExperimentElapsedSeconds := LREAL#479.0,
        EventTimeSeconds := LREAL#480.0,
        GainAfterEvent := LREAL#2.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0,
    ACTUAL := Lesson73OccupancyGain(
        ExperimentElapsedSeconds := LREAL#480.0,
        EventTimeSeconds := LREAL#480.0,
        GainAfterEvent := LREAL#2.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM Lesson73MpcValidEquilibriumSearchesFullGrid
VAR
    Controller : FiniteGridMpcController;
    Request : BuildingMpcRequest;
END_VAR
Request.Enable := TRUE; Request.UpstreamValid := TRUE;
Request.Zone1Temperature := LREAL#21.0; Request.Zone2Temperature := LREAL#21.0;
Request.Zone1Setpoint := LREAL#21.0; Request.Zone2Setpoint := LREAL#21.0;
Request.ComfortMinimum := LREAL#18.0; Request.ComfortMaximum := LREAL#25.0;
Request.TemperatureWeight := LREAL#100.0; Request.EnergyWeight := LREAL#0.0; Request.MoveWeight := LREAL#1.0;
Request.PreviousZone1Heat := LREAL#80.0; Request.PreviousZone2Heat := LREAL#80.0;
Request.OutsideCurrent := LREAL#5.0; Request.Occupancy1Current := LREAL#0.0; Request.Occupancy2Current := LREAL#0.0;
Request.Forecast.OutsideTemperature[0] := LREAL#5.0; Request.Forecast.OutsideTemperature[1] := LREAL#5.0; Request.Forecast.OutsideTemperature[2] := LREAL#5.0;
Request.Forecast.Tariff[0] := LREAL#0.0; Request.Forecast.Tariff[1] := LREAL#0.0; Request.Forecast.Tariff[2] := LREAL#0.0;
Controller(Request := Request);
ASSERT_TRUE(IN := Controller.Result.ConfigurationValid AND Controller.Result.SolverComplete AND Controller.Result.SolverFeasible);
ASSERT_TRUE(IN := Controller.Result.CandidateCount = DINT#729 AND Controller.Result.FeasibleCandidateCount > DINT#0);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := Controller.Result.FirstZone1Heat, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := Controller.Result.FirstZone2Heat, DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Controller.Result.FirstZone1Heat >= LREAL#0.0 AND Controller.Result.FirstZone1Heat <= LREAL#100.0);
ASSERT_TRUE(IN := Controller.Result.FirstZone2Heat >= LREAL#0.0 AND Controller.Result.FirstZone2Heat <= LREAL#100.0);
ASSERT_TRUE(IN := Controller.Result.FirstZone1Heat + Controller.Result.FirstZone2Heat <= LREAL#170.0);
END_TEST_PROGRAM

TEST_PROGRAM Lesson73MpcChangedStateAndForecastReplansFromEquilibriumMove
VAR
    Controller : FiniteGridMpcController;
    Request : BuildingMpcRequest;
END_VAR
Request.Enable := TRUE; Request.UpstreamValid := TRUE;
Request.Zone1Temperature := LREAL#24.5; Request.Zone2Temperature := LREAL#24.5;
Request.Zone1Setpoint := LREAL#21.0; Request.Zone2Setpoint := LREAL#21.0;
Request.ComfortMinimum := LREAL#18.0; Request.ComfortMaximum := LREAL#25.0;
Request.TemperatureWeight := LREAL#100.0; Request.EnergyWeight := LREAL#0.0; Request.MoveWeight := LREAL#1.0;
Request.PreviousZone1Heat := LREAL#80.0; Request.PreviousZone2Heat := LREAL#80.0;
Request.OutsideCurrent := LREAL#15.0; Request.Occupancy1Current := LREAL#0.0; Request.Occupancy2Current := LREAL#0.0;
Request.Forecast.OutsideTemperature[0] := LREAL#15.0; Request.Forecast.OutsideTemperature[1] := LREAL#15.0; Request.Forecast.OutsideTemperature[2] := LREAL#15.0;
Request.Forecast.Tariff[0] := LREAL#0.0; Request.Forecast.Tariff[1] := LREAL#0.0; Request.Forecast.Tariff[2] := LREAL#0.0;
Controller(Request := Request);
ASSERT_TRUE(IN := Controller.Result.ConfigurationValid AND Controller.Result.SolverComplete AND Controller.Result.SolverFeasible);
ASSERT_TRUE(IN := Controller.Result.CandidateCount = DINT#729 AND Controller.Result.FeasibleCandidateCount > DINT#0);
ASSERT_TRUE(IN := Controller.Result.FirstZone1Heat <> LREAL#80.0 AND Controller.Result.FirstZone2Heat <> LREAL#80.0);
ASSERT_TRUE(IN := Controller.Result.FirstZone1Heat >= LREAL#0.0 AND Controller.Result.FirstZone1Heat <= LREAL#100.0);
ASSERT_TRUE(IN := Controller.Result.FirstZone2Heat >= LREAL#0.0 AND Controller.Result.FirstZone2Heat <= LREAL#100.0);
ASSERT_TRUE(IN := Controller.Result.FirstZone1Heat + Controller.Result.FirstZone2Heat <= LREAL#170.0);
END_TEST_PROGRAM

TEST_PROGRAM Lesson73MpcReportsInfeasibleComfortRequest
VAR
    Controller : FiniteGridMpcController;
    Request : BuildingMpcRequest;
END_VAR
Request.Enable := TRUE; Request.UpstreamValid := TRUE;
Request.Zone1Temperature := LREAL#15.0; Request.Zone2Temperature := LREAL#15.0;
Request.Zone1Setpoint := LREAL#21.0; Request.Zone2Setpoint := LREAL#21.0;
Request.ComfortMinimum := LREAL#18.0; Request.ComfortMaximum := LREAL#25.0;
Request.TemperatureWeight := LREAL#100.0; Request.EnergyWeight := LREAL#0.0; Request.MoveWeight := LREAL#1.0;
Request.PreviousZone1Heat := LREAL#0.0; Request.PreviousZone2Heat := LREAL#0.0;
Request.OutsideCurrent := LREAL#-10.0; Request.Occupancy1Current := LREAL#0.0; Request.Occupancy2Current := LREAL#0.0;
Request.Forecast.OutsideTemperature[0] := LREAL#-10.0; Request.Forecast.OutsideTemperature[1] := LREAL#-10.0; Request.Forecast.OutsideTemperature[2] := LREAL#-10.0;
Request.Forecast.Tariff[0] := LREAL#0.0; Request.Forecast.Tariff[1] := LREAL#0.0; Request.Forecast.Tariff[2] := LREAL#0.0;
Controller(Request := Request);
ASSERT_TRUE(IN := Controller.Result.ConfigurationValid AND Controller.Result.SolverComplete);
ASSERT_FALSE(IN := Controller.Result.SolverFeasible);
ASSERT_TRUE(IN := Controller.Result.Infeasible AND Controller.Result.FeasibleCandidateCount = DINT#0);
END_TEST_PROGRAM

TEST_PROGRAM Lesson73MpcDisabledRequestClearsSolverResult
VAR
    Controller : FiniteGridMpcController;
    Request : BuildingMpcRequest;
END_VAR
Request.Enable := TRUE; Request.UpstreamValid := TRUE;
Request.Zone1Temperature := LREAL#21.0; Request.Zone2Temperature := LREAL#21.0;
Request.Zone1Setpoint := LREAL#21.0; Request.Zone2Setpoint := LREAL#21.0;
Request.ComfortMinimum := LREAL#18.0; Request.ComfortMaximum := LREAL#25.0;
Request.TemperatureWeight := LREAL#100.0; Request.EnergyWeight := LREAL#0.0; Request.MoveWeight := LREAL#1.0;
Request.PreviousZone1Heat := LREAL#80.0; Request.PreviousZone2Heat := LREAL#80.0;
Request.OutsideCurrent := LREAL#5.0; Request.Occupancy1Current := LREAL#0.0; Request.Occupancy2Current := LREAL#0.0;
Request.Forecast.OutsideTemperature[0] := LREAL#5.0; Request.Forecast.OutsideTemperature[1] := LREAL#5.0; Request.Forecast.OutsideTemperature[2] := LREAL#5.0;
Request.Forecast.Tariff[0] := LREAL#0.0; Request.Forecast.Tariff[1] := LREAL#0.0; Request.Forecast.Tariff[2] := LREAL#0.0;
Controller(Request := Request);
ASSERT_TRUE(IN := Controller.Result.ConfigurationValid AND Controller.Result.SolverComplete AND Controller.Result.SolverFeasible);
Request.Enable := FALSE;
Controller(Request := Request);
ASSERT_TRUE(IN := Controller.Result.ConfigurationValid);
ASSERT_FALSE(IN := Controller.Result.SolverComplete OR Controller.Result.SolverFeasible OR Controller.Result.Infeasible);
ASSERT_TRUE(IN := Controller.Result.CandidateCount = DINT#0 AND Controller.Result.FeasibleCandidateCount = DINT#0);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Result.FirstZone1Heat, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Result.FirstZone2Heat, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM Lesson73MpcInvalidRequestClearsFreshFeasibleResult
VAR
    Controller : FiniteGridMpcController;
    Request : BuildingMpcRequest;
END_VAR
Request.Enable := TRUE; Request.UpstreamValid := TRUE;
Request.Zone1Temperature := LREAL#21.0; Request.Zone2Temperature := LREAL#21.0;
Request.Zone1Setpoint := LREAL#21.0; Request.Zone2Setpoint := LREAL#21.0;
Request.ComfortMinimum := LREAL#18.0; Request.ComfortMaximum := LREAL#25.0;
Request.TemperatureWeight := LREAL#100.0; Request.EnergyWeight := LREAL#0.0; Request.MoveWeight := LREAL#1.0;
Request.PreviousZone1Heat := LREAL#80.0; Request.PreviousZone2Heat := LREAL#80.0;
Request.OutsideCurrent := LREAL#5.0; Request.Occupancy1Current := LREAL#0.0; Request.Occupancy2Current := LREAL#0.0;
Request.Forecast.OutsideTemperature[0] := LREAL#5.0; Request.Forecast.OutsideTemperature[1] := LREAL#5.0; Request.Forecast.OutsideTemperature[2] := LREAL#5.0;
Request.Forecast.Tariff[0] := LREAL#0.0; Request.Forecast.Tariff[1] := LREAL#0.0; Request.Forecast.Tariff[2] := LREAL#0.0;
Controller(Request := Request);
ASSERT_TRUE(IN := Controller.Result.ConfigurationValid AND Controller.Result.SolverComplete AND Controller.Result.SolverFeasible);
ASSERT_TRUE(IN := Controller.Result.CandidateCount = DINT#729 AND Controller.Result.FeasibleCandidateCount > DINT#0);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := Controller.Result.FirstZone1Heat, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := Controller.Result.FirstZone2Heat, DELTA := LREAL#1.0e-12);
Request.UpstreamValid := FALSE;
Controller(Request := Request);
ASSERT_FALSE(IN := Controller.Result.ConfigurationValid OR Controller.Result.SolverComplete OR Controller.Result.SolverFeasible OR Controller.Result.Infeasible);
ASSERT_TRUE(IN := Controller.Result.CandidateCount = DINT#0 AND Controller.Result.FeasibleCandidateCount = DINT#0);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Result.FirstZone1Heat, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Controller.Result.FirstZone2Heat, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
