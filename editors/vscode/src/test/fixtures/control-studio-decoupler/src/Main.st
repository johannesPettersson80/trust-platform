USING Control;

(* Two heated zones that share a wall, one Control.PID per zone and one Auto/Manual switch for both. Heating one zone
   also warms the other, so in Auto the program passes the controllers' requests v through a static decoupler,
   u = D·v, that Control Studio designs; in Manual each request goes straight to its heater. *)
PROGRAM Main
VAR
    Zone1 : Control.PID;
    Zone2 : Control.PID;
    Zone1Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#4.0, IntegralGain := LREAL#0.5, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    Zone2Parameters : Control.PIDParameters := (
        ProportionalGain := LREAL#4.0, IntegralGain := LREAL#0.5, DerivativeGain := LREAL#0.0,
        ProportionalSetpointWeight := LREAL#1.0, DerivativeSetpointWeight := LREAL#0.0, Bias := LREAL#0.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0,
        RateMinimum := LREAL#-1000.0, RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0,
        AntiWindup := PIDAntiWindup#BackCalculation);
    Zone1Input : Control.PIDInput;
    Zone2Input : Control.PIDInput;
    State : Control.PIDInitialState;
    Initialized : BOOL := FALSE;
    (* TRUE selects Auto for both controllers; they start in Manual at the resting point. *)
    ZonesAuto : BOOL := FALSE;
    Zone1ManualPct : LREAL := 20.0;
    Zone2ManualPct : LREAL := 20.0;
    Zone1SetpointC : LREAL := 26.0;
    Zone2SetpointC : LREAL := 26.0;
    Zone1C : LREAL := 26.0;
    Zone2C : LREAL := 26.0;
    Heater1Pct : LREAL := 20.0;
    Heater2Pct : LREAL := 20.0;
    Request1Pct : LREAL := 20.0;
    Request2Pct : LREAL := 20.0;
    D11 : LREAL := 1.0;
    D12 : LREAL := 0.0;
    D21 : LREAL := 0.0;
    D22 : LREAL := 1.0;
END_VAR

Zone1.Configure(Parameters := Zone1Parameters);
Zone2.Configure(Parameters := Zone2Parameters);
IF NOT Initialized THEN
    (* Memory-mapped variables start from the %M image; set the resting values explicitly. *)
    Zone1SetpointC := LREAL#26.0;
    Zone2SetpointC := LREAL#26.0;
    Zone1ManualPct := LREAL#20.0;
    Zone2ManualPct := LREAL#20.0;
    State.Setpoint := Zone1SetpointC;
    State.ProcessValue := Zone1C;
    State.NominalOutput := Zone1ManualPct;
    State.SaturatedOutput := Zone1ManualPct;
    Zone1.Initialize(State := State);
    State.Setpoint := Zone2SetpointC;
    State.ProcessValue := Zone2C;
    State.NominalOutput := Zone2ManualPct;
    State.SaturatedOutput := Zone2ManualPct;
    Zone2.Initialize(State := State);
    Initialized := TRUE;
END_IF;

Zone1Input.Setpoint := Zone1SetpointC;
Zone1Input.ProcessValue := Zone1C;
Zone1Input.ManualOutput := Zone1ManualPct;
Zone1Input.TrackingOutput := Zone1ManualPct;
Zone1Input.CycleTime := LREAL#0.1;
IF ZonesAuto THEN
    Zone1Input.Mode := PIDMode#Auto;
ELSE
    Zone1Input.Mode := PIDMode#Manual;
END_IF;
Request1Pct := Zone1.Update(Input := Zone1Input);

Zone2Input.Setpoint := Zone2SetpointC;
Zone2Input.ProcessValue := Zone2C;
Zone2Input.ManualOutput := Zone2ManualPct;
Zone2Input.TrackingOutput := Zone2ManualPct;
Zone2Input.CycleTime := LREAL#0.1;
IF ZonesAuto THEN
    Zone2Input.Mode := PIDMode#Auto;
ELSE
    Zone2Input.Mode := PIDMode#Manual;
END_IF;
Request2Pct := Zone2.Update(Input := Zone2Input);
IF ZonesAuto THEN
    Heater1Pct := LIMIT(LREAL#0.0, D11 * Request1Pct + D12 * Request2Pct, LREAL#100.0);
    Heater2Pct := LIMIT(LREAL#0.0, D21 * Request1Pct + D22 * Request2Pct, LREAL#100.0);
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
    P1.Zone1C AT %IL0 : LREAL;
    P1.Zone2C AT %IL8 : LREAL;
    P1.Zone1SetpointC AT %ML0 : LREAL;
    P1.Zone2SetpointC AT %ML8 : LREAL;
    P1.Zone1ManualPct AT %ML16 : LREAL;
    P1.Zone2ManualPct AT %ML24 : LREAL;
END_VAR
END_CONFIGURATION
