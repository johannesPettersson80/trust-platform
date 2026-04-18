TEST_PROGRAM oscat_logic_generator_triggers_behave
VAR
    AnalogTrigger : A_TRIG;
    BothEdgeTrigger : B_TRIG;
    DWordTrigger : D_TRIG;
END_VAR

AnalogTrigger(IN := REAL#1.0, RES := REAL#0.5);
ASSERT_TRUE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

AnalogTrigger(IN := REAL#1.0, RES := REAL#0.5);
ASSERT_FALSE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

AnalogTrigger(IN := REAL#1.3, RES := REAL#0.5);
ASSERT_FALSE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.3, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

AnalogTrigger(IN := REAL#1.6, RES := REAL#0.5);
ASSERT_TRUE(IN := AnalogTrigger.Q);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := AnalogTrigger.D, DELTA := REAL#0.0001);

BothEdgeTrigger(CLK := FALSE);
ASSERT_FALSE(IN := BothEdgeTrigger.Q);

BothEdgeTrigger(CLK := TRUE);
ASSERT_TRUE(IN := BothEdgeTrigger.Q);

BothEdgeTrigger(CLK := TRUE);
ASSERT_FALSE(IN := BothEdgeTrigger.Q);

BothEdgeTrigger(CLK := FALSE);
ASSERT_TRUE(IN := BothEdgeTrigger.Q);

DWordTrigger(IN := DWORD#0);
ASSERT_FALSE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordTrigger.X);

DWordTrigger(IN := DWORD#1);
ASSERT_TRUE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := DWordTrigger.X);

DWordTrigger(IN := DWORD#1);
ASSERT_FALSE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordTrigger.X);

DWordTrigger(IN := DWORD#2);
ASSERT_TRUE(IN := DWordTrigger.Q);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := DWordTrigger.X);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_generator_click_and_divider_helpers_behave
VAR
    ClickCounter : CLICK_CNT;
    ClickDecoder : CLICK_DEC;
    Divider : CLK_DIV;
END_VAR

ClickCounter(IN := TRUE, N := INT#0, TC := T#0ms);
ASSERT_FALSE(IN := ClickCounter.Q);

ClickCounter(IN := FALSE, N := INT#0, TC := T#0ms);
ASSERT_TRUE(IN := ClickCounter.Q);

ClickCounter(IN := FALSE, N := INT#0, TC := T#0ms);
ASSERT_FALSE(IN := ClickCounter.Q);

ClickDecoder(IN := TRUE, TC := T#0ms);
ASSERT_FALSE(IN := ClickDecoder.Q0);
ASSERT_FALSE(IN := ClickDecoder.Q1);

ClickDecoder(IN := FALSE, TC := T#0ms);
ASSERT_TRUE(IN := ClickDecoder.Q0);
ASSERT_FALSE(IN := ClickDecoder.Q1);

ClickDecoder(IN := FALSE, TC := T#0ms);
ASSERT_FALSE(IN := ClickDecoder.Q0);
ASSERT_FALSE(IN := ClickDecoder.Q1);

Divider(CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := Divider.Q0);
ASSERT_FALSE(IN := Divider.Q1);

Divider(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Divider.Q0);
ASSERT_FALSE(IN := Divider.Q1);

Divider(CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Divider.Q0);
ASSERT_TRUE(IN := Divider.Q1);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_generator_pattern_and_cycle_helpers_behave
VAR
    Pattern : GEN_BIT;
    Cycle4 : CYCLE_4;
END_VAR

Pattern(
    IN0 := DWORD#16#5,
    IN1 := DWORD#16#3,
    IN2 := DWORD#16#1,
    IN3 := DWORD#16#0,
    CLK := TRUE,
    STEPS := INT#3,
    REP := INT#1,
    RST := FALSE
);
ASSERT_TRUE(IN := Pattern.RUN);
ASSERT_TRUE(IN := Pattern.Q0);
ASSERT_TRUE(IN := Pattern.Q1);
ASSERT_TRUE(IN := Pattern.Q2);
ASSERT_FALSE(IN := Pattern.Q3);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Pattern.CNT);

Pattern(
    IN0 := DWORD#16#5,
    IN1 := DWORD#16#3,
    IN2 := DWORD#16#1,
    IN3 := DWORD#16#0,
    CLK := TRUE,
    STEPS := INT#3,
    REP := INT#1,
    RST := FALSE
);
ASSERT_TRUE(IN := Pattern.RUN);
ASSERT_FALSE(IN := Pattern.Q0);
ASSERT_TRUE(IN := Pattern.Q1);
ASSERT_FALSE(IN := Pattern.Q2);
ASSERT_FALSE(IN := Pattern.Q3);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Pattern.CNT);

Pattern(
    IN0 := DWORD#16#5,
    IN1 := DWORD#16#3,
    IN2 := DWORD#16#1,
    IN3 := DWORD#16#0,
    CLK := TRUE,
    STEPS := INT#3,
    REP := INT#1,
    RST := FALSE
);
ASSERT_FALSE(IN := Pattern.RUN);
ASSERT_TRUE(IN := Pattern.Q0);
ASSERT_FALSE(IN := Pattern.Q1);
ASSERT_FALSE(IN := Pattern.Q2);
ASSERT_FALSE(IN := Pattern.Q3);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Pattern.CNT);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cycle4.STATE);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cycle4.STATE);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Cycle4.STATE);

Cycle4(
    E := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms,
    S0 := TRUE,
    SX := INT#0,
    SL := FALSE
);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cycle4.STATE);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_scheduler_helpers_behave
VAR
    TimeScheduler : SCHEDULER;
    CycleScheduler : SCHEDULER_2;
END_VAR

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_TRUE(IN := TimeScheduler.Q0);
ASSERT_FALSE(IN := TimeScheduler.Q1);
ASSERT_FALSE(IN := TimeScheduler.Q2);
ASSERT_FALSE(IN := TimeScheduler.Q3);

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_FALSE(IN := TimeScheduler.Q0);
ASSERT_TRUE(IN := TimeScheduler.Q1);
ASSERT_FALSE(IN := TimeScheduler.Q2);
ASSERT_FALSE(IN := TimeScheduler.Q3);

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_FALSE(IN := TimeScheduler.Q0);
ASSERT_FALSE(IN := TimeScheduler.Q1);
ASSERT_TRUE(IN := TimeScheduler.Q2);
ASSERT_FALSE(IN := TimeScheduler.Q3);

TimeScheduler(
    E0 := TRUE,
    E1 := TRUE,
    E2 := TRUE,
    E3 := TRUE,
    T0 := T#0ms,
    T1 := T#0ms,
    T2 := T#0ms,
    T3 := T#0ms
);
ASSERT_FALSE(IN := TimeScheduler.Q0);
ASSERT_FALSE(IN := TimeScheduler.Q1);
ASSERT_FALSE(IN := TimeScheduler.Q2);
ASSERT_TRUE(IN := TimeScheduler.Q3);

CycleScheduler(
    E0 := TRUE,
    E1 := FALSE,
    E2 := FALSE,
    E3 := FALSE,
    C0 := UINT#2,
    C1 := UINT#1,
    C2 := UINT#1,
    C3 := UINT#1,
    O0 := UINT#0,
    O1 := UINT#0,
    O2 := UINT#0,
    O3 := UINT#0
);
ASSERT_TRUE(IN := CycleScheduler.Q0);

CycleScheduler(
    E0 := TRUE,
    E1 := FALSE,
    E2 := FALSE,
    E3 := FALSE,
    C0 := UINT#2,
    C1 := UINT#1,
    C2 := UINT#1,
    C3 := UINT#1,
    O0 := UINT#0,
    O1 := UINT#0,
    O2 := UINT#0,
    O3 := UINT#0
);
ASSERT_FALSE(IN := CycleScheduler.Q0);

CycleScheduler(
    E0 := TRUE,
    E1 := FALSE,
    E2 := FALSE,
    E3 := FALSE,
    C0 := UINT#2,
    C1 := UINT#1,
    C2 := UINT#1,
    C3 := UINT#1,
    O0 := UINT#0,
    O1 := UINT#0,
    O2 := UINT#0,
    O3 := UINT#0
);
ASSERT_TRUE(IN := CycleScheduler.Q0);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_sequence4_helpers_behave
VAR
    Sequence4 : SEQUENCE_4;
END_VAR

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := TRUE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_TRUE(IN := Sequence4.RUN);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Sequence4.STATE);
ASSERT_TRUE(IN := Sequence4.Q0);
ASSERT_TRUE(IN := Sequence4.QX);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Sequence4.STATE);
ASSERT_FALSE(IN := Sequence4.Q0);
ASSERT_TRUE(IN := Sequence4.Q1);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Sequence4.STATE);
ASSERT_TRUE(IN := Sequence4.Q2);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Sequence4.STATE);
ASSERT_TRUE(IN := Sequence4.Q3);

Sequence4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
ASSERT_FALSE(IN := Sequence4.RUN);
ASSERT_EQUAL(EXPECTED := BYTE#110, ACTUAL := Sequence4.STATUS);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_sequence8_helpers_behave
VAR
    Sequence8 : SEQUENCE_8;
END_VAR

Sequence8(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    IN4 := TRUE,
    IN5 := TRUE,
    IN6 := TRUE,
    IN7 := TRUE,
    START := TRUE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms,
    WAIT4 := T#0ms,
    DELAY4 := T#0ms,
    WAIT5 := T#0ms,
    DELAY5 := T#0ms,
    WAIT6 := T#0ms,
    DELAY6 := T#0ms,
    WAIT7 := T#0ms,
    DELAY7 := T#0ms
);
ASSERT_TRUE(IN := Sequence8.Q0);

Sequence8(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    IN4 := TRUE,
    IN5 := TRUE,
    IN6 := TRUE,
    IN7 := TRUE,
    START := FALSE,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms,
    WAIT4 := T#0ms,
    DELAY4 := T#0ms,
    WAIT5 := T#0ms,
    DELAY5 := T#0ms,
    WAIT6 := T#0ms,
    DELAY6 := T#0ms,
    WAIT7 := T#0ms,
    DELAY7 := T#0ms
);
ASSERT_TRUE(IN := Sequence8.Q1);
ASSERT_FALSE(IN := Sequence8.Q0);
END_TEST_PROGRAM


TEST_PROGRAM oscat_logic_programmable_clock_behaves
VAR
    Clock : CLK_PRG;
END_VAR

Clock(PT := T#1d);
ASSERT_TRUE(IN := Clock.Q);

Clock(PT := T#1d);
ASSERT_FALSE(IN := Clock.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_dead_time_pulse_helper_behaves
VAR
    Pulse1D : TP_1D;
END_VAR

Pulse1D(IN := TRUE, PT1 := T#0ms, PTD := T#0ms, RST := FALSE);
ASSERT_TRUE(IN := Pulse1D.Q);

Pulse1D(IN := FALSE, PT1 := T#0ms, PTD := T#0ms, RST := FALSE);
ASSERT_FALSE(IN := Pulse1D.Q);
ASSERT_TRUE(IN := Pulse1D.W);

Pulse1D(IN := FALSE, PT1 := T#0ms, PTD := T#0ms, RST := FALSE);
ASSERT_FALSE(IN := Pulse1D.W);
END_TEST_PROGRAM
