USING Control;

(* The heater commands that hold deviations X1, X2 from 35 °C against loads D1, D2 in kW, for wall coupling Hc in kW/°C:
   2·v1 = (10 + Hc)·x1 − Hc·x2 + D1 and 2·v2 = −Hc·x1 + (10 + Hc)·x2 + D2 around 75 %. *)
FUNCTION HoldingCommand1Pct : LREAL
VAR_INPUT
    CouplingKwPerC : LREAL;
    X1C : LREAL;
    X2C : LREAL;
    D1Kw : LREAL;
END_VAR
HoldingCommand1Pct := LREAL#75.0 + ((LREAL#10.0 + CouplingKwPerC) * X1C - CouplingKwPerC * X2C + D1Kw) / LREAL#2.0;
END_FUNCTION

FUNCTION HoldingCommand2Pct : LREAL
VAR_INPUT
    CouplingKwPerC : LREAL;
    X1C : LREAL;
    X2C : LREAL;
    D2Kw : LREAL;
END_VAR
HoldingCommand2Pct := LREAL#75.0 + (-CouplingKwPerC * X1C + (LREAL#10.0 + CouplingKwPerC) * X2C + D2Kw) / LREAL#2.0;
END_FUNCTION

(* The steady cross gain left by a map D = Scale·[[1, −C], [−C, 1]] at wall coupling Hc: the off-diagonal of K·D, in °C per
   point, with K = 2·[[10 + Hc, −Hc], [−Hc, 10 + Hc]]⁻¹. Zero means the map cancels the steady cross effect. *)
FUNCTION MappedCrossGainCPerPct : LREAL
VAR_INPUT
    CouplingKwPerC : LREAL;
    CrossGain : LREAL;
    Scale : LREAL := 1.0;
END_VAR
MappedCrossGainCPerPct := Scale * (LREAL#2.0 * CouplingKwPerC - LREAL#2.0 * CrossGain * (LREAL#10.0 + CouplingKwPerC))
    / (LREAL#100.0 + LREAL#20.0 * CouplingKwPerC);
END_FUNCTION

(* The map u = 75 + D·v with D = Scale·[[1, −C], [−C, 1]]: C = 0 and Scale 1 leave each request on its own heater; C = 4/9
   and Scale 81/65 are Control Studio's decoupler of Lesson 72, K⁻¹·diag(K). The 75 % holding command is added after the
   map. *)
FUNCTION PhysicalRequest1Pct : LREAL
VAR_INPUT
    V1Pct : LREAL;
    V2Pct : LREAL;
    CrossGain : LREAL;
    Scale : LREAL := 1.0;
END_VAR
PhysicalRequest1Pct := LREAL#75.0 + Scale * (V1Pct - CrossGain * V2Pct);
END_FUNCTION

FUNCTION PhysicalRequest2Pct : LREAL
VAR_INPUT
    V1Pct : LREAL;
    V2Pct : LREAL;
    CrossGain : LREAL;
    Scale : LREAL := 1.0;
END_VAR
PhysicalRequest2Pct := LREAL#75.0 + Scale * (V2Pct - CrossGain * V1Pct);
END_FUNCTION

(* A physical difference E = applied − requested, converted back to virtual coordinates with D⁻¹ = [[1, C], [C, 1]]/(Scale·(1 − C²)). *)
FUNCTION VirtualCorrection1Pct : LREAL
VAR_INPUT
    E1Pct : LREAL;
    E2Pct : LREAL;
    CrossGain : LREAL;
    Scale : LREAL := 1.0;
END_VAR
VirtualCorrection1Pct := (E1Pct + CrossGain * E2Pct) / (Scale * (LREAL#1.0 - CrossGain * CrossGain));
END_FUNCTION

FUNCTION VirtualCorrection2Pct : LREAL
VAR_INPUT
    E1Pct : LREAL;
    E2Pct : LREAL;
    CrossGain : LREAL;
    Scale : LREAL := 1.0;
END_VAR
VirtualCorrection2Pct := (CrossGain * E1Pct + E2Pct) / (Scale * (LREAL#1.0 - CrossGain * CrossGain));
END_FUNCTION

(* Two PI loops through the map D: each virtual request is P + I in deviation points, with its integral starting at zero;
   the heaters are limited to 0–100 %, and the limiting error goes back to the integrals through D⁻¹ with the tracking
   time. CrossGain 0 with Scale 1 gives Lesson 71's decentralized PI, one PI per vessel with back-calculation; CrossGain
   4/9 with Scale 81/65 gives Lesson 72's decoupled group, both as Control Studio designed them. *)
FUNCTION_BLOCK TwoVesselPi
VAR_INPUT
    Setpoint1C : LREAL;
    Setpoint2C : LREAL;
    Vessel1C : LREAL;
    Vessel2C : LREAL;
    CrossGain : LREAL := 0.0;
    DecouplerScale : LREAL := 1.0;
    ProportionalGain : LREAL := 5.0;
    IntegralGain : LREAL := 1.0;
    TrackingTimeS : LREAL := 1.0;
    UpdateS : LREAL := 0.1;
END_VAR
VAR_OUTPUT
    Heater1Pct : LREAL := 75.0;
    Heater2Pct : LREAL := 75.0;
    Virtual1Pct : LREAL := 0.0;
    Virtual2Pct : LREAL := 0.0;
    Request1Pct : LREAL := 75.0;
    Request2Pct : LREAL := 75.0;
END_VAR
VAR
    Integral1Pct : LREAL := 0.0;
    Integral2Pct : LREAL := 0.0;
    Error1C : LREAL;
    Error2C : LREAL;
    Trial1Pct : LREAL;
    Trial2Pct : LREAL;
END_VAR
Error1C := Setpoint1C - Vessel1C;
Error2C := Setpoint2C - Vessel2C;
Trial1Pct := Integral1Pct + IntegralGain * Error1C * UpdateS;
Trial2Pct := Integral2Pct + IntegralGain * Error2C * UpdateS;
Virtual1Pct := ProportionalGain * Error1C + Trial1Pct;
Virtual2Pct := ProportionalGain * Error2C + Trial2Pct;
Request1Pct := PhysicalRequest1Pct(V1Pct := Virtual1Pct, V2Pct := Virtual2Pct, CrossGain := CrossGain, Scale := DecouplerScale);
Request2Pct := PhysicalRequest2Pct(V1Pct := Virtual1Pct, V2Pct := Virtual2Pct, CrossGain := CrossGain, Scale := DecouplerScale);
Heater1Pct := LIMIT(LREAL#0.0, Request1Pct, LREAL#100.0);
Heater2Pct := LIMIT(LREAL#0.0, Request2Pct, LREAL#100.0);
Integral1Pct := Trial1Pct + UpdateS / TrackingTimeS
    * VirtualCorrection1Pct(E1Pct := Heater1Pct - Request1Pct, E2Pct := Heater2Pct - Request2Pct, CrossGain := CrossGain, Scale := DecouplerScale);
Integral2Pct := Trial2Pct + UpdateS / TrackingTimeS
    * VirtualCorrection2Pct(E1Pct := Heater1Pct - Request1Pct, E2Pct := Heater2Pct - Request2Pct, CrossGain := CrossGain, Scale := DecouplerScale);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    (* 24 copies of the coupled vessels, at wall coupling 6, 8 and 10 kW/°C: decentralized PI (Pi) and the decoupled group
       (Dc), both frozen as Control Studio designed them for 8 kW/°C in Lessons 71 and 72, on a vessel-1 target (T1), a
       vessel-2 target (T2) and a 10 kW load on vessel 1 (L1) or vessel 2 (L2). *)
    (* Control Studio's declared experiment setting: the wall coupling, in kW/°C, of the vessel set this run exercises. The
       other sets rest at 35 °C and 75 %. *)
    CouplingCase : LREAL := 8.0;
    ActiveCoupling : LREAL := 8.0;
    PairedCrossGain : LREAL := 0.0;
    PairedKp : LREAL := 6.75;
    PairedKi : LREAL := 1.73;
    DecouplerCrossGain : LREAL := 0.444444444444444444;
    DecouplerScale : LREAL := 1.246153846153846;
    DecoupledKp : LREAL := 5.42;
    DecoupledKi : LREAL := 1.74;
    Pi6T1Loops : TwoVesselPi;
    Pi6T1Heater1Pct : LREAL := 75.0;
    Pi6T1Heater2Pct : LREAL := 75.0;
    Pi6T1Vessel1C : LREAL := 35.0;
    Pi6T1Vessel2C : LREAL := 35.0;
    Pi6T2Loops : TwoVesselPi;
    Pi6T2Heater1Pct : LREAL := 75.0;
    Pi6T2Heater2Pct : LREAL := 75.0;
    Pi6T2Vessel1C : LREAL := 35.0;
    Pi6T2Vessel2C : LREAL := 35.0;
    Pi6L1Loops : TwoVesselPi;
    Pi6L1Heater1Pct : LREAL := 75.0;
    Pi6L1Heater2Pct : LREAL := 75.0;
    Pi6L1Vessel1C : LREAL := 35.0;
    Pi6L1Vessel2C : LREAL := 35.0;
    Pi6L2Loops : TwoVesselPi;
    Pi6L2Heater1Pct : LREAL := 75.0;
    Pi6L2Heater2Pct : LREAL := 75.0;
    Pi6L2Vessel1C : LREAL := 35.0;
    Pi6L2Vessel2C : LREAL := 35.0;
    Pi8T1Loops : TwoVesselPi;
    Pi8T1Heater1Pct : LREAL := 75.0;
    Pi8T1Heater2Pct : LREAL := 75.0;
    Pi8T1Vessel1C : LREAL := 35.0;
    Pi8T1Vessel2C : LREAL := 35.0;
    Pi8T2Loops : TwoVesselPi;
    Pi8T2Heater1Pct : LREAL := 75.0;
    Pi8T2Heater2Pct : LREAL := 75.0;
    Pi8T2Vessel1C : LREAL := 35.0;
    Pi8T2Vessel2C : LREAL := 35.0;
    Pi8L1Loops : TwoVesselPi;
    Pi8L1Heater1Pct : LREAL := 75.0;
    Pi8L1Heater2Pct : LREAL := 75.0;
    Pi8L1Vessel1C : LREAL := 35.0;
    Pi8L1Vessel2C : LREAL := 35.0;
    Pi8L2Loops : TwoVesselPi;
    Pi8L2Heater1Pct : LREAL := 75.0;
    Pi8L2Heater2Pct : LREAL := 75.0;
    Pi8L2Vessel1C : LREAL := 35.0;
    Pi8L2Vessel2C : LREAL := 35.0;
    Pi10T1Loops : TwoVesselPi;
    Pi10T1Heater1Pct : LREAL := 75.0;
    Pi10T1Heater2Pct : LREAL := 75.0;
    Pi10T1Vessel1C : LREAL := 35.0;
    Pi10T1Vessel2C : LREAL := 35.0;
    Pi10T2Loops : TwoVesselPi;
    Pi10T2Heater1Pct : LREAL := 75.0;
    Pi10T2Heater2Pct : LREAL := 75.0;
    Pi10T2Vessel1C : LREAL := 35.0;
    Pi10T2Vessel2C : LREAL := 35.0;
    Pi10L1Loops : TwoVesselPi;
    Pi10L1Heater1Pct : LREAL := 75.0;
    Pi10L1Heater2Pct : LREAL := 75.0;
    Pi10L1Vessel1C : LREAL := 35.0;
    Pi10L1Vessel2C : LREAL := 35.0;
    Pi10L2Loops : TwoVesselPi;
    Pi10L2Heater1Pct : LREAL := 75.0;
    Pi10L2Heater2Pct : LREAL := 75.0;
    Pi10L2Vessel1C : LREAL := 35.0;
    Pi10L2Vessel2C : LREAL := 35.0;
    Dc6T1Loops : TwoVesselPi;
    Dc6T1Heater1Pct : LREAL := 75.0;
    Dc6T1Heater2Pct : LREAL := 75.0;
    Dc6T1Vessel1C : LREAL := 35.0;
    Dc6T1Vessel2C : LREAL := 35.0;
    Dc6T2Loops : TwoVesselPi;
    Dc6T2Heater1Pct : LREAL := 75.0;
    Dc6T2Heater2Pct : LREAL := 75.0;
    Dc6T2Vessel1C : LREAL := 35.0;
    Dc6T2Vessel2C : LREAL := 35.0;
    Dc6L1Loops : TwoVesselPi;
    Dc6L1Heater1Pct : LREAL := 75.0;
    Dc6L1Heater2Pct : LREAL := 75.0;
    Dc6L1Vessel1C : LREAL := 35.0;
    Dc6L1Vessel2C : LREAL := 35.0;
    Dc6L2Loops : TwoVesselPi;
    Dc6L2Heater1Pct : LREAL := 75.0;
    Dc6L2Heater2Pct : LREAL := 75.0;
    Dc6L2Vessel1C : LREAL := 35.0;
    Dc6L2Vessel2C : LREAL := 35.0;
    Dc8T1Loops : TwoVesselPi;
    Dc8T1Heater1Pct : LREAL := 75.0;
    Dc8T1Heater2Pct : LREAL := 75.0;
    Dc8T1Vessel1C : LREAL := 35.0;
    Dc8T1Vessel2C : LREAL := 35.0;
    Dc8T2Loops : TwoVesselPi;
    Dc8T2Heater1Pct : LREAL := 75.0;
    Dc8T2Heater2Pct : LREAL := 75.0;
    Dc8T2Vessel1C : LREAL := 35.0;
    Dc8T2Vessel2C : LREAL := 35.0;
    Dc8L1Loops : TwoVesselPi;
    Dc8L1Heater1Pct : LREAL := 75.0;
    Dc8L1Heater2Pct : LREAL := 75.0;
    Dc8L1Vessel1C : LREAL := 35.0;
    Dc8L1Vessel2C : LREAL := 35.0;
    Dc8L2Loops : TwoVesselPi;
    Dc8L2Heater1Pct : LREAL := 75.0;
    Dc8L2Heater2Pct : LREAL := 75.0;
    Dc8L2Vessel1C : LREAL := 35.0;
    Dc8L2Vessel2C : LREAL := 35.0;
    Dc10T1Loops : TwoVesselPi;
    Dc10T1Heater1Pct : LREAL := 75.0;
    Dc10T1Heater2Pct : LREAL := 75.0;
    Dc10T1Vessel1C : LREAL := 35.0;
    Dc10T1Vessel2C : LREAL := 35.0;
    Dc10T2Loops : TwoVesselPi;
    Dc10T2Heater1Pct : LREAL := 75.0;
    Dc10T2Heater2Pct : LREAL := 75.0;
    Dc10T2Vessel1C : LREAL := 35.0;
    Dc10T2Vessel2C : LREAL := 35.0;
    Dc10L1Loops : TwoVesselPi;
    Dc10L1Heater1Pct : LREAL := 75.0;
    Dc10L1Heater2Pct : LREAL := 75.0;
    Dc10L1Vessel1C : LREAL := 35.0;
    Dc10L1Vessel2C : LREAL := 35.0;
    Dc10L2Loops : TwoVesselPi;
    Dc10L2Heater1Pct : LREAL := 75.0;
    Dc10L2Heater2Pct : LREAL := 75.0;
    Dc10L2Vessel1C : LREAL := 35.0;
    Dc10L2Vessel2C : LREAL := 35.0;
    (* Control Studio's project experiment raises the targets to 37 °C and starts the 10 kW loads. *)
    TargetC : LREAL := 35.0;
    LoadKw : LREAL := 0.0;
    SelectedLoadKw : LREAL := 0.0;
    NoLoadKw : LREAL := 0.0;
    (* The active set, for Control Studio. *)
    PiT1Vessel1C : LREAL := 35.0;
    PiT1Vessel2C : LREAL := 35.0;
    PiT2Vessel1C : LREAL := 35.0;
    PiT2Vessel2C : LREAL := 35.0;
    PiL1Vessel1C : LREAL := 35.0;
    PiL1Vessel2C : LREAL := 35.0;
    PiL2Vessel1C : LREAL := 35.0;
    PiL2Vessel2C : LREAL := 35.0;
    DcT1Vessel1C : LREAL := 35.0;
    DcT1Vessel2C : LREAL := 35.0;
    DcT2Vessel1C : LREAL := 35.0;
    DcT2Vessel2C : LREAL := 35.0;
    DcL1Vessel1C : LREAL := 35.0;
    DcL1Vessel2C : LREAL := 35.0;
    DcL2Vessel1C : LREAL := 35.0;
    DcL2Vessel2C : LREAL := 35.0;
    PiT1Heater1Pct : LREAL := 75.0;
    PiT1Heater2Pct : LREAL := 75.0;
    DcT1Heater1Pct : LREAL := 75.0;
    DcT1Heater2Pct : LREAL := 75.0;
    DcL1Heater1Pct : LREAL := 75.0;
    DcL1Heater2Pct : LREAL := 75.0;
    ExperimentEnabled : BOOL := FALSE;
END_VAR
IF ExperimentEnabled THEN
    TargetC := LREAL#37.0;
    LoadKw := LREAL#10.0;
ELSE
    TargetC := LREAL#35.0;
    LoadKw := LREAL#0.0;
END_IF;
(* The load cases' plants read the load from one address; every other load input reads the zero address. *)
SelectedLoadKw := LoadKw;
NoLoadKw := LREAL#0.0;
(* The rig's walls are 6, 8 and 10 kW/°C; the setting selects the nearest set. *)
IF CouplingCase < LREAL#7.0 THEN
    ActiveCoupling := LREAL#6.0;
ELSIF CouplingCase < LREAL#9.0 THEN
    ActiveCoupling := LREAL#8.0;
ELSE
    ActiveCoupling := LREAL#10.0;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Pi6T1Loops(Setpoint1C := TargetC, Setpoint2C := LREAL#35.0, Vessel1C := Pi6T1Vessel1C, Vessel2C := Pi6T1Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi6T1Heater1Pct := Pi6T1Loops.Heater1Pct;
    Pi6T1Heater2Pct := Pi6T1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Pi6T2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := TargetC, Vessel1C := Pi6T2Vessel1C, Vessel2C := Pi6T2Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi6T2Heater1Pct := Pi6T2Loops.Heater1Pct;
    Pi6T2Heater2Pct := Pi6T2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Pi6L1Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Pi6L1Vessel1C, Vessel2C := Pi6L1Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi6L1Heater1Pct := Pi6L1Loops.Heater1Pct;
    Pi6L1Heater2Pct := Pi6L1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Pi6L2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Pi6L2Vessel1C, Vessel2C := Pi6L2Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi6L2Heater1Pct := Pi6L2Loops.Heater1Pct;
    Pi6L2Heater2Pct := Pi6L2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Pi8T1Loops(Setpoint1C := TargetC, Setpoint2C := LREAL#35.0, Vessel1C := Pi8T1Vessel1C, Vessel2C := Pi8T1Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi8T1Heater1Pct := Pi8T1Loops.Heater1Pct;
    Pi8T1Heater2Pct := Pi8T1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Pi8T2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := TargetC, Vessel1C := Pi8T2Vessel1C, Vessel2C := Pi8T2Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi8T2Heater1Pct := Pi8T2Loops.Heater1Pct;
    Pi8T2Heater2Pct := Pi8T2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Pi8L1Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Pi8L1Vessel1C, Vessel2C := Pi8L1Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi8L1Heater1Pct := Pi8L1Loops.Heater1Pct;
    Pi8L1Heater2Pct := Pi8L1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Pi8L2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Pi8L2Vessel1C, Vessel2C := Pi8L2Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi8L2Heater1Pct := Pi8L2Loops.Heater1Pct;
    Pi8L2Heater2Pct := Pi8L2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Pi10T1Loops(Setpoint1C := TargetC, Setpoint2C := LREAL#35.0, Vessel1C := Pi10T1Vessel1C, Vessel2C := Pi10T1Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi10T1Heater1Pct := Pi10T1Loops.Heater1Pct;
    Pi10T1Heater2Pct := Pi10T1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Pi10T2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := TargetC, Vessel1C := Pi10T2Vessel1C, Vessel2C := Pi10T2Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi10T2Heater1Pct := Pi10T2Loops.Heater1Pct;
    Pi10T2Heater2Pct := Pi10T2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Pi10L1Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Pi10L1Vessel1C, Vessel2C := Pi10L1Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi10L1Heater1Pct := Pi10L1Loops.Heater1Pct;
    Pi10L1Heater2Pct := Pi10L1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Pi10L2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Pi10L2Vessel1C, Vessel2C := Pi10L2Vessel2C, CrossGain := PairedCrossGain,
        ProportionalGain := PairedKp, IntegralGain := PairedKi);
    Pi10L2Heater1Pct := Pi10L2Loops.Heater1Pct;
    Pi10L2Heater2Pct := Pi10L2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Dc6T1Loops(Setpoint1C := TargetC, Setpoint2C := LREAL#35.0, Vessel1C := Dc6T1Vessel1C, Vessel2C := Dc6T1Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc6T1Heater1Pct := Dc6T1Loops.Heater1Pct;
    Dc6T1Heater2Pct := Dc6T1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Dc6T2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := TargetC, Vessel1C := Dc6T2Vessel1C, Vessel2C := Dc6T2Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc6T2Heater1Pct := Dc6T2Loops.Heater1Pct;
    Dc6T2Heater2Pct := Dc6T2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Dc6L1Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Dc6L1Vessel1C, Vessel2C := Dc6L1Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc6L1Heater1Pct := Dc6L1Loops.Heater1Pct;
    Dc6L1Heater2Pct := Dc6L1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#6.0 THEN
    Dc6L2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Dc6L2Vessel1C, Vessel2C := Dc6L2Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc6L2Heater1Pct := Dc6L2Loops.Heater1Pct;
    Dc6L2Heater2Pct := Dc6L2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Dc8T1Loops(Setpoint1C := TargetC, Setpoint2C := LREAL#35.0, Vessel1C := Dc8T1Vessel1C, Vessel2C := Dc8T1Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc8T1Heater1Pct := Dc8T1Loops.Heater1Pct;
    Dc8T1Heater2Pct := Dc8T1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Dc8T2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := TargetC, Vessel1C := Dc8T2Vessel1C, Vessel2C := Dc8T2Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc8T2Heater1Pct := Dc8T2Loops.Heater1Pct;
    Dc8T2Heater2Pct := Dc8T2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Dc8L1Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Dc8L1Vessel1C, Vessel2C := Dc8L1Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc8L1Heater1Pct := Dc8L1Loops.Heater1Pct;
    Dc8L1Heater2Pct := Dc8L1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#8.0 THEN
    Dc8L2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Dc8L2Vessel1C, Vessel2C := Dc8L2Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc8L2Heater1Pct := Dc8L2Loops.Heater1Pct;
    Dc8L2Heater2Pct := Dc8L2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Dc10T1Loops(Setpoint1C := TargetC, Setpoint2C := LREAL#35.0, Vessel1C := Dc10T1Vessel1C, Vessel2C := Dc10T1Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc10T1Heater1Pct := Dc10T1Loops.Heater1Pct;
    Dc10T1Heater2Pct := Dc10T1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Dc10T2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := TargetC, Vessel1C := Dc10T2Vessel1C, Vessel2C := Dc10T2Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc10T2Heater1Pct := Dc10T2Loops.Heater1Pct;
    Dc10T2Heater2Pct := Dc10T2Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Dc10L1Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Dc10L1Vessel1C, Vessel2C := Dc10L1Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc10L1Heater1Pct := Dc10L1Loops.Heater1Pct;
    Dc10L1Heater2Pct := Dc10L1Loops.Heater2Pct;
END_IF;
IF ActiveCoupling = LREAL#10.0 THEN
    Dc10L2Loops(Setpoint1C := LREAL#35.0, Setpoint2C := LREAL#35.0, Vessel1C := Dc10L2Vessel1C, Vessel2C := Dc10L2Vessel2C, CrossGain := DecouplerCrossGain,
        DecouplerScale := DecouplerScale, ProportionalGain := DecoupledKp, IntegralGain := DecoupledKi);
    Dc10L2Heater1Pct := Dc10L2Loops.Heater1Pct;
    Dc10L2Heater2Pct := Dc10L2Loops.Heater2Pct;
END_IF;
(* The active set's temperatures and vessel-1 target heaters, under names that do not depend on the set. *)
IF ActiveCoupling = LREAL#6.0 THEN
    PiT1Vessel1C := Pi6T1Vessel1C;
    PiT1Vessel2C := Pi6T1Vessel2C;
    PiT2Vessel1C := Pi6T2Vessel1C;
    PiT2Vessel2C := Pi6T2Vessel2C;
    PiL1Vessel1C := Pi6L1Vessel1C;
    PiL1Vessel2C := Pi6L1Vessel2C;
    PiL2Vessel1C := Pi6L2Vessel1C;
    PiL2Vessel2C := Pi6L2Vessel2C;
    DcT1Vessel1C := Dc6T1Vessel1C;
    DcT1Vessel2C := Dc6T1Vessel2C;
    DcT2Vessel1C := Dc6T2Vessel1C;
    DcT2Vessel2C := Dc6T2Vessel2C;
    DcL1Vessel1C := Dc6L1Vessel1C;
    DcL1Vessel2C := Dc6L1Vessel2C;
    DcL2Vessel1C := Dc6L2Vessel1C;
    DcL2Vessel2C := Dc6L2Vessel2C;
    PiT1Heater1Pct := Pi6T1Heater1Pct;
    PiT1Heater2Pct := Pi6T1Heater2Pct;
    DcT1Heater1Pct := Dc6T1Heater1Pct;
    DcT1Heater2Pct := Dc6T1Heater2Pct;
    DcL1Heater1Pct := Dc6L1Heater1Pct;
    DcL1Heater2Pct := Dc6L1Heater2Pct;
ELSIF ActiveCoupling = LREAL#8.0 THEN
    PiT1Vessel1C := Pi8T1Vessel1C;
    PiT1Vessel2C := Pi8T1Vessel2C;
    PiT2Vessel1C := Pi8T2Vessel1C;
    PiT2Vessel2C := Pi8T2Vessel2C;
    PiL1Vessel1C := Pi8L1Vessel1C;
    PiL1Vessel2C := Pi8L1Vessel2C;
    PiL2Vessel1C := Pi8L2Vessel1C;
    PiL2Vessel2C := Pi8L2Vessel2C;
    DcT1Vessel1C := Dc8T1Vessel1C;
    DcT1Vessel2C := Dc8T1Vessel2C;
    DcT2Vessel1C := Dc8T2Vessel1C;
    DcT2Vessel2C := Dc8T2Vessel2C;
    DcL1Vessel1C := Dc8L1Vessel1C;
    DcL1Vessel2C := Dc8L1Vessel2C;
    DcL2Vessel1C := Dc8L2Vessel1C;
    DcL2Vessel2C := Dc8L2Vessel2C;
    PiT1Heater1Pct := Pi8T1Heater1Pct;
    PiT1Heater2Pct := Pi8T1Heater2Pct;
    DcT1Heater1Pct := Dc8T1Heater1Pct;
    DcT1Heater2Pct := Dc8T1Heater2Pct;
    DcL1Heater1Pct := Dc8L1Heater1Pct;
    DcL1Heater2Pct := Dc8L1Heater2Pct;
ELSIF ActiveCoupling = LREAL#10.0 THEN
    PiT1Vessel1C := Pi10T1Vessel1C;
    PiT1Vessel2C := Pi10T1Vessel2C;
    PiT2Vessel1C := Pi10T2Vessel1C;
    PiT2Vessel2C := Pi10T2Vessel2C;
    PiL1Vessel1C := Pi10L1Vessel1C;
    PiL1Vessel2C := Pi10L1Vessel2C;
    PiL2Vessel1C := Pi10L2Vessel1C;
    PiL2Vessel2C := Pi10L2Vessel2C;
    DcT1Vessel1C := Dc10T1Vessel1C;
    DcT1Vessel2C := Dc10T1Vessel2C;
    DcT2Vessel1C := Dc10T2Vessel1C;
    DcT2Vessel2C := Dc10T2Vessel2C;
    DcL1Vessel1C := Dc10L1Vessel1C;
    DcL1Vessel2C := Dc10L1Vessel2C;
    DcL2Vessel1C := Dc10L2Vessel1C;
    DcL2Vessel2C := Dc10L2Vessel2C;
    PiT1Heater1Pct := Pi10T1Heater1Pct;
    PiT1Heater2Pct := Pi10T1Heater2Pct;
    DcT1Heater1Pct := Dc10T1Heater1Pct;
    DcT1Heater2Pct := Dc10T1Heater2Pct;
    DcL1Heater1Pct := Dc10L1Heater1Pct;
    DcL1Heater2Pct := Dc10L1Heater2Pct;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Pi6T1Heater1Pct AT %QL0 : LREAL;
    P1.Pi6T1Heater2Pct AT %QL8 : LREAL;
    P1.Pi6T1Vessel1C AT %IL0 : LREAL;
    P1.Pi6T1Vessel2C AT %IL8 : LREAL;
    P1.Pi6T2Heater1Pct AT %QL16 : LREAL;
    P1.Pi6T2Heater2Pct AT %QL24 : LREAL;
    P1.Pi6T2Vessel1C AT %IL16 : LREAL;
    P1.Pi6T2Vessel2C AT %IL24 : LREAL;
    P1.Pi6L1Heater1Pct AT %QL32 : LREAL;
    P1.Pi6L1Heater2Pct AT %QL40 : LREAL;
    P1.Pi6L1Vessel1C AT %IL32 : LREAL;
    P1.Pi6L1Vessel2C AT %IL40 : LREAL;
    P1.Pi6L2Heater1Pct AT %QL48 : LREAL;
    P1.Pi6L2Heater2Pct AT %QL56 : LREAL;
    P1.Pi6L2Vessel1C AT %IL48 : LREAL;
    P1.Pi6L2Vessel2C AT %IL56 : LREAL;
    P1.Pi8T1Heater1Pct AT %QL64 : LREAL;
    P1.Pi8T1Heater2Pct AT %QL72 : LREAL;
    P1.Pi8T1Vessel1C AT %IL64 : LREAL;
    P1.Pi8T1Vessel2C AT %IL72 : LREAL;
    P1.Pi8T2Heater1Pct AT %QL80 : LREAL;
    P1.Pi8T2Heater2Pct AT %QL88 : LREAL;
    P1.Pi8T2Vessel1C AT %IL80 : LREAL;
    P1.Pi8T2Vessel2C AT %IL88 : LREAL;
    P1.Pi8L1Heater1Pct AT %QL96 : LREAL;
    P1.Pi8L1Heater2Pct AT %QL104 : LREAL;
    P1.Pi8L1Vessel1C AT %IL96 : LREAL;
    P1.Pi8L1Vessel2C AT %IL104 : LREAL;
    P1.Pi8L2Heater1Pct AT %QL112 : LREAL;
    P1.Pi8L2Heater2Pct AT %QL120 : LREAL;
    P1.Pi8L2Vessel1C AT %IL112 : LREAL;
    P1.Pi8L2Vessel2C AT %IL120 : LREAL;
    P1.Pi10T1Heater1Pct AT %QL128 : LREAL;
    P1.Pi10T1Heater2Pct AT %QL136 : LREAL;
    P1.Pi10T1Vessel1C AT %IL128 : LREAL;
    P1.Pi10T1Vessel2C AT %IL136 : LREAL;
    P1.Pi10T2Heater1Pct AT %QL144 : LREAL;
    P1.Pi10T2Heater2Pct AT %QL152 : LREAL;
    P1.Pi10T2Vessel1C AT %IL144 : LREAL;
    P1.Pi10T2Vessel2C AT %IL152 : LREAL;
    P1.Pi10L1Heater1Pct AT %QL160 : LREAL;
    P1.Pi10L1Heater2Pct AT %QL168 : LREAL;
    P1.Pi10L1Vessel1C AT %IL160 : LREAL;
    P1.Pi10L1Vessel2C AT %IL168 : LREAL;
    P1.Pi10L2Heater1Pct AT %QL176 : LREAL;
    P1.Pi10L2Heater2Pct AT %QL184 : LREAL;
    P1.Pi10L2Vessel1C AT %IL176 : LREAL;
    P1.Pi10L2Vessel2C AT %IL184 : LREAL;
    P1.Dc6T1Heater1Pct AT %QL192 : LREAL;
    P1.Dc6T1Heater2Pct AT %QL200 : LREAL;
    P1.Dc6T1Vessel1C AT %IL192 : LREAL;
    P1.Dc6T1Vessel2C AT %IL200 : LREAL;
    P1.Dc6T2Heater1Pct AT %QL208 : LREAL;
    P1.Dc6T2Heater2Pct AT %QL216 : LREAL;
    P1.Dc6T2Vessel1C AT %IL208 : LREAL;
    P1.Dc6T2Vessel2C AT %IL216 : LREAL;
    P1.Dc6L1Heater1Pct AT %QL224 : LREAL;
    P1.Dc6L1Heater2Pct AT %QL232 : LREAL;
    P1.Dc6L1Vessel1C AT %IL224 : LREAL;
    P1.Dc6L1Vessel2C AT %IL232 : LREAL;
    P1.Dc6L2Heater1Pct AT %QL240 : LREAL;
    P1.Dc6L2Heater2Pct AT %QL248 : LREAL;
    P1.Dc6L2Vessel1C AT %IL240 : LREAL;
    P1.Dc6L2Vessel2C AT %IL248 : LREAL;
    P1.Dc8T1Heater1Pct AT %QL256 : LREAL;
    P1.Dc8T1Heater2Pct AT %QL264 : LREAL;
    P1.Dc8T1Vessel1C AT %IL256 : LREAL;
    P1.Dc8T1Vessel2C AT %IL264 : LREAL;
    P1.Dc8T2Heater1Pct AT %QL272 : LREAL;
    P1.Dc8T2Heater2Pct AT %QL280 : LREAL;
    P1.Dc8T2Vessel1C AT %IL272 : LREAL;
    P1.Dc8T2Vessel2C AT %IL280 : LREAL;
    P1.Dc8L1Heater1Pct AT %QL288 : LREAL;
    P1.Dc8L1Heater2Pct AT %QL296 : LREAL;
    P1.Dc8L1Vessel1C AT %IL288 : LREAL;
    P1.Dc8L1Vessel2C AT %IL296 : LREAL;
    P1.Dc8L2Heater1Pct AT %QL304 : LREAL;
    P1.Dc8L2Heater2Pct AT %QL312 : LREAL;
    P1.Dc8L2Vessel1C AT %IL304 : LREAL;
    P1.Dc8L2Vessel2C AT %IL312 : LREAL;
    P1.Dc10T1Heater1Pct AT %QL320 : LREAL;
    P1.Dc10T1Heater2Pct AT %QL328 : LREAL;
    P1.Dc10T1Vessel1C AT %IL320 : LREAL;
    P1.Dc10T1Vessel2C AT %IL328 : LREAL;
    P1.Dc10T2Heater1Pct AT %QL336 : LREAL;
    P1.Dc10T2Heater2Pct AT %QL344 : LREAL;
    P1.Dc10T2Vessel1C AT %IL336 : LREAL;
    P1.Dc10T2Vessel2C AT %IL344 : LREAL;
    P1.Dc10L1Heater1Pct AT %QL352 : LREAL;
    P1.Dc10L1Heater2Pct AT %QL360 : LREAL;
    P1.Dc10L1Vessel1C AT %IL352 : LREAL;
    P1.Dc10L1Vessel2C AT %IL360 : LREAL;
    P1.Dc10L2Heater1Pct AT %QL368 : LREAL;
    P1.Dc10L2Heater2Pct AT %QL376 : LREAL;
    P1.Dc10L2Vessel1C AT %IL368 : LREAL;
    P1.Dc10L2Vessel2C AT %IL376 : LREAL;
    P1.SelectedLoadKw AT %QL384 : LREAL;
    P1.NoLoadKw AT %QL392 : LREAL;
    P1.TargetC AT %ML0 : LREAL;
    P1.LoadKw AT %ML8 : LREAL;
    P1.ActiveCoupling AT %ML24 : LREAL;
    P1.PiT1Vessel1C AT %ML32 : LREAL;
    P1.PiT1Vessel2C AT %ML40 : LREAL;
    P1.PiT2Vessel1C AT %ML48 : LREAL;
    P1.PiT2Vessel2C AT %ML56 : LREAL;
    P1.PiL1Vessel1C AT %ML64 : LREAL;
    P1.PiL1Vessel2C AT %ML72 : LREAL;
    P1.PiL2Vessel1C AT %ML80 : LREAL;
    P1.PiL2Vessel2C AT %ML88 : LREAL;
    P1.DcT1Vessel1C AT %ML96 : LREAL;
    P1.DcT1Vessel2C AT %ML104 : LREAL;
    P1.DcT2Vessel1C AT %ML112 : LREAL;
    P1.DcT2Vessel2C AT %ML120 : LREAL;
    P1.DcL1Vessel1C AT %ML128 : LREAL;
    P1.DcL1Vessel2C AT %ML136 : LREAL;
    P1.DcL2Vessel1C AT %ML144 : LREAL;
    P1.DcL2Vessel2C AT %ML152 : LREAL;
    P1.PiT1Heater1Pct AT %ML160 : LREAL;
    P1.PiT1Heater2Pct AT %ML168 : LREAL;
    P1.DcT1Heater1Pct AT %ML176 : LREAL;
    P1.DcT1Heater2Pct AT %ML184 : LREAL;
    P1.DcL1Heater1Pct AT %ML192 : LREAL;
    P1.DcL1Heater2Pct AT %ML200 : LREAL;
    P1.ExperimentEnabled AT %MX16.0 : BOOL;
END_VAR
END_CONFIGURATION
