(* One heater's part of a plan: three one-second predictions, T + (2·u − 10·(T − 20) − D)/50. The first second uses the
   previous command, which the heater is still receiving; the next two use UPct. The part is feasible when UPct lies in
   0–100 points and every predicted temperature in MinimumC–MaximumC. Its cost is the sum of the three squared temperature
   errors plus MoveWeight times the squared command change. *)
FUNCTION_BLOCK HeaterPlan
VAR_INPUT
    TemperatureC : LREAL;
    PreviousPct : LREAL;
    UPct : LREAL;
    SetpointC : LREAL;
    LoadKw : LREAL;
    MoveWeight : LREAL;
    MinimumC : LREAL;
    MaximumC : LREAL;
END_VAR
VAR_OUTPUT
    Feasible : BOOL := FALSE;
    Cost : LREAL := 0.0;
    FirstC : LREAL := 0.0;
    SecondC : LREAL := 0.0;
    EndC : LREAL := 0.0;
END_VAR
FirstC := TemperatureC + (LREAL#2.0 * PreviousPct - LREAL#10.0 * (TemperatureC - LREAL#20.0) - LoadKw) / LREAL#50.0;
SecondC := FirstC + (LREAL#2.0 * UPct - LREAL#10.0 * (FirstC - LREAL#20.0) - LoadKw) / LREAL#50.0;
EndC := SecondC + (LREAL#2.0 * UPct - LREAL#10.0 * (SecondC - LREAL#20.0) - LoadKw) / LREAL#50.0;
Feasible := UPct >= LREAL#0.0 AND UPct <= LREAL#100.0 AND FirstC >= MinimumC AND FirstC <= MaximumC
    AND SecondC >= MinimumC AND SecondC <= MaximumC AND EndC >= MinimumC AND EndC <= MaximumC;
Cost := (FirstC - SetpointC) * (FirstC - SetpointC) + (SecondC - SetpointC) * (SecondC - SetpointC)
    + (EndC - SetpointC) * (EndC - SetpointC) + MoveWeight * (UPct - PreviousPct) * (UPct - PreviousPct);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* Two heaters share a 100-point supply. Every second the program plans both commands together over the 231 pairs on
       a 5-point grid with u1 + u2 ≤ 100: a plan is rejected when either heater's prediction leaves 26–33 °C, and its cost
       is the sum of the two heaters' costs, since they share only the supply. So each heater is predicted once for each
       of its 21 commands, and the pairs are compared by adding the two costs. The program applies the pair of the
       cheapest plan and plans again at the next update. Without a feasible plan it holds the commands and reports the
       planning failure. The project experiment steps the setpoints to 31 °C and 28 °C and, from 60 s, takes 30 kW more
       from heater 2. A temperature outside 0–100 °C or an update interval outside 1 s ± 50 ms latches zero output. *)
    Heater1C : LREAL := 28.0;
    Heater2C : LREAL := 28.0;
    Command1Pct : LREAL := 40.0;
    Command2Pct : LREAL := 40.0;
    Load1Kw : LREAL := 0.0;
    Load2Kw : LREAL := 0.0;
    Setpoint1C : LREAL := 28.0;
    Setpoint2C : LREAL := 28.0;
    SupplyPct : LREAL := 80.0;
    Predicted1C : LREAL := 28.0;
    Predicted2C : LREAL := 28.0;
    PlanCost : LREAL := 0.0;
    RejectedPlans : LREAL := 0.0;
    MoveWeight : LREAL := 0.002;
    (* The commands the heaters received last update, unbound so that they hold the resting 40 points from the first
       update. *)
    Previous1Pct : LREAL := 40.0;
    Previous2Pct : LREAL := 40.0;
    Best1Pct : LREAL;
    Best2Pct : LREAL;
    BestCost : LREAL;
    Found : BOOL;
    I : INT;
    J : INT;
    Plan1 : HeaterPlan;
    Plan2 : HeaterPlan;
    Cost1 : ARRAY[0..20] OF LREAL;
    Cost2 : ARRAY[0..20] OF LREAL;
    End1 : ARRAY[0..20] OF LREAL;
    End2 : ARRAY[0..20] OF LREAL;
    Feasible1 : ARRAY[0..20] OF BOOL;
    Feasible2 : ARRAY[0..20] OF BOOL;
    ElapsedS : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    WasEnabled : BOOL := FALSE;
    PlanningFailed : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    ClockPrimed : BOOL := FALSE;
    ClockValid : BOOL := TRUE;
    NowTime : TIME;
    PreviousTime : TIME;
END_VAR
NowTime := TIME();
ClockValid := NOT ClockPrimed OR ((NowTime - PreviousTime) >= T#950ms AND (NowTime - PreviousTime) <= T#1050ms);
IF NOT ClockValid OR Heater1C < LREAL#0.0 OR Heater1C > LREAL#100.0 OR Heater2C < LREAL#0.0 OR Heater2C > LREAL#100.0 THEN
    FaultLatched := TRUE;
END_IF;
IF ExperimentEnabled AND NOT WasEnabled THEN
    ElapsedS := LREAL#0.0;
ELSIF ExperimentEnabled THEN
    ElapsedS := ElapsedS + LREAL#1.0;
END_IF;
Setpoint1C := LREAL#28.0;
Setpoint2C := LREAL#28.0;
Load2Kw := LREAL#0.0;
IF ExperimentEnabled THEN
    Setpoint1C := LREAL#31.0;
    IF ElapsedS >= LREAL#59.5 THEN
        Load2Kw := LREAL#30.0;
    END_IF;
END_IF;
IF FaultLatched THEN
    Command1Pct := LREAL#0.0;
    Command2Pct := LREAL#0.0;
ELSE
    Found := FALSE;
    BestCost := LREAL#0.0;
    RejectedPlans := LREAL#0.0;
    FOR I := 0 TO 20 DO
        Plan1(TemperatureC := Heater1C, PreviousPct := Previous1Pct, UPct := LREAL#5.0 * INT_TO_LREAL(I), SetpointC := Setpoint1C,
            LoadKw := Load1Kw, MoveWeight := MoveWeight, MinimumC := LREAL#26.0, MaximumC := LREAL#33.0);
        Plan2(TemperatureC := Heater2C, PreviousPct := Previous2Pct, UPct := LREAL#5.0 * INT_TO_LREAL(I), SetpointC := Setpoint2C,
            LoadKw := Load2Kw, MoveWeight := MoveWeight, MinimumC := LREAL#26.0, MaximumC := LREAL#33.0);
        Cost1[I] := Plan1.Cost;
        Cost2[I] := Plan2.Cost;
        End1[I] := Plan1.EndC;
        End2[I] := Plan2.EndC;
        Feasible1[I] := Plan1.Feasible;
        Feasible2[I] := Plan2.Feasible;
    END_FOR;
    FOR I := 0 TO 20 DO
        FOR J := 0 TO 20 - I DO
            IF NOT (Feasible1[I] AND Feasible2[J]) THEN
                RejectedPlans := RejectedPlans + LREAL#1.0;
            ELSIF NOT Found OR Cost1[I] + Cost2[J] < BestCost THEN
                Found := TRUE;
                BestCost := Cost1[I] + Cost2[J];
                Best1Pct := LREAL#5.0 * INT_TO_LREAL(I);
                Best2Pct := LREAL#5.0 * INT_TO_LREAL(J);
                Predicted1C := End1[I];
                Predicted2C := End2[J];
            END_IF;
        END_FOR;
    END_FOR;
    PlanningFailed := NOT Found;
    IF Found THEN
        Command1Pct := Best1Pct;
        Command2Pct := Best2Pct;
        PlanCost := BestCost;
    END_IF;
    Previous1Pct := Command1Pct;
    Previous2Pct := Command2Pct;
END_IF;
SupplyPct := Command1Pct + Command2Pct;
WasEnabled := ExperimentEnabled;
PreviousTime := NowTime;
ClockPrimed := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#1s, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Heater1C AT %IL0 : LREAL;
    P1.Heater2C AT %IL8 : LREAL;
    P1.Command1Pct AT %QL0 : LREAL;
    P1.Load1Kw AT %QL8 : LREAL;
    P1.Command2Pct AT %QL16 : LREAL;
    P1.Load2Kw AT %QL24 : LREAL;
    P1.Setpoint1C AT %ML0 : LREAL;
    P1.Setpoint2C AT %ML8 : LREAL;
    P1.SupplyPct AT %ML16 : LREAL;
    P1.Predicted1C AT %ML24 : LREAL;
    P1.Predicted2C AT %ML32 : LREAL;
    P1.PlanCost AT %ML40 : LREAL;
    P1.RejectedPlans AT %ML48 : LREAL;
    P1.ExperimentEnabled AT %MX56.0 : BOOL;
    P1.PlanningFailed AT %MX56.1 : BOOL;
    P1.FaultLatched AT %MX56.2 : BOOL;
    P1.ClockValid AT %MX56.3 : BOOL;
END_VAR
END_CONFIGURATION
