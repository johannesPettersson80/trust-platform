(* FiniteGridMpcController is the lesson's transparent model-predictive controller
   for two thermally coupled rooms. It searches a deliberately small finite move
   grid so every prediction, constraint, and objective comparison can be studied;
   it is not a general continuous quadratic-program solver. *)
FUNCTION_BLOCK FiniteGridMpcController
VAR_INPUT
    Request : BuildingMpcRequest;
END_VAR
VAR_OUTPUT
    Result : BuildingMpcResult;
END_VAR
VAR
    MoveChoices : ARRAY[0..2] OF LREAL;
    Moves1 : ARRAY[0..2] OF LREAL;
    Moves2 : ARRAY[0..2] OF LREAL;
    OutsideForecast : ARRAY[0..2] OF LREAL;
    Occupancy1Forecast : ARRAY[0..2] OF LREAL;
    Occupancy2Forecast : ARRAY[0..2] OF LREAL;
    TariffForecast : ARRAY[0..2] OF LREAL;
    I10 : DINT;
    I20 : DINT;
    I11 : DINT;
    I21 : DINT;
    I12 : DINT;
    I22 : DINT;
    Horizon : DINT;
    CandidateFeasible : BOOL;
    CandidateObjective : LREAL;
    CandidateTemperature1 : LREAL;
    CandidateTemperature2 : LREAL;
    NextTemperature1 : LREAL;
    NextTemperature2 : LREAL;
    CandidatePreviousHeat1 : LREAL;
    CandidatePreviousHeat2 : LREAL;
    CandidateHeat1 : LREAL;
    CandidateHeat2 : LREAL;
    CandidateError1 : LREAL;
    CandidateError2 : LREAL;
END_VAR

MoveChoices[0] := LREAL#-20.0;
MoveChoices[1] := LREAL#0.0;
MoveChoices[2] := LREAL#20.0;
OutsideForecast[0] := Request.Forecast.OutsideTemperature[0];
OutsideForecast[1] := Request.Forecast.OutsideTemperature[1];
OutsideForecast[2] := Request.Forecast.OutsideTemperature[2];
Occupancy1Forecast[0] := Request.Forecast.Zone1OccupancyGain[0];
Occupancy1Forecast[1] := Request.Forecast.Zone1OccupancyGain[1];
Occupancy1Forecast[2] := Request.Forecast.Zone1OccupancyGain[2];
Occupancy2Forecast[0] := Request.Forecast.Zone2OccupancyGain[0];
Occupancy2Forecast[1] := Request.Forecast.Zone2OccupancyGain[1];
Occupancy2Forecast[2] := Request.Forecast.Zone2OccupancyGain[2];
TariffForecast[0] := Request.Forecast.Tariff[0];
TariffForecast[1] := Request.Forecast.Tariff[1];
TariffForecast[2] := Request.Forecast.Tariff[2];

Result.ConfigurationValid := Request.UpstreamValid
    AND (Request.Zone1Temperature >= LREAL#15.0)
    AND (Request.Zone1Temperature <= LREAL#27.0)
    AND (Request.Zone2Temperature >= LREAL#15.0)
    AND (Request.Zone2Temperature <= LREAL#27.0)
    AND (Request.Zone1Setpoint >= LREAL#18.0)
    AND (Request.Zone1Setpoint <= LREAL#24.0)
    AND (Request.Zone2Setpoint >= LREAL#18.0)
    AND (Request.Zone2Setpoint <= LREAL#24.0)
    AND (Request.ComfortMinimum >= LREAL#18.0)
    AND (Request.ComfortMaximum <= LREAL#25.0)
    AND (Request.ComfortMinimum < Request.ComfortMaximum)
    AND (Request.TemperatureWeight > LREAL#0.0)
    AND (Request.TemperatureWeight <= LREAL#10000.0)
    AND (Request.EnergyWeight >= LREAL#0.0)
    AND (Request.EnergyWeight <= LREAL#100.0)
    AND (Request.MoveWeight >= LREAL#0.0)
    AND (Request.MoveWeight <= LREAL#100.0)
    AND (Request.PreviousZone1Heat >= LREAL#0.0)
    AND (Request.PreviousZone1Heat <= LREAL#100.0)
    AND (Request.PreviousZone2Heat >= LREAL#0.0)
    AND (Request.PreviousZone2Heat <= LREAL#100.0)
    AND (Request.OutsideCurrent >= LREAL#-10.0)
    AND (Request.OutsideCurrent <= LREAL#15.0)
    AND (Request.Occupancy1Current >= LREAL#0.0)
    AND (Request.Occupancy1Current <= LREAL#4.0)
    AND (Request.Occupancy2Current >= LREAL#0.0)
    AND (Request.Occupancy2Current <= LREAL#4.0);

FOR Horizon := DINT#0 TO DINT#2 DO
    Result.ConfigurationValid := Result.ConfigurationValid
        AND (OutsideForecast[Horizon] >= LREAL#-10.0)
        AND (OutsideForecast[Horizon] <= LREAL#15.0)
        AND (Occupancy1Forecast[Horizon] >= LREAL#0.0)
        AND (Occupancy1Forecast[Horizon] <= LREAL#4.0)
        AND (Occupancy2Forecast[Horizon] >= LREAL#0.0)
        AND (Occupancy2Forecast[Horizon] <= LREAL#4.0)
        AND (TariffForecast[Horizon] >= LREAL#0.0)
        AND (TariffForecast[Horizon] <= LREAL#10.0);
END_FOR;

Result.CandidateCount := DINT#0;
Result.FeasibleCandidateCount := DINT#0;
Result.BestObjective := LREAL#1.0E30;
Result.FirstZone1Heat := LREAL#0.0;
Result.FirstZone2Heat := LREAL#0.0;
Result.SolverComplete := FALSE;
Result.SolverFeasible := FALSE;
Result.Infeasible := FALSE;

IF Request.Enable AND Result.ConfigurationValid THEN
    (* Each pair of nested choices selects one move for each room at each of
       three future intervals: 3^6 = 729 complete candidate sequences. *)
    FOR I10 := DINT#0 TO DINT#2 DO
        FOR I20 := DINT#0 TO DINT#2 DO
            FOR I11 := DINT#0 TO DINT#2 DO
                FOR I21 := DINT#0 TO DINT#2 DO
                    FOR I12 := DINT#0 TO DINT#2 DO
                        FOR I22 := DINT#0 TO DINT#2 DO
                            Result.CandidateCount := Result.CandidateCount + DINT#1;
                            Moves1[0] := MoveChoices[I10];
                            Moves2[0] := MoveChoices[I20];
                            Moves1[1] := MoveChoices[I11];
                            Moves2[1] := MoveChoices[I21];
                            Moves1[2] := MoveChoices[I12];
                            Moves2[2] := MoveChoices[I22];
                            CandidateTemperature1 := Request.Zone1Temperature;
                            CandidateTemperature2 := Request.Zone2Temperature;
                            CandidatePreviousHeat1 := Request.PreviousZone1Heat;
                            CandidatePreviousHeat2 := Request.PreviousZone2Heat;
                            CandidateObjective := LREAL#0.0;
                            CandidateFeasible := TRUE;

                            FOR Horizon := DINT#0 TO DINT#2 DO
                                IF CandidateFeasible THEN
                                    CandidateHeat1 := CandidatePreviousHeat1 + Moves1[Horizon];
                                    CandidateHeat2 := CandidatePreviousHeat2 + Moves2[Horizon];
                                    CandidateFeasible := (CandidateHeat1 >= LREAL#0.0)
                                        AND (CandidateHeat1 <= LREAL#100.0)
                                        AND (CandidateHeat2 >= LREAL#0.0)
                                        AND (CandidateHeat2 <= LREAL#100.0)
                                        AND (CandidateHeat1 + CandidateHeat2 <= LREAL#170.0);
                                END_IF;
                                IF CandidateFeasible THEN
                                    NextTemperature1 := LREAL#0.875659571463 * CandidateTemperature1
                                        + LREAL#0.029177846573 * CandidateTemperature2
                                        + LREAL#0.018727354703 * CandidateHeat1
                                        + LREAL#0.000305161690 * CandidateHeat2
                                        + LREAL#0.095162581964 * OutsideForecast[Horizon]
                                        + LREAL#0.093636773515 * Occupancy1Forecast[Horizon]
                                        + LREAL#0.001525808449 * Occupancy2Forecast[Horizon]
                                        + Request.EstimatedBias1;
                                    NextTemperature2 := LREAL#0.029177846573 * CandidateTemperature1
                                        + LREAL#0.875659571463 * CandidateTemperature2
                                        + LREAL#0.000305161690 * CandidateHeat1
                                        + LREAL#0.018727354703 * CandidateHeat2
                                        + LREAL#0.095162581964 * OutsideForecast[Horizon]
                                        + LREAL#0.001525808449 * Occupancy1Forecast[Horizon]
                                        + LREAL#0.093636773515 * Occupancy2Forecast[Horizon]
                                        + Request.EstimatedBias2;
                                    CandidateFeasible := (NextTemperature1 >= Request.ComfortMinimum)
                                        AND (NextTemperature1 <= Request.ComfortMaximum)
                                        AND (NextTemperature2 >= Request.ComfortMinimum)
                                        AND (NextTemperature2 <= Request.ComfortMaximum);
                                END_IF;
                                IF CandidateFeasible THEN
                                    CandidateError1 := NextTemperature1 - Request.Zone1Setpoint;
                                    CandidateError2 := NextTemperature2 - Request.Zone2Setpoint;
                                    CandidateObjective := CandidateObjective
                                        + Request.TemperatureWeight
                                            * (CandidateError1 * CandidateError1
                                                + CandidateError2 * CandidateError2)
                                        + Request.EnergyWeight * TariffForecast[Horizon]
                                            * (CandidateHeat1 + CandidateHeat2)
                                        + Request.MoveWeight
                                            * (Moves1[Horizon] * Moves1[Horizon]
                                                + Moves2[Horizon] * Moves2[Horizon]);
                                    CandidateTemperature1 := NextTemperature1;
                                    CandidateTemperature2 := NextTemperature2;
                                    CandidatePreviousHeat1 := CandidateHeat1;
                                    CandidatePreviousHeat2 := CandidateHeat2;
                                END_IF;
                            END_FOR;

                            IF CandidateFeasible THEN
                                Result.FeasibleCandidateCount := Result.FeasibleCandidateCount + DINT#1;
                                IF CandidateObjective < Result.BestObjective THEN
                                    Result.BestObjective := CandidateObjective;
                                    (* Receding-horizon control applies only the first move. The
                                       next scan measures the plant again and solves a new horizon. *)
                                    Result.FirstZone1Heat := Request.PreviousZone1Heat + Moves1[0];
                                    Result.FirstZone2Heat := Request.PreviousZone2Heat + Moves2[0];
                                END_IF;
                            END_IF;
                        END_FOR;
                    END_FOR;
                END_FOR;
            END_FOR;
        END_FOR;
    END_FOR;
    Result.SolverComplete := TRUE;
    Result.SolverFeasible := Result.FeasibleCandidateCount > DINT#0;
    Result.Infeasible := NOT Result.SolverFeasible;
END_IF;
END_FUNCTION_BLOCK
