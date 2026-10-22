(* The plant and its operator, simulated in a task of their own so that the vat runs whole batches
   on a bench with no plant wired. A real installation drops this program and wires FieldSignals to
   its I/O.

   The vat: the inlet fills it at 2 %/s with product at 8 C; the outlet with the drain pump running
   empties it at 2.5 %/s. The jacket heats the product at up to 0.25 C/s in a vat filled to 80 %,
   faster in a vat that holds less, and half as fast without agitation; the cooling water draws it
   towards 4 C with a time constant of 100 s; it loses heat towards 20 C slowly. Valves open
   1.5 s after they are commanded, motors run 0.5 s after they are started.

   The operator starts the bench recipe when the vat is idle, acknowledges a new alarm after 5 s and
   resets an aborted batch once its alarms are acknowledged. Every second batch the steam supply
   fails for 20 s while the vat holds, which restarts the hold; every third batch the agitator's
   contactor drops for 4 s while the vat heats, which aborts the batch. *)
PROGRAM PlantSimulation
VAR_EXTERNAL
    Io : FieldSignals;
    Panel : OperatorCommands;
    Status : VatStatus;
END_VAR
VAR
    LevelPercent : REAL := 0.0;
    Temp : REAL := 8.0;
    Seconds : REAL;
    Now : TIME;
    LastTime : TIME;
    Started : BOOL;
    InletTravel : TON;
    OutletTravel : TON;
    CoolingTravel : TON;
    AgitatorStart : TON;
    PumpStart : TON;
    HeatRate : REAL;
    (* The operator: a press lasts 0.3 s, as a finger on a button does. *)
    WaitIdle : TON;
    WaitAlarm : TON;
    WaitAborted : TON;
    Press : TP;
    Pressing : INT;
    (* The disturbances, by the batch they happen in and the time in the step. *)
    SimulatedBatches : UDINT;
    LastState : VatState := VatState#Idle;
    SteamFailure : BOOL;
    ContactorDrop : BOOL;
END_VAR

Now := TIME();
IF NOT Started THEN
    LastTime := Now;
    Started := TRUE;
END_IF;
Seconds := UDINT_TO_REAL(DWORD_TO_UDINT(TIME_TO_DWORD(Now - LastTime))) / 1000.0;
LastTime := Now;

(* The equipment follows its outputs. *)
InletTravel(IN := Io.InletValve, PT := T#1500ms);
OutletTravel(IN := Io.OutletValve, PT := T#1500ms);
CoolingTravel(IN := Io.CoolingValve, PT := T#1500ms);
AgitatorStart(IN := Io.Agitator, PT := T#500ms);
PumpStart(IN := Io.Pump, PT := T#500ms);
IF Status.State <> LastState THEN
    IF Status.State = VatState#Filling THEN
        SimulatedBatches := SimulatedBatches + 1;
    END_IF;
    LastState := Status.State;
END_IF;
SteamFailure := SimulatedBatches MOD 2 = 0 AND Status.State = VatState#Holding
    AND Status.StepTime >= T#10s AND Status.StepTime < T#30s;
ContactorDrop := SimulatedBatches MOD 3 = 0 AND Status.State = VatState#Heating
    AND Status.StepTime >= T#20s AND Status.StepTime < T#24s;
Io.InletOpen := InletTravel.Q;
Io.OutletOpen := OutletTravel.Q;
Io.CoolingOpen := CoolingTravel.Q;
Io.AgitatorRunning := AgitatorStart.Q AND NOT ContactorDrop;
Io.PumpRunning := PumpStart.Q;
Io.EmergencyStopOk := TRUE;

(* The vat's level and its product's temperature. *)
IF Io.InletOpen THEN
    LevelPercent := LevelPercent + 2.0 * Seconds;
    IF LevelPercent > 0.0 THEN
        Temp := Temp + (8.0 - Temp) * 2.0 * Seconds / LevelPercent;
    END_IF;
END_IF;
IF Io.OutletOpen AND Io.PumpRunning THEN
    LevelPercent := MAX(LevelPercent - 2.5 * Seconds, 0.0);
END_IF;
IF NOT SteamFailure AND LevelPercent > 5.0 THEN
    HeatRate := 0.25 * 80.0 / LevelPercent;
    IF NOT Io.AgitatorRunning THEN
        HeatRate := HeatRate * 0.5;
    END_IF;
    Temp := Temp + INT_TO_REAL(Io.HeaterPower) / 27648.0 * HeatRate * Seconds;
END_IF;
IF Io.CoolingOpen THEN
    Temp := Temp + (4.0 - Temp) * 0.01 * Seconds;
END_IF;
Temp := Temp + (20.0 - Temp) * 0.0005 * Seconds;
IF SteamFailure THEN
    (* The jacket's condensate cools the product. *)
    Temp := Temp - 0.08 * Seconds;
END_IF;
Io.LevelRaw := REAL_TO_INT(LevelPercent / 100.0 * 27648.0);
Io.TempRaw := REAL_TO_INT(Temp / 100.0 * 27648.0);
Io.LevelHighHigh := LevelPercent > 95.0;

(* The operator. *)
WaitIdle(IN := Status.State = VatState#Idle, PT := T#5s);
WaitAlarm(IN := Status.Unacknowledged > 0, PT := T#5s);
WaitAborted(IN := Status.State = VatState#Aborted AND Status.Unacknowledged = 0, PT := T#5s);
IF NOT Press.Q THEN
    Pressing := 0;
    IF WaitAlarm.Q THEN
        Pressing := 3;
    ELSIF WaitAborted.Q THEN
        Pressing := 2;
    ELSIF WaitIdle.Q THEN
        Pressing := 1;
    END_IF;
END_IF;
Press(IN := Pressing > 0, PT := T#300ms);
Panel.RecipeIndex := 2;
Panel.Start := Press.Q AND Pressing = 1;
Panel.Reset := Press.Q AND Pressing = 2;
Panel.Acknowledge := Press.Q AND Pressing = 3;
END_PROGRAM
