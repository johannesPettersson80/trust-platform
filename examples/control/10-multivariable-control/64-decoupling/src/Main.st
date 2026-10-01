USING Control;

(* The plant input heater i receives behind a static decoupler: u_i = D_ii·v_i + D_ij·v_j from the two controller
   requests, limited to the heater's 0–100 %. *)
FUNCTION DecoupledHeaterPct : LREAL
VAR_INPUT
    OwnEntry : LREAL;
    CrossEntry : LREAL;
    OwnRequestPct : LREAL;
    OtherRequestPct : LREAL;
END_VAR
DecoupledHeaterPct := LIMIT(LREAL#0.0, OwnEntry * OwnRequestPct + CrossEntry * OtherRequestPct, LREAL#100.0);
END_FUNCTION

(* Where a decoupled controller starts so that the heaters keep their present commands: v = D⁻¹·u, its first row.
   0.0 when D has no inverse. *)
FUNCTION StartingRequest1Pct : LREAL
VAR_INPUT
    D11 : LREAL;
    D12 : LREAL;
    D21 : LREAL;
    D22 : LREAL;
    Heater1Pct : LREAL;
    Heater2Pct : LREAL;
END_VAR
VAR
    Determinant : LREAL;
END_VAR
Determinant := D11 * D22 - D12 * D21;
IF ABS(Determinant) > LREAL#1.0E-12 THEN
    StartingRequest1Pct := (D22 * Heater1Pct - D12 * Heater2Pct) / Determinant;
ELSE
    StartingRequest1Pct := LREAL#0.0;
END_IF;
END_FUNCTION

(* The coupled vessels of Lesson 69, one Control.PID per vessel and one Auto/Manual switch for both. In Auto the program
   passes the controllers' requests v through the static decoupler u = D·v that Control Studio designs; in Manual each
   request goes straight to its heater. The vessels rest at 30 °C, where the heaters hold at 50 %: Control Studio starts
   the decoupled controllers at v = D⁻¹·u, which must lie inside their 0–100 %. Until a design is applied both are in Manual,
   with Lesson 71's gains and D the identity. *)
PROGRAM Main
VAR
    Vessel1 : Control.PID;
    Vessel2 : Control.PID;
    Vessel1Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#6.75, IntegralGain := LREAL#1.73, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    Vessel2Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#6.75, IntegralGain := LREAL#1.73, DerivativeGain := LREAL#0.0,
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
    Vessel1ManualPct : LREAL := 50.0;
    Vessel2ManualPct : LREAL := 50.0;
    Vessel1SetpointC : LREAL := 30.0;
    Vessel2SetpointC : LREAL := 30.0;
    Vessel1C : LREAL := 30.0;
    Vessel2C : LREAL := 30.0;
    Heater1Pct : LREAL := 50.0;
    Heater2Pct : LREAL := 50.0;
    Request1Pct : LREAL := 50.0;
    Request2Pct : LREAL := 50.0;
    D11 : LREAL := 1.0;
    D12 : LREAL := 0.0;
    D21 : LREAL := 0.0;
    D22 : LREAL := 1.0;
END_VAR

Vessel1.Configure(Parameters := Vessel1Parameters);
Vessel2.Configure(Parameters := Vessel2Parameters);
IF NOT Initialized THEN
    (* Memory-mapped variables start from the %M image; set the resting values explicitly. *)
    Vessel1SetpointC := LREAL#30.0;
    Vessel2SetpointC := LREAL#30.0;
    Vessel1ManualPct := LREAL#50.0;
    Vessel2ManualPct := LREAL#50.0;
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
Request1Pct := Vessel1.Update(Input := Vessel1Input);

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
Request2Pct := Vessel2.Update(Input := Vessel2Input);

IF VesselsAuto THEN
    Heater1Pct := DecoupledHeaterPct(OwnEntry := D11, CrossEntry := D12, OwnRequestPct := Request1Pct, OtherRequestPct := Request2Pct);
    Heater2Pct := DecoupledHeaterPct(OwnEntry := D22, CrossEntry := D21, OwnRequestPct := Request2Pct, OtherRequestPct := Request1Pct);
ELSE
    Heater1Pct := Request1Pct;
    Heater2Pct := Request2Pct;
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
    P1.Vessel1SetpointC AT %ML0 : LREAL;
    P1.Vessel2SetpointC AT %ML8 : LREAL;
    P1.Vessel1ManualPct AT %ML16 : LREAL;
    P1.Vessel2ManualPct AT %ML24 : LREAL;
END_VAR
END_CONFIGURATION
