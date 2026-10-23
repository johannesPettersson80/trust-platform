(* The OSCAT blocks that measure time, run scan after scan of 10 ms (specification 31 §2.4, §2.5):
   each takes the milliseconds of the PLC timer as numbers. The test oscat_timed_blocks.rs reads
   the values below after given scans. *)
PROGRAM Timed
VAR
    Scan : INT;
    Ready : BOOL;
    Pt1 : FT_PT1;
    Pt1Out : REAL;
    Simulated : DT_SIMU;
    SimulatedOneSecond : BOOL;
    SimulatedTwoSeconds : BOOL;
    Flow : FLOW_METER;
    FlowVolume : REAL;
    FlowUnits : UDINT;
    FlowRate : REAL;
    Pulses : M_TX;
    PulseFrequency : REAL;
    PulseDuty : REAL;
    Heat : HEAT_METER;
    HeatEnergy : REAL;
    HeatRate : REAL;
    Profile : FT_Profile;
    ProfileOut : REAL;
    Random : REAL;
END_VAR
Scan := Scan + INT#1;
Ready := OSCAT_BASIC_Constants();

(* A step from 0 to 1 after the first scan, through a first-order lag of 1 s. *)
Pt1(in := SEL(G := Scan > INT#1, IN0 := REAL#0.0, IN1 := REAL#1.0), T := T#1s, K := REAL#1.0);
Pt1Out := Pt1.out;

(* A simulated clock at twice the PLC's speed. *)
Simulated(START := DT#2026-01-01-00:00:00, SPEED := REAL#2.0);
SimulatedOneSecond := Simulated.DTS = DT#2026-01-01-00:00:01;
SimulatedTwoSeconds := Simulated.DTS = DT#2026-01-01-00:00:02;

(* A flow of 3600 units an hour, its rate updated every second. *)
Flow(VX := REAL#3600.0, E := TRUE, RST := FALSE, UPDATE_TIME := T#1s, X := FlowVolume, Y := FlowUnits);
FlowRate := Flow.F;

(* Pulses 30 ms high and 70 ms low: 10 Hz at a duty cycle of 0.3. *)
Pulses(IN := Scan MOD INT#10 >= INT#1 AND Scan MOD INT#10 <= INT#3);
PulseFrequency := Pulses.F;
PulseDuty := Pulses.DC;

(* A heat meter whose medium is all content of heat capacity 1 and density 1: 10 K across it at
   3600 litres an hour delivers 10 units a second. *)
Heat(TF := REAL#60.0, TR := REAL#50.0, LPH := REAL#3600.0, E := TRUE, RST := FALSE, CP := REAL#1.0, Density := REAL#1.0, Content := REAL#1.0, AVG_time := T#1s, Y := HeatEnergy);
HeatRate := Heat.C;

(* A profile that ramps from 0 to 100 over its first second. *)
Profile(E := TRUE, value_0 := REAL#0.0, time_1 := T#1s, value_1 := REAL#100.0, time_2 := T#2s, value_2 := REAL#100.0, time_3 := T#3s, value_3 := REAL#0.0);
ProfileOut := Profile.Y;

Random := RDM(LAST := REAL#0.5);
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Cyclic (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Cyclic : Timed;
END_RESOURCE
END_CONFIGURATION
