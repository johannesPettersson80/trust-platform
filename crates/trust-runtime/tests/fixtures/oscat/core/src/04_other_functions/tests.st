TEST_PROGRAM oscat_constants_resolve_and_match_ported_values
ASSERT_TRUE(IN := OSCAT_BASIC_Constants());
ASSERT_EQUAL(EXPECTED := INT#250, ACTUAL := STRING_LENGTH);
ASSERT_EQUAL(EXPECTED := INT#250, ACTUAL := LIST_LENGTH);
ASSERT_NEAR(EXPECTED := REAL#6.283185307179586, ACTUAL := MATH.PI2, DELTA := REAL#0.0001);
ASSERT_EQUAL(EXPECTED := DINT#120, ACTUAL := MATH.FACTS[5]);
ASSERT_NEAR(EXPECTED := REAL#-273.15, ACTUAL := PHYS.T0, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#101325.0, ACTUAL := PHYS.PN, DELTA := REAL#0.1);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := LANGUAGE.DEFAULT);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := LANGUAGE.LMAX);
END_TEST_PROGRAM

FUNCTION_BLOCK OSCAT_FB_INPUT_DEFAULT_PROBE
VAR_INPUT
    BASE : INT;
    INC : INT := INT#5;
END_VAR
VAR_OUTPUT
    RESULT : INT;
END_VAR

RESULT := BASE + INC;
END_FUNCTION_BLOCK

TEST_PROGRAM oscat_fb_omitted_input_defaults_follow_iec_state_rules
VAR
Probe : OSCAT_FB_INPUT_DEFAULT_PROBE;
END_VAR

Probe(BASE := INT#3);
ASSERT_EQUAL(EXPECTED := INT#8, ACTUAL := Probe.RESULT);
Probe(BASE := INT#3, INC := INT#9);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := Probe.RESULT);
Probe(BASE := INT#3);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := Probe.RESULT);
END_TEST_PROGRAM

TEST_PROGRAM oscat_status_to_esr_and_version_behave
VAR
    ErrorEvent : ESR_DATA;
    StatusEvent : ESR_DATA;
    DebugEvent : ESR_DATA;
    VersionDate : DWORD;
END_VAR

ErrorEvent := STATUS_TO_ESR(status := BYTE#42, adress := 'ERR', DT_in := DT#2026-04-16-12:00:00, TS := TIME#3s);
StatusEvent := STATUS_TO_ESR(status := BYTE#150, adress := 'STS', DT_in := DT#2026-04-16-12:00:01, TS := TIME#4s);
DebugEvent := STATUS_TO_ESR(status := BYTE#250, adress := 'DBG', DT_in := DT#2026-04-16-12:00:02, TS := TIME#5s);
VersionDate := OSCAT_VERSION(IN := TRUE);

ASSERT_EQUAL(EXPECTED := BYTE#1, ACTUAL := ErrorEvent.TYP);
ASSERT_EQUAL(EXPECTED := 'ERR', ACTUAL := ErrorEvent.ADRESS);
ASSERT_EQUAL(EXPECTED := BYTE#42, ACTUAL := ErrorEvent.DATA[0]);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := StatusEvent.TYP);
ASSERT_EQUAL(EXPECTED := BYTE#150, ACTUAL := StatusEvent.DATA[0]);
ASSERT_EQUAL(EXPECTED := BYTE#3, ACTUAL := DebugEvent.TYP);
ASSERT_EQUAL(EXPECTED := BYTE#250, ACTUAL := DebugEvent.DATA[0]);
ASSERT_EQUAL(EXPECTED := DWORD#333, ACTUAL := OSCAT_VERSION(IN := FALSE));
ASSERT_TRUE(IN := VersionDate > DWORD#333);
END_TEST_PROGRAM

TEST_PROGRAM oscat_esr_monitors_and_collector_behave
VAR
    BoolMonitor : ESR_MON_B8;
    RealMonitor : ESR_MON_R4;
    StatusMonitor : ESR_MON_X8;
    Collector : ESR_COLLECT;
    BoolEvents : ARRAY[0..3] OF ESR_DATA;
    RealEvents : ARRAY[0..3] OF ESR_DATA;
    StatusEvents : ARRAY[0..3] OF ESR_DATA;
    EmptyEvents : ARRAY[0..3] OF ESR_DATA;
    Collected : ARRAY[0..31] OF ESR_DATA;
    Position : INT := INT#-1;
END_VAR

BoolMonitor(
    s0 := TRUE,
    s1 := FALSE,
    s2 := TRUE,
    s3 := FALSE,
    s4 := FALSE,
    s5 := FALSE,
    s6 := FALSE,
    s7 := FALSE,
    DT_in := DT#2026-04-16-12:00:00,
    a0 := 'B0',
    a1 := 'B1',
    a2 := 'B2',
    a3 := 'B3',
    a4 := 'B4',
    a5 := 'B5',
    a6 := 'B6',
    a7 := 'B7',
    ESR_Out := BoolEvents
);
ASSERT_TRUE(IN := BoolMonitor.ESR_Flag);
ASSERT_EQUAL(EXPECTED := BYTE#11, ACTUAL := BoolEvents[0].TYP);
ASSERT_EQUAL(EXPECTED := 'B0', ACTUAL := BoolEvents[0].ADRESS);
ASSERT_EQUAL(EXPECTED := BYTE#11, ACTUAL := BoolEvents[1].TYP);
ASSERT_EQUAL(EXPECTED := 'B2', ACTUAL := BoolEvents[1].ADRESS);

RealMonitor(
    R0 := REAL#1.5,
    R1 := REAL#2.0,
    R2 := REAL#0.0,
    R3 := REAL#0.0,
    DT_in := DT#2026-04-16-12:00:00,
    a0 := 'R0',
    a1 := 'R1',
    a2 := 'R2',
    a3 := 'R3',
    s0 := REAL#0.1,
    s1 := REAL#0.1,
    s2 := REAL#0.1,
    s3 := REAL#0.1,
    ESR_Out := RealEvents
);
ASSERT_TRUE(IN := RealMonitor.ESR_Flag);
ASSERT_EQUAL(EXPECTED := BYTE#20, ACTUAL := RealEvents[0].TYP);
ASSERT_EQUAL(EXPECTED := 'R0', ACTUAL := RealEvents[0].ADRESS);
ASSERT_EQUAL(EXPECTED := BYTE#20, ACTUAL := RealEvents[1].TYP);
ASSERT_EQUAL(EXPECTED := 'R1', ACTUAL := RealEvents[1].ADRESS);

StatusMonitor(
    s0 := BYTE#50,
    s1 := BYTE#150,
    s2 := BYTE#250,
    s3 := BYTE#0,
    s4 := BYTE#0,
    s5 := BYTE#0,
    s6 := BYTE#0,
    s7 := BYTE#0,
    DT_in := DT#2026-04-16-12:00:00,
    Mode := BYTE#2,
    a0 := 'X0',
    a1 := 'X1',
    a2 := 'X2',
    a3 := 'X3',
    a4 := 'X4',
    a5 := 'X5',
    a6 := 'X6',
    a7 := 'X7',
    ESR_Out := StatusEvents
);
ASSERT_TRUE(IN := StatusMonitor.ESR_Flag);
ASSERT_EQUAL(EXPECTED := BYTE#1, ACTUAL := StatusEvents[0].TYP);
ASSERT_EQUAL(EXPECTED := BYTE#50, ACTUAL := StatusEvents[0].DATA[0]);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := StatusEvents[1].TYP);
ASSERT_EQUAL(EXPECTED := BYTE#150, ACTUAL := StatusEvents[1].DATA[0]);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := StatusEvents[2].TYP);

Collector(
    ESR_0 := BoolEvents,
    ESR_1 := RealEvents,
    ESR_2 := StatusEvents,
    ESR_3 := EmptyEvents,
    ESR_4 := EmptyEvents,
    ESR_5 := EmptyEvents,
    ESR_6 := EmptyEvents,
    ESR_7 := EmptyEvents,
    rst := FALSE,
    pos := Position
);

ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := Position);
ASSERT_EQUAL(EXPECTED := 'B0', ACTUAL := Collector.ESR_OUT[0].ADRESS);
ASSERT_EQUAL(EXPECTED := 'R0', ACTUAL := Collector.ESR_OUT[1].ADRESS);
ASSERT_EQUAL(EXPECTED := 'X0', ACTUAL := Collector.ESR_OUT[2].ADRESS);
ASSERT_EQUAL(EXPECTED := 'B2', ACTUAL := Collector.ESR_OUT[3].ADRESS);
ASSERT_EQUAL(EXPECTED := 'R1', ACTUAL := Collector.ESR_OUT[4].ADRESS);
ASSERT_EQUAL(EXPECTED := 'X1', ACTUAL := Collector.ESR_OUT[5].ADRESS);
END_TEST_PROGRAM
