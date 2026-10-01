USING Control;

(* The heater commands that hold temperature deviations X1, X2 from 35 °C against loads D1, D2 in kW, from the steady
   balances 2·v1 = 18·x1 − 8·x2 + D1 and 2·v2 = −8·x1 + 18·x2 + D2 around 75 %. *)
FUNCTION HoldingCommand1Pct : LREAL
VAR_INPUT
    X1C : LREAL;
    X2C : LREAL;
    D1Kw : LREAL;
END_VAR
HoldingCommand1Pct := LREAL#75.0 + (LREAL#18.0 * X1C - LREAL#8.0 * X2C + D1Kw) / LREAL#2.0;
END_FUNCTION

FUNCTION HoldingCommand2Pct : LREAL
VAR_INPUT
    X1C : LREAL;
    X2C : LREAL;
    D2Kw : LREAL;
END_VAR
HoldingCommand2Pct := LREAL#75.0 + (LREAL#-8.0 * X1C + LREAL#18.0 * X2C + D2Kw) / LREAL#2.0;
END_FUNCTION

(* The gain of path 1 → 1 while the other loop holds output 2, for the steady gains K = [[A, B], [C, D]]: A − B·C/D, which
   is K11/λ11. 0.0 when D = 0: input 2 cannot hold output 2. *)
FUNCTION OtherLoopClosedGain : LREAL
VAR_INPUT
    A : LREAL;
    B : LREAL;
    C : LREAL;
    D : LREAL;
END_VAR
IF ABS(D) > LREAL#1.0E-12 THEN
    OtherLoopClosedGain := A - B * C / D;
ELSE
    OtherLoopClosedGain := LREAL#0.0;
END_IF;
END_FUNCTION

(* The coupled vessels of Lesson 69 with one Control.PID per vessel, each with its own parameters, memory and limits,
   and one Auto/Manual switch for both, so Control Studio commissions them together as a PID group. Until a design is
   applied both are in Manual at 75 %. Load1Kw and Load2Kw take heat from the vessels; validation scenarios set them. *)
PROGRAM Main
VAR
    Vessel1 : Control.PID;
    Vessel2 : Control.PID;
    Vessel1Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0, IntegralGain := LREAL#1.0, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    Vessel2Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#5.0, IntegralGain := LREAL#1.0, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    Vessel1Input : Control.PIDInput;
    Vessel2Input : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
    (* TRUE selects Auto for both controllers. *)
    VesselsAuto : BOOL := FALSE;
    Vessel1ManualPct : LREAL := 75.0;
    Vessel2ManualPct : LREAL := 75.0;
    Vessel1SetpointC : LREAL := 35.0;
    Vessel2SetpointC : LREAL := 35.0;
    Vessel1C : LREAL := 35.0;
    Vessel2C : LREAL := 35.0;
    Heater1Pct : LREAL := 75.0;
    Heater2Pct : LREAL := 75.0;
    Load1Kw : LREAL := 0.0;
    Load2Kw : LREAL := 0.0;
END_VAR

Vessel1.Configure(Parameters := Vessel1Parameters);
Vessel2.Configure(Parameters := Vessel2Parameters);
IF NOT Initialized THEN
    (* Memory-mapped variables start from the %M image; set the resting values explicitly. *)
    Vessel1SetpointC := LREAL#35.0;
    Vessel2SetpointC := LREAL#35.0;
    Vessel1ManualPct := LREAL#75.0;
    Vessel2ManualPct := LREAL#75.0;
    State.Setpoint := Vessel1SetpointC;
    State.ProcessValue := Vessel1C;
    State.NominalOutput := Vessel1ManualPct;
    State.SaturatedOutput := Vessel1ManualPct;
    Vessel1.Initialize(State := State);
    State.Setpoint := Vessel2SetpointC;
    State.ProcessValue := Vessel2C;
    State.NominalOutput := Vessel2ManualPct;
    State.SaturatedOutput := Vessel2ManualPct;
    Vessel2.Initialize(State := State);
    Initialized := TRUE;
END_IF;

Vessel1Input.Setpoint := Vessel1SetpointC;
Vessel1Input.ProcessValue := Vessel1C;
Vessel1Input.ManualOutput := Vessel1ManualPct;
Vessel1Input.TrackingOutput := Vessel1ManualPct;
Vessel1Input.CycleTime := LREAL#0.1;
IF VesselsAuto THEN
    Vessel1Input.Mode := PIDMode#Auto;
ELSE
    Vessel1Input.Mode := PIDMode#Manual;
END_IF;
Heater1Pct := Vessel1.Update(Input := Vessel1Input);

Vessel2Input.Setpoint := Vessel2SetpointC;
Vessel2Input.ProcessValue := Vessel2C;
Vessel2Input.ManualOutput := Vessel2ManualPct;
Vessel2Input.TrackingOutput := Vessel2ManualPct;
Vessel2Input.CycleTime := LREAL#0.1;
IF VesselsAuto THEN
    Vessel2Input.Mode := PIDMode#Auto;
ELSE
    Vessel2Input.Mode := PIDMode#Manual;
END_IF;
Heater2Pct := Vessel2.Update(Input := Vessel2Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Heater1Pct AT %QL0 : LREAL;
    P1.Heater2Pct AT %QL8 : LREAL;
    P1.Load1Kw AT %QL16 : LREAL;
    P1.Load2Kw AT %QL24 : LREAL;
    P1.Vessel1C AT %IL0 : LREAL;
    P1.Vessel2C AT %IL8 : LREAL;
    P1.Vessel1SetpointC AT %ML0 : LREAL;
    P1.Vessel2SetpointC AT %ML8 : LREAL;
    P1.Vessel1ManualPct AT %ML16 : LREAL;
    P1.Vessel2ManualPct AT %ML24 : LREAL;
END_VAR
END_CONFIGURATION
