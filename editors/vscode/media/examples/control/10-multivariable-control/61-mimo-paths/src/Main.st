(* Steady rises for heater changes V1 and V2 from 75 %, in °C: the rows of K = [[9/65, 4/65], [4/65, 9/65]]. *)
FUNCTION SteadyRise1C : LREAL
VAR_INPUT
    V1Pct : LREAL;
    V2Pct : LREAL;
END_VAR
SteadyRise1C := (LREAL#9.0 * V1Pct + LREAL#4.0 * V2Pct) / LREAL#65.0;
END_FUNCTION

FUNCTION SteadyRise2C : LREAL
VAR_INPUT
    V1Pct : LREAL;
    V2Pct : LREAL;
END_VAR
SteadyRise2C := (LREAL#4.0 * V1Pct + LREAL#9.0 * V2Pct) / LREAL#65.0;
END_FUNCTION

(* The two balances, advanced by one explicit step of UpdateS: both rates come from the old temperatures and are
   calculated before either temperature is stored. The wall flow leaves vessel 1 and enters vessel 2. *)
FUNCTION_BLOCK CoupledVesselModel
VAR_INPUT
    Heater1Pct : LREAL;
    Heater2Pct : LREAL;
    UpdateS : LREAL := 0.1;
END_VAR
VAR_OUTPUT
    Vessel1C : LREAL := 35.0;
    Vessel2C : LREAL := 35.0;
    TransferKw : LREAL := 0.0;
END_VAR
VAR
    Rate1 : LREAL;
    Rate2 : LREAL;
END_VAR
TransferKw := LREAL#8.0 * (Vessel1C - Vessel2C);
Rate1 := (LREAL#2.0 * Heater1Pct - LREAL#10.0 * (Vessel1C - LREAL#20.0) - TransferKw) / LREAL#50.0;
Rate2 := (LREAL#2.0 * Heater2Pct - LREAL#10.0 * (Vessel2C - LREAL#20.0) + TransferKw) / LREAL#50.0;
Vessel1C := Vessel1C + UpdateS * Rate1;
Vessel2C := Vessel2C + UpdateS * Rate2;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Two vessels coupled through a wall, each with its own heater. No controller acts: each path is measured with the
       other heater fixed. *)
    Model : CoupledVesselModel;
    Heater1Pct : LREAL := 75.0;
    Heater2Pct : LREAL := 75.0;
    Vessel1C : LREAL := 35.0;
    Vessel2C : LREAL := 35.0;
    (* Control Studio's project experiment raises heater 1 to 80 % for 30 s, returns it to 75 % for 30 s, then raises
       heater 2 to 80 %. *)
    ElapsedS : LREAL := 0.0;
    Model1C : LREAL := 35.0;
    Model2C : LREAL := 35.0;
    TransferKw : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
END_VAR
(* The model takes the commands of the previous update, so its temperatures belong to the same instant as the
   measured ones. *)
Model(Heater1Pct := Heater1Pct, Heater2Pct := Heater2Pct);
Model1C := Model.Vessel1C;
Model2C := Model.Vessel2C;
TransferKw := Model.TransferKw;
Heater1Pct := LREAL#75.0;
Heater2Pct := LREAL#75.0;
IF ExperimentEnabled THEN
    IF ElapsedS < LREAL#29.95 THEN
        Heater1Pct := LREAL#80.0;
    ELSIF ElapsedS >= LREAL#59.95 THEN
        Heater2Pct := LREAL#80.0;
    END_IF;
    ElapsedS := ElapsedS + LREAL#0.1;
ELSE
    ElapsedS := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Heater1Pct AT %QL0 : LREAL;
    P1.Heater2Pct AT %QL8 : LREAL;
    P1.Vessel1C AT %IL0 : LREAL;
    P1.Vessel2C AT %IL8 : LREAL;
    P1.Model1C AT %ML0 : LREAL;
    P1.Model2C AT %ML8 : LREAL;
    P1.TransferKw AT %ML16 : LREAL;
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
END_VAR
END_CONFIGURATION
