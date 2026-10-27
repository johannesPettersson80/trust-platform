(* The OSCAT blocks that read the PLC timer, run across its wraps (specification 31 §2.1, §2.4):
   T_PLC_US wraps after 2^32 us (about 71.6 minutes) and T_PLC_MS after 2^32 ms (about 49.7
   days), as the upstream DWORD timers do. The test oscat_timer_wrap.rs starts the test clock just
   before a wrap and reads the values below after given scans; every block must keep running
   without a fault. *)
PROGRAM Wrap
VAR
    Scan : INT;
    Ready : BOOL;
    (* Values the test checks. *)
    TimerMs : DWORD;
    TimerUs : DWORD;
    CycleMs : TC_MS;
    CycleUs : TC_US;
    CycleSeconds : TC_S;
    Since : M_T;
    Pulse : TP_X;
    Square : GEN_SQ;
    CycleTime : CYCLE_TIME;
    Lag : FT_PT1;
    (* Blocks that must keep running across the wraps. *)
    OnSeconds : UDINT;
    OnCycles : UDINT;
    OnTime : ONTIME;
    Simulated : DT_SIMU;
    FlowVolume : REAL;
    FlowUnits : UDINT;
    Flow : FLOW_METER;
    Duration : M_D;
    Pulses : M_TX;
    MeterValue : REAL;
    Meter1 : METER;
    Coil : ACTUATOR_COIL;
    Runtime1 : UDINT;
    Runtime2 : UDINT;
    BurnerCycles : UDINT;
    Burner : BURNER;
    Derivative : FT_DERIV;
    PiLimited : FT_PIWL;
    Delay8 : FT_TN8;
    Delay16 : FT_TN16;
    Delay64 : FT_TN64;
    HeatEnergy : REAL;
    Heat : HEAT_METER;
    Integrated : REAL;
    Integrator : INTEGRATE;
    Outside : TEMP_EXT;
    Pid : CTRL_PID;
    ClockN : CLK_N;
    ClockProgram : CLK_PRG;
    ClockPulse : CLK_PULSE;
    Cycle4 : CYCLE_4;
    Schedule : SCHEDULER;
    Sequence4 : SEQUENCE_4;
    Sequence8 : SEQUENCE_8;
    Pulse1D : TP_1D;
    GenPulse : GEN_PULSE;
    GenPw2 : GEN_PW2;
    GenRdm : GEN_RDM;
    GenRmp : GEN_RMP;
    GenSin : GEN_SIN;
    GenSqr : GEN_SQR;
    RampB : RMP_B;
    RampW : RMP_W;
    FilterDw : FILTER_DW;
    FilterI : FILTER_I;
    FilterW : FILTER_W;
    Hold1 : SH_1;
    Hold2 : SH_2;
    Profile : FT_Profile;
    Params : PARSET;
    Light : SIGNAL;
    Tuner : TUNE;
    Tuner2 : TUNE2;
    Radio : DCF77;
    Clock : RTC_MS;
    Ramp : FT_RMP;
    Random : REAL;
    Rate : DerivativeFilter;
    RateOut : REAL;
END_VAR
Scan := Scan + INT#1;
Ready := OSCAT_BASIC_Constants();
IF Scan = INT#1 THEN
    Rate.Initialize();
    Rate.Configure(Gain := REAL#1.0);
END_IF;

TimerMs := T_PLC_MS();
TimerUs := T_PLC_US();
CycleMs();
CycleUs();
CycleSeconds();
Since(IN := TRUE, TMAX := T#10d, RST := FALSE);
Pulse(IN := TRUE, PT := T#400ms);
Square(PT := T#100ms);
CycleTime(RST := FALSE);
Lag(in := SEL(G := Scan > INT#1, IN0 := REAL#0.0, IN1 := REAL#1.0), T := T#1s, K := REAL#1.0);

OnTime(IN := TRUE, RST := FALSE, SECONDS := OnSeconds, CYCLES := OnCycles);
Simulated(START := DT#2026-01-01-00:00:00, SPEED := REAL#2.0);
Flow(VX := REAL#3600.0, E := TRUE, RST := FALSE, UPDATE_TIME := T#100ms, X := FlowVolume, Y := FlowUnits);
Duration(start := Scan = INT#1, stop := FALSE, rst := FALSE);
Pulses(IN := Scan MOD INT#10 < INT#3, rst := FALSE);
Meter1(M1 := REAL#1.0, M2 := REAL#2.0, I1 := TRUE, I2 := TRUE, D := REAL#1.0, RST := FALSE, MX := MeterValue);
Coil(IN := FALSE, self_act_cycle := T#100ms, self_act_time := T#50ms);
Burner(in := TRUE, stage2 := FALSE, over_temp := FALSE, oil_temp := TRUE, Flame := Scan > INT#20, rst := FALSE, rst_timer := FALSE, pre_heat_time := T#50ms, pre_vent_time := T#100ms, pre_ignite_time := T#50ms, post_ignite_time := T#100ms, stage2_delay := T#100ms, safety_time := T#200ms, lockout_time := T#100ms, KW1 := REAL#1.0, KW2 := REAL#2.0, runtime1 := Runtime1, runtime2 := Runtime2, cycles := BurnerCycles);
Derivative(in := INT_TO_REAL(Scan), K := REAL#1.0);
PiLimited(IN := REAL#1.0, KP := REAL#1.0, KI := REAL#1.0, LIM_L := REAL#-100.0, LIM_H := REAL#100.0);
Delay8(in := INT_TO_REAL(Scan), T := T#80ms);
Delay16(in := INT_TO_REAL(Scan), T := T#160ms);
Delay64(in := INT_TO_REAL(Scan), T := T#640ms);
Heat(TF := REAL#60.0, TR := REAL#50.0, LPH := REAL#3600.0, E := TRUE, RST := FALSE, CP := REAL#1.0, Density := REAL#1.0, Content := REAL#1.0, AVG_time := T#100ms, Y := HeatEnergy);
Integrator(E := TRUE, X := REAL#1.0, K := REAL#1.0, Y := Integrated);
Outside(t_ext1 := REAL#5.0, t_ext2 := REAL#5.0, t_ext3 := REAL#5.0, t_ext_config := BYTE#0, dt_in := DT#2026-01-15-12:00:00, CYCLE_TIME := T#100ms);
Pid(ACT := REAL#10.0, SET_POINT := REAL#20.0, SUP := REAL#0.0, OFS := REAL#0.0, M_I := REAL#0.0, MAN := FALSE, RST := FALSE, KP := REAL#1.0, TN := REAL#1.0, TV := REAL#0.1, LL := REAL#-100.0, LH := REAL#100.0);
ClockN(N := INT#3);
ClockProgram(PT := T#30ms);
ClockPulse(PT := T#30ms, N := INT#0, RST := FALSE);
Cycle4(E := TRUE, T0 := T#30ms, T1 := T#30ms, T2 := T#30ms, T3 := T#30ms, S0 := TRUE, SX := INT#0, SL := FALSE);
Schedule(E0 := TRUE, E1 := TRUE, E2 := TRUE, E3 := TRUE, T0 := T#50ms, T1 := T#70ms, T2 := T#90ms, T3 := T#110ms);
Sequence4(START := Scan = INT#1, RST := FALSE, WAIT0 := T#30ms, DELAY0 := T#30ms, WAIT1 := T#30ms, DELAY1 := T#30ms, WAIT2 := T#30ms, DELAY2 := T#30ms, WAIT3 := T#30ms, DELAY3 := T#30ms);
Sequence8(START := Scan = INT#1, RST := FALSE, WAIT0 := T#20ms, DELAY0 := T#20ms, WAIT1 := T#20ms, DELAY1 := T#20ms, WAIT2 := T#20ms, DELAY2 := T#20ms, WAIT3 := T#20ms, DELAY3 := T#20ms, WAIT4 := T#20ms, DELAY4 := T#20ms, WAIT5 := T#20ms, DELAY5 := T#20ms, WAIT6 := T#20ms, DELAY6 := T#20ms, WAIT7 := T#20ms, DELAY7 := T#20ms);
Pulse1D(IN := Scan MOD INT#20 = INT#1, PT1 := T#50ms, PTD := T#50ms, RST := FALSE);
GenPulse(ENQ := TRUE, PTH := T#30ms, PTL := T#20ms);
GenPw2(ENQ := TRUE, TH1 := T#30ms, TL1 := T#20ms, TH2 := T#40ms, TL2 := T#10ms, TS := FALSE);
GenRdm(PT := T#50ms);
GenRmp(PT := T#200ms);
GenSin(PT := T#200ms);
GenSqr(PT := T#200ms);
RampB(SET_HIGH := FALSE, PT := T#500ms, E := TRUE, UP := TRUE, RST := FALSE);
RampW(SET_HIGH := FALSE, PT := T#500ms, E := TRUE, UP := TRUE, RST := FALSE);
FilterDw(X := DWORD#1000, T := T#200ms);
FilterI(X := INT#100, T := T#200ms);
FilterW(X := WORD#100, T := T#200ms);
Hold1(in := INT_TO_REAL(Scan), PT := T#50ms);
Hold2(in := INT_TO_REAL(Scan), PT := T#50ms, N := INT#4, disc := INT#0);
Profile(E := TRUE, value_0 := REAL#0.0, time_1 := T#200ms, value_1 := REAL#100.0, time_2 := T#400ms, value_2 := REAL#100.0, time_3 := T#600ms, value_3 := REAL#0.0);
Params(A0 := Scan > INT#10, A1 := FALSE, TC := T#100ms);
Light(IN := TRUE, SIG := BYTE#2#10101010, TS := T#30ms);
Tuner(SET_REQ := FALSE, SU := TRUE, SD := FALSE, RST := FALSE, T1 := T#50ms, T2 := T#200ms);
Tuner2(SET_REQ := FALSE, SU := TRUE, SD := FALSE, FU := FALSE, FD := FALSE, RST := FALSE, TR := T#50ms);
Radio(REC := Scan MOD INT#100 < INT#10, SET_RTC := FALSE, SDT := DT#2026-01-01-00:00:00, DSI := FALSE);
Clock(SET_RTC := Scan = INT#1, SDT := DT#2026-01-01-00:00:00, SMS := INT#0);
Ramp(RMP := TRUE, IN := INT_TO_REAL(Scan MOD INT#20), KR := REAL#100.0, KF := REAL#100.0);
Random := RDM(LAST := REAL#0.5);
RateOut := Rate.Update(Sample := INT_TO_REAL(Scan));
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Cyclic (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Cyclic : Wrap;
END_RESOURCE
END_CONFIGURATION
