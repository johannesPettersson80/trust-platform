TEST_PROGRAM oscat_plc_clock_helpers_behave
VAR
FirstMs : DWORD;
SecondMs : DWORD;
ClockUs : DWORD;
END_VAR

FirstMs := T_PLC_MS();
SecondMs := T_PLC_MS();
ClockUs := T_PLC_US();

ASSERT_TRUE(IN := SecondMs >= FirstMs);
ASSERT_TRUE(IN := DWORD_TO_UDINT(ClockUs) >= DWORD_TO_UDINT(FirstMs) * UDINT#1000);
END_TEST_PROGRAM

TEST_PROGRAM oscat_measuring_cycle_time_helpers_behave
VAR
    CycleMs : TC_MS;
    CycleS : TC_S;
    CycleUs : TC_US;
END_VAR

CycleMs();
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := CycleMs.TC);
CycleS();
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := CycleS.TC, DELTA := REAL#0.0001);
CycleUs();
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := CycleUs.TC);
END_TEST_PROGRAM

TEST_PROGRAM oscat_measuring_modules_ontime_behaves
VAR
    Meter : ONTIME;
    Seconds : UDINT;
    Cycles : UDINT;
END_VAR

Meter(IN := FALSE, RST := TRUE, SECONDS := Seconds, CYCLES := Cycles);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Seconds);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Cycles);

Meter(IN := TRUE, RST := FALSE, SECONDS := Seconds, CYCLES := Cycles);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Cycles);
END_TEST_PROGRAM

TEST_PROGRAM oscat_measuring_window_and_calibration_helpers_behave
VAR
    AlarmWindow : ALARM_2;
    Graph : BAR_GRAPH;
    CalibrateInput : CALIBRATE;
END_VAR

AlarmWindow(
    X := REAL#5.0,
    LO_1 := REAL#10.0,
    HI_1 := REAL#20.0,
    LO_2 := REAL#0.0,
    HI_2 := REAL#30.0,
    HYS := REAL#2.0
);
Graph(
    X := REAL#80.0,
    rst := FALSE,
    trigger_Low := REAL#10.0,
    trigger_High := REAL#70.0,
    Alarm_low := TRUE,
    Alarm_high := TRUE,
    log_scale := FALSE
);
CalibrateInput(
    X := REAL#3.0,
    CO := TRUE,
    CS := FALSE,
    Y_Offset := REAL#10.0,
    Y_Scale := REAL#2.0
);
CalibrateInput(
    X := REAL#5.0,
    CO := FALSE,
    CS := FALSE,
    Y_Offset := REAL#10.0,
    Y_Scale := REAL#2.0
);

ASSERT_TRUE(IN := AlarmWindow.Q1_LO);
ASSERT_FALSE(IN := AlarmWindow.Q1_HI);
ASSERT_TRUE(IN := Graph.HIGH);
ASSERT_TRUE(IN := Graph.Alarm);
ASSERT_NEAR(EXPECTED := REAL#12.0, ACTUAL := CalibrateInput.Y, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_measuring_runtime_and_meter_helpers_behave
VAR
    Cycle : CYCLE_TIME;
    SimClock : DT_SIMU;
    Flow : FLOW_METER;
    BetweenEdges : M_D;
    PulseWidth : M_T;
    SignalTiming : M_TX;
    Consumption : METER;
    Stats : METER_STAT;
    FlowX : REAL;
    FlowY : UDINT;
    TotalEnergy : REAL;
    LastDay : REAL;
    CurrentDay : REAL;
    LastWeek : REAL;
    CurrentWeek : REAL;
    LastMonth : REAL;
    CurrentMonth : REAL;
    LastYear : REAL;
    CurrentYear : REAL;
END_VAR

Cycle(RST := TRUE);
SimClock(START := DT#2026-01-01-00:00:00, SPEED := REAL#0.0);
SimClock(START := DT#2026-01-01-00:00:00, SPEED := REAL#0.0);

Flow(VX := REAL#0.6, E := FALSE, RST := TRUE, PULSE_MODE := TRUE, UPDATE_TIME := T#1s, X := FlowX, Y := FlowY);
Flow(VX := REAL#0.6, E := TRUE, RST := FALSE, PULSE_MODE := TRUE, UPDATE_TIME := T#1s, X := FlowX, Y := FlowY);

BetweenEdges(start := FALSE, stop := FALSE, rst := TRUE);
BetweenEdges(start := TRUE, stop := FALSE, rst := FALSE);
BetweenEdges(start := TRUE, stop := TRUE, rst := FALSE);

PulseWidth(IN := FALSE, RST := TRUE);
PulseWidth(IN := TRUE, RST := FALSE);
PulseWidth(IN := FALSE, RST := FALSE);

SignalTiming(IN := FALSE, rst := TRUE);
SignalTiming(IN := TRUE, rst := FALSE);
SignalTiming(IN := FALSE, rst := FALSE);

Consumption(M1 := REAL#10.0, M2 := REAL#5.0, I1 := TRUE, I2 := FALSE, D := REAL#1.0, RST := TRUE, MX := TotalEnergy);

Stats(IN := REAL#100.0, DI := DATE#2026-04-15, RST := TRUE, Last_Day := LastDay, Current_Day := CurrentDay, Last_Week := LastWeek, Current_Week := CurrentWeek, Last_Month := LastMonth, Current_Month := CurrentMonth, Last_Year := LastYear, Current_Year := CurrentYear);
Stats(IN := REAL#130.0, DI := DATE#2026-04-15, RST := FALSE, Last_Day := LastDay, Current_Day := CurrentDay, Last_Week := LastWeek, Current_Week := CurrentWeek, Last_Month := LastMonth, Current_Month := CurrentMonth, Last_Year := LastYear, Current_Year := CurrentYear);
Stats(IN := REAL#140.0, DI := DATE#2026-04-16, RST := FALSE, Last_Day := LastDay, Current_Day := CurrentDay, Last_Week := LastWeek, Current_Week := CurrentWeek, Last_Month := LastMonth, Current_Month := CurrentMonth, Last_Year := LastYear, Current_Year := CurrentYear);

ASSERT_TRUE(IN := DWORD_TO_UDINT(Cycle.cycles) >= UDINT#1);
ASSERT_EQUAL(EXPECTED := DT#2026-01-01-00:00:01, ACTUAL := SimClock.DTS);
ASSERT_NEAR(EXPECTED := REAL#0.6, ACTUAL := FlowX, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := FlowY);
ASSERT_FALSE(IN := BetweenEdges.run);
ASSERT_EQUAL(EXPECTED := T#0s, ACTUAL := PulseWidth.PT);
ASSERT_EQUAL(EXPECTED := REAL#0.0, ACTUAL := SignalTiming.F);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := TotalEnergy, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#40.0, ACTUAL := LastDay, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := CurrentDay, DELTA := REAL#0.001);
END_TEST_PROGRAM
