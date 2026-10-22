USING Control;

(* The vat pasteurizer's batch control, run in the control task. It reads the transmitters, runs the
   batch's state machine, the product temperature loop (Control.PID on the jacket heater), the
   valves and motors, raises the alarms and drives the outputs and the panel's values.

   A batch fills the vat to the recipe's level, heats the product with the agitator running, holds
   it at the pasteurizing temperature for the hold time, cools it and drains it. The hold counts
   only while the product stays within HoldBand of the pasteurizing temperature: if it falls below,
   the hold starts again, as the rules for a vat pasteurizer require. Stop pauses a batch and Start
   resumes it, a hold from its beginning. A critical alarm or Abort aborts the batch; Reset, once no
   critical alarm is left, drains a vat that holds product, else returns to Idle. *)
PROGRAM VatControl
VAR_EXTERNAL
    Io : FieldSignals;
    Panel : OperatorCommands;
    Status : VatStatus;
END_VAR
VAR
    State : VatState := VatState#Idle;
    PreviousState : VatState := VatState#Idle;
    Interrupted : VatState := VatState#Idle;
    Recipes : ARRAY[1..2] OF Recipe := [
        (Name := 'Milk LTLT', FillLevel := 80.0, PasteurTemp := 63.0, HoldTime := T#30m,
            CoolTemp := 6.0),
        (Name := 'Bench test', FillLevel := 40.0, PasteurTemp := 63.0, HoldTime := T#60s,
            CoolTemp := 30.0)
    ];
    Active : Recipe;
    Level : AnalogInput;
    Temperature : AnalogInput;
    TempFilter : Control.LowPass;
    FilterParameters : Control.LowPassParameters := (
        TimeConstant := LREAL#2.0,
        Order := FilterOrder#FirstOrder
    );
    Heater : Control.PID;
    HeaterParameters : Control.PIDParameters := (
        ProportionalGain := LREAL#8.0,
        IntegralGain := LREAL#0.08,
        DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0,
        DerivativeSetpointWeight := LREAL#0.0,
        Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0,
        OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-20.0,
        RateMaximum := LREAL#20.0,
        TrackingTime := LREAL#5.0,
        AntiWindup := PIDAntiWindup#Combined
    );
    HeaterInitial : Control.PIDInitialState;
    HeaterInput : Control.PIDInput;
    HeaterOutput : LREAL;
    Inlet : Actuator;
    Outlet : Actuator;
    Cooling : Actuator;
    Agitator : Actuator;
    Pump : Actuator;
    Alarms : AlarmList;
    Conditions : ARRAY[1..12] OF BOOL;
    StartEdge : R_TRIG;
    StopEdge : R_TRIG;
    AbortEdge : R_TRIG;
    ResetEdge : R_TRIG;
    ResetRequested : BOOL;
    StepTimer : TON;
    HoldTimer : TON;
    HeatingTimeout : TON;
    Initialized : BOOL;
    Now : TIME;
    LastTime : TIME;
    LoopElapsed : TIME;
    HoldRestarted : BOOL;
    InBatch : BOOL;
    (* The lamps of the panel, and the panel's register map. *)
    LampRunning : BOOL;
    LampWarning : BOOL;
    LampAlarm : BOOL;
    RegState : INT;
    RegTemperature : DINT;
    RegLevel : DINT;
    RegAlarms : INT;
    RegBatches : UDINT;
END_VAR
VAR RETAIN
    Batches : UDINT;
END_VAR
VAR CONSTANT
    (* The temperature loop's sample period. *)
    LoopPeriod : TIME := T#100ms;
    HoldBand : REAL := 0.5;
    OverTemperature : REAL := 75.0;
    (* Below this level the vat counts as empty. *)
    EmptyLevel : REAL := 2.0;
    (* Below this level the agitator's blades are not covered. *)
    AgitatorLevel : REAL := 20.0;
    MaxHeatingTime : TIME := T#15m;
    LoopAuto : Control.PIDMode := PIDMode#Auto;
    LoopManual : Control.PIDMode := PIDMode#Manual;
END_VAR

(* The transmitters. *)
Level(Raw := Io.LevelRaw, RangeLow := 0.0, RangeHigh := 100.0);
Temperature(Raw := Io.TempRaw, RangeLow := 0.0, RangeHigh := 100.0);
Now := TIME();
IF NOT Initialized THEN
    TempFilter.Configure(Parameters := FilterParameters);
    TempFilter.Initialize(Input := REAL_TO_LREAL(Temperature.Value));
    Heater.Configure(Parameters := HeaterParameters);
    HeaterInitial.Setpoint := REAL_TO_LREAL(Temperature.Value);
    HeaterInitial.ProcessValue := HeaterInitial.Setpoint;
    Heater.Initialize(State := HeaterInitial);
    Active := Recipes[1];
    LastTime := Now;
    Initialized := TRUE;
END_IF;

(* The operator's commands act on their rising edges. *)
StartEdge(CLK := Panel.Start);
StopEdge(CLK := Panel.Stop);
AbortEdge(CLK := Panel.Abort);
ResetEdge(CLK := Panel.Reset);

(* The alarms, in the order of AlarmList's critical table and of the panel's texts. *)
Conditions[1] := NOT Io.EmergencyStopOk;
Conditions[2] := Io.LevelHighHigh;
Conditions[3] := Level.Fault;
Conditions[4] := Temperature.Fault;
Conditions[5] := Temperature.Value > OverTemperature;
Conditions[6] := Agitator.Fault;
Conditions[7] := Inlet.Fault;
Conditions[8] := Outlet.Fault;
Conditions[9] := Cooling.Fault;
Conditions[10] := Pump.Fault;
Conditions[11] := HeatingTimeout.Q;
Conditions[12] := HoldRestarted;
Alarms(Conditions := Conditions, Acknowledge := Panel.Acknowledge);

(* The state machine. *)
CASE State OF
    VatState#Filling, VatState#Heating, VatState#Holding, VatState#Cooling, VatState#Draining:
        InBatch := TRUE;
ELSE
    InBatch := FALSE;
END_CASE;
IF (Alarms.CriticalActive OR AbortEdge.Q) AND InBatch THEN
    State := VatState#Aborted;
ELSIF StopEdge.Q AND InBatch THEN
    Interrupted := State;
    State := VatState#Stopped;
END_IF;

CASE State OF
    VatState#Idle:
        IF StartEdge.Q AND NOT Alarms.CriticalActive THEN
            Active := Recipes[LIMIT(1, Panel.RecipeIndex, 2)];
            State := VatState#Filling;
        END_IF;
    VatState#Filling:
        IF Level.Value >= Active.FillLevel THEN
            State := VatState#Heating;
        END_IF;
    VatState#Heating:
        (* The loop approaches its setpoint from below, so the hold starts within its band. *)
        IF Temperature.Value >= Active.PasteurTemp - HoldBand THEN
            State := VatState#Holding;
        END_IF;
    VatState#Holding:
        IF HoldTimer.Q THEN
            State := VatState#Cooling;
        END_IF;
    VatState#Cooling:
        IF Temperature.Value <= Active.CoolTemp THEN
            State := VatState#Draining;
        END_IF;
    VatState#Draining:
        (* The pump runs on for 5 s after the vat reads empty. *)
        IF Level.Value > EmptyLevel THEN
            StepTimer(IN := FALSE);
        ELSIF StepTimer.ET >= T#5s THEN
            State := VatState#Done;
        END_IF;
    VatState#Done:
        IF StepTimer.ET >= T#3s THEN
            State := VatState#Idle;
        END_IF;
    VatState#Stopped:
        IF StartEdge.Q AND NOT Alarms.CriticalActive THEN
            State := Interrupted;
        END_IF;
    VatState#Aborted:
        IF ResetEdge.Q THEN
            ResetRequested := TRUE;
        END_IF;
        IF ResetRequested AND NOT Alarms.CriticalActive THEN
            ResetRequested := FALSE;
            IF Level.Value > EmptyLevel THEN
                State := VatState#Draining;
            ELSE
                State := VatState#Idle;
            END_IF;
        END_IF;
END_CASE;

IF State <> PreviousState THEN
    StepTimer(IN := FALSE);
    IF State = VatState#Done THEN
        Batches := Batches + 1;
    END_IF;
    PreviousState := State;
END_IF;
StepTimer(IN := TRUE, PT := T#24h);

(* The hold counts only while the product is hot enough; a fall below restarts it. *)
HoldRestarted := State = VatState#Holding
    AND Temperature.Value < Active.PasteurTemp - HoldBand
    AND HoldTimer.ET > T#0s;
HoldTimer(IN := State = VatState#Holding AND Temperature.Value >= Active.PasteurTemp - HoldBand,
    PT := Active.HoldTime);
HeatingTimeout(IN := State = VatState#Heating, PT := MaxHeatingTime);

(* The valves and motors; the emergency stop drops them all. *)
Inlet(Command := State = VatState#Filling,
    Interlock := Io.EmergencyStopOk AND NOT Io.LevelHighHigh,
    Feedback := Io.InletOpen, Reset := ResetEdge.Q);
Agitator(Command := State = VatState#Heating OR State = VatState#Holding
        OR State = VatState#Cooling,
    Interlock := Io.EmergencyStopOk AND Level.Value > AgitatorLevel,
    Feedback := Io.AgitatorRunning, Reset := ResetEdge.Q, SupervisionTime := T#2s);
Cooling(Command := State = VatState#Cooling, Interlock := Io.EmergencyStopOk,
    Feedback := Io.CoolingOpen, Reset := ResetEdge.Q);
Outlet(Command := State = VatState#Draining, Interlock := Io.EmergencyStopOk,
    Feedback := Io.OutletOpen, Reset := ResetEdge.Q);
Pump(Command := State = VatState#Draining AND Io.OutletOpen,
    Interlock := Io.EmergencyStopOk, Feedback := Io.PumpRunning, Reset := ResetEdge.Q,
    SupervisionTime := T#2s);

(* The temperature loop runs at its own sample period, on the time that has actually passed. *)
LoopElapsed := LoopElapsed + (Now - LastTime);
LastTime := Now;
IF LoopElapsed >= LoopPeriod THEN
    HeaterInput.CycleTime := UDINT_TO_LREAL(DWORD_TO_UDINT(TIME_TO_DWORD(LoopElapsed))) / LREAL#1000.0;
    HeaterInput.ProcessValue := TempFilter.Update(
        Input := REAL_TO_LREAL(Temperature.Value),
        CycleTime := HeaterInput.CycleTime
    );
    HeaterInput.Setpoint := REAL_TO_LREAL(Active.PasteurTemp);
    IF State = VatState#Heating OR State = VatState#Holding THEN
        HeaterInput.Mode := LoopAuto;
    ELSE
        HeaterInput.Mode := LoopManual;
    END_IF;
    HeaterInput.ManualOutput := LREAL#0.0;
    HeaterOutput := Heater.Update(Input := HeaterInput);
    LoopElapsed := T#0s;
END_IF;

(* The outputs: heating without agitation would scorch the product. *)
Io.InletValve := Inlet.Output;
Io.OutletValve := Outlet.Output;
Io.CoolingValve := Cooling.Output;
Io.Agitator := Agitator.Output;
Io.Pump := Pump.Output;
IF Agitator.Output AND Io.AgitatorRunning THEN
    Io.HeaterPower := REAL_TO_INT(LREAL_TO_REAL(HeaterOutput) * 276.48);
ELSE
    Io.HeaterPower := 0;
END_IF;
Io.Horn := Alarms.Horn;
LampRunning := InBatch;
LampWarning := Alarms.Active > 0 AND NOT Alarms.CriticalActive;
LampAlarm := Alarms.CriticalActive;

(* The panel's values and register map. *)
Status.State := State;
Status.Recipe := Active.Name;
Status.Level := Level.Value;
Status.Temperature := Temperature.Value;
Status.Setpoint := Active.PasteurTemp;
Status.HeaterPower := LREAL_TO_REAL(HeaterOutput);
IF State = VatState#Holding THEN
    Status.HoldRemaining := Active.HoldTime - HoldTimer.ET;
ELSE
    Status.HoldRemaining := T#0s;
END_IF;
Status.StepTime := StepTimer.ET;
Status.Batches := Batches;
Status.ActiveAlarms := Alarms.Active;
Status.Unacknowledged := Alarms.Unacknowledged;
Status.FirstOut := Alarms.FirstOut;
RegState := StateCode(State := State);
RegTemperature := REAL_TO_DINT(Temperature.Value * 10.0);
RegLevel := REAL_TO_DINT(Level.Value * 10.0);
RegAlarms := Alarms.Active;
RegBatches := Batches;
END_PROGRAM
