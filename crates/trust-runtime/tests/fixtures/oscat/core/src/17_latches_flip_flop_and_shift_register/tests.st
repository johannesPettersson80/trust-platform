TEST_PROGRAM oscat_logic_pulse_latches_and_store_behave
VAR
    Latch : LTCH;
    Latch4 : LTCH_4;
    Store : STORE_8;
END_VAR

Latch(DATA := TRUE, L := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Latch.Q);

Latch(DATA := FALSE, L := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Latch.Q);

Latch(DATA := FALSE, L := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Latch.Q);

Latch(DATA := TRUE, L := TRUE, RST := TRUE);
ASSERT_FALSE(IN := Latch.Q);

Latch4(D0 := TRUE, D1 := FALSE, D2 := TRUE, D3 := FALSE, L := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Latch4.Q0);
ASSERT_FALSE(IN := Latch4.Q1);
ASSERT_TRUE(IN := Latch4.Q2);
ASSERT_FALSE(IN := Latch4.Q3);

Latch4(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, L := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Latch4.Q0);
ASSERT_FALSE(IN := Latch4.Q1);
ASSERT_TRUE(IN := Latch4.Q2);
ASSERT_FALSE(IN := Latch4.Q3);

Latch4(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, L := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Latch4.Q0);
ASSERT_TRUE(IN := Latch4.Q1);
ASSERT_FALSE(IN := Latch4.Q2);
ASSERT_TRUE(IN := Latch4.Q3);

Latch4(D0 := FALSE, D1 := FALSE, D2 := FALSE, D3 := FALSE, L := TRUE, RST := TRUE);
ASSERT_FALSE(IN := Latch4.Q0);
ASSERT_FALSE(IN := Latch4.Q1);
ASSERT_FALSE(IN := Latch4.Q2);
ASSERT_FALSE(IN := Latch4.Q3);

Store(
    SetIn := FALSE,
    D0 := TRUE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := TRUE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := FALSE
);
ASSERT_TRUE(IN := Store.Q0);
ASSERT_FALSE(IN := Store.Q1);
ASSERT_FALSE(IN := Store.Q2);
ASSERT_TRUE(IN := Store.Q3);

Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := TRUE,
    Rst := FALSE
);
ASSERT_FALSE(IN := Store.Q0);
ASSERT_TRUE(IN := Store.Q3);

Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := FALSE
);
Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := TRUE,
    Rst := FALSE
);
ASSERT_FALSE(IN := Store.Q0);
ASSERT_FALSE(IN := Store.Q1);
ASSERT_FALSE(IN := Store.Q2);
ASSERT_FALSE(IN := Store.Q3);

Store(
    SetIn := TRUE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := FALSE
);
ASSERT_TRUE(IN := Store.Q0);
ASSERT_TRUE(IN := Store.Q7);

Store(
    SetIn := FALSE,
    D0 := FALSE,
    D1 := FALSE,
    D2 := FALSE,
    D3 := FALSE,
    D4 := FALSE,
    D5 := FALSE,
    D6 := FALSE,
    D7 := FALSE,
    ClrIn := FALSE,
    Rst := TRUE
);
ASSERT_FALSE(IN := Store.Q0);
ASSERT_FALSE(IN := Store.Q7);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_edge_counters_and_toggle_behave
VAR
    ByteCounter : COUNT_BR;
    DWordCounter : COUNT_DR;
    ToggleFf : TOGGLE;
END_VAR

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#4, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := TRUE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#4, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := TRUE, IN := BYTE#9, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#5, ACTUAL := ByteCounter.CNT);

ByteCounter(SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, StepSize := BYTE#2, MX := BYTE#5, RST := TRUE);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ByteCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#4, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := TRUE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := TRUE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#4, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := TRUE, IN := DWORD#9, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#5, ACTUAL := DWordCounter.CNT);

DWordCounter(SetIn := FALSE, IN := DWORD#0, UP := FALSE, DN := FALSE, StepSize := DWORD#2, MX := DWORD#5, RST := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := DWordCounter.CNT);

ToggleFf(CLK := FALSE, RST := FALSE);
ASSERT_FALSE(IN := ToggleFf.Q);

ToggleFf(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := ToggleFf.Q);

ToggleFf(CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := ToggleFf.Q);

ToggleFf(CLK := FALSE, RST := FALSE);
ToggleFf(CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := ToggleFf.Q);

ToggleFf(CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := ToggleFf.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_edge_flip_flops_behave
VAR
    DualD : FF_D2E;
    QuadD : FF_D4E;
    DWithSetReset : FF_DRE;
END_VAR

DualD(D0 := TRUE, D1 := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DualD.Q0);
ASSERT_FALSE(IN := DualD.Q1);

DualD(D0 := FALSE, D1 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DualD.Q0);
ASSERT_FALSE(IN := DualD.Q1);

DualD(D0 := FALSE, D1 := TRUE, CLK := FALSE, RST := FALSE);
DualD(D0 := FALSE, D1 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := DualD.Q0);
ASSERT_TRUE(IN := DualD.Q1);

DualD(D0 := FALSE, D1 := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := DualD.Q0);
ASSERT_FALSE(IN := DualD.Q1);

QuadD(D0 := TRUE, D1 := FALSE, D2 := TRUE, D3 := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := QuadD.Q0);
ASSERT_FALSE(IN := QuadD.Q1);
ASSERT_TRUE(IN := QuadD.Q2);
ASSERT_FALSE(IN := QuadD.Q3);

QuadD(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, CLK := FALSE, RST := FALSE);
QuadD(D0 := FALSE, D1 := TRUE, D2 := FALSE, D3 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := QuadD.Q0);
ASSERT_TRUE(IN := QuadD.Q1);
ASSERT_FALSE(IN := QuadD.Q2);
ASSERT_TRUE(IN := QuadD.Q3);

QuadD(D0 := FALSE, D1 := FALSE, D2 := FALSE, D3 := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := QuadD.Q0);
ASSERT_FALSE(IN := QuadD.Q1);
ASSERT_FALSE(IN := QuadD.Q2);
ASSERT_FALSE(IN := QuadD.Q3);

DWithSetReset(SetIn := TRUE, DATA := FALSE, CLK := FALSE, RST := FALSE);
ASSERT_TRUE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, DATA := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, DATA := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, DATA := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := DWithSetReset.Q);

DWithSetReset(SetIn := FALSE, DATA := FALSE, CLK := FALSE, RST := FALSE);
DWithSetReset(SetIn := FALSE, DATA := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := DWithSetReset.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_jk_rs_and_selector_behave
VAR
    JkFf : FF_JKE;
    RsFf : FF_RSE;
    Selector : SELECT_8;
END_VAR

JkFf(SetIn := TRUE, J := FALSE, CLK := FALSE, K := FALSE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := FALSE, K := FALSE, RST := TRUE);
ASSERT_FALSE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := TRUE, CLK := TRUE, K := FALSE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := TRUE, K := FALSE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := FALSE, K := FALSE, RST := FALSE);
JkFf(SetIn := FALSE, J := FALSE, CLK := TRUE, K := TRUE, RST := FALSE);
ASSERT_FALSE(IN := JkFf.Q);

JkFf(SetIn := FALSE, J := FALSE, CLK := FALSE, K := FALSE, RST := FALSE);
JkFf(SetIn := FALSE, J := TRUE, CLK := TRUE, K := TRUE, RST := FALSE);
ASSERT_TRUE(IN := JkFf.Q);

RsFf(CS := FALSE, CR := FALSE, RST := TRUE);
ASSERT_FALSE(IN := RsFf.Q);

RsFf(CS := TRUE, CR := FALSE, RST := FALSE);
ASSERT_TRUE(IN := RsFf.Q);

RsFf(CS := TRUE, CR := FALSE, RST := FALSE);
ASSERT_TRUE(IN := RsFf.Q);

RsFf(CS := FALSE, CR := FALSE, RST := FALSE);
RsFf(CS := FALSE, CR := TRUE, RST := FALSE);
ASSERT_FALSE(IN := RsFf.Q);

RsFf(CS := FALSE, CR := FALSE, RST := FALSE);
RsFf(CS := TRUE, CR := TRUE, RST := FALSE);
ASSERT_FALSE(IN := RsFf.Q);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, RST := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q0);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q1);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := TRUE, DN := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q1);

Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, RST := FALSE);
Selector(E := TRUE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := TRUE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q0);

Selector(E := TRUE, SetIn := TRUE, IN := BYTE#6, UP := FALSE, DN := FALSE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Selector.STATE);
ASSERT_TRUE(IN := Selector.Q6);

Selector(E := FALSE, SetIn := FALSE, IN := BYTE#0, UP := FALSE, DN := FALSE, RST := FALSE);
ASSERT_FALSE(IN := Selector.Q0);
ASSERT_FALSE(IN := Selector.Q6);
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Selector.STATE);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_shift_registers_behave
VAR
    Shift4 : SHR_4E;
    Shift4Dir : SHR_4UDE;
    Shift8Load : SHR_8PLE;
    Shift8Dir : SHR_8UDE;
END_VAR

Shift4(SetIn := TRUE, D0 := FALSE, CLK := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift4.Q0);
ASSERT_TRUE(IN := Shift4.Q3);

Shift4(SetIn := FALSE, D0 := TRUE, CLK := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift4.Q0);
ASSERT_TRUE(IN := Shift4.Q1);

Shift4(SetIn := FALSE, D0 := FALSE, CLK := FALSE, RST := FALSE);
Shift4(SetIn := FALSE, D0 := FALSE, CLK := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Shift4.Q0);
ASSERT_TRUE(IN := Shift4.Q2);

Shift4(SetIn := FALSE, D0 := FALSE, CLK := FALSE, RST := TRUE);
ASSERT_FALSE(IN := Shift4.Q0);
ASSERT_FALSE(IN := Shift4.Q3);

Shift4Dir(SetIn := TRUE, D0 := FALSE, D3 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift4Dir.Q0);
ASSERT_TRUE(IN := Shift4Dir.Q3);

Shift4Dir(SetIn := FALSE, D0 := FALSE, D3 := FALSE, CLK := FALSE, DN := FALSE, RST := TRUE);
Shift4Dir(SetIn := FALSE, D0 := TRUE, D3 := FALSE, CLK := TRUE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift4Dir.Q0);
ASSERT_FALSE(IN := Shift4Dir.Q1);

Shift4Dir(SetIn := FALSE, D0 := FALSE, D3 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
Shift4Dir(SetIn := FALSE, D0 := FALSE, D3 := TRUE, CLK := TRUE, DN := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift4Dir.Q3);
ASSERT_FALSE(IN := Shift4Dir.Q0);

Shift8Load(Din := FALSE, Dload := BYTE#16#A5, CLK := FALSE, UP := TRUE, LoadIn := FALSE, RST := TRUE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#16#A5, CLK := TRUE, UP := TRUE, LoadIn := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := TRUE, LoadIn := FALSE, RST := FALSE);
Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := TRUE, UP := TRUE, LoadIn := FALSE, RST := FALSE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := FALSE, LoadIn := FALSE, RST := FALSE);
Shift8Load(Din := TRUE, Dload := BYTE#16#80, CLK := TRUE, UP := FALSE, LoadIn := TRUE, RST := FALSE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := FALSE, UP := FALSE, LoadIn := FALSE, RST := FALSE);
Shift8Load(Din := FALSE, Dload := BYTE#0, CLK := TRUE, UP := FALSE, LoadIn := FALSE, RST := FALSE);
ASSERT_FALSE(IN := Shift8Load.DOut);

Shift8Dir(SetIn := TRUE, D0 := FALSE, D7 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Dir.Q0);
ASSERT_TRUE(IN := Shift8Dir.Q7);

Shift8Dir(SetIn := FALSE, D0 := FALSE, D7 := FALSE, CLK := FALSE, DN := FALSE, RST := TRUE);
Shift8Dir(SetIn := FALSE, D0 := TRUE, D7 := FALSE, CLK := TRUE, DN := FALSE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Dir.Q0);
ASSERT_FALSE(IN := Shift8Dir.Q1);

Shift8Dir(SetIn := FALSE, D0 := FALSE, D7 := FALSE, CLK := FALSE, DN := FALSE, RST := FALSE);
Shift8Dir(SetIn := FALSE, D0 := FALSE, D7 := TRUE, CLK := TRUE, DN := TRUE, RST := FALSE);
ASSERT_TRUE(IN := Shift8Dir.Q7);
ASSERT_FALSE(IN := Shift8Dir.Q0);
END_TEST_PROGRAM

