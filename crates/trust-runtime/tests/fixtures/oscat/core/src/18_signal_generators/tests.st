TEST_PROGRAM oscat_signal_generator_ramp_helpers_behave
VAR
    RawByte : BYTE := BYTE#10;
    RawWord : WORD := WORD#10;
    ByteRamp : _RMP_B;
    WordRamp : _RMP_W;
    ByteWrapper : RMP_B;
    WordWrapper : RMP_W;
    SoftRamp : RMP_SOFT;
END_VAR

ByteRamp(DIR := TRUE, E := FALSE, TR := T#1s, RMP := RawByte);
ByteRamp(DIR := TRUE, E := TRUE, TR := T#0ms, RMP := RawByte);
ByteRamp(DIR := TRUE, E := TRUE, TR := T#0ms, RMP := RawByte);
ASSERT_EQUAL(EXPECTED := BYTE#255, ACTUAL := RawByte);

WordRamp(DIR := TRUE, E := FALSE, TR := T#1s, RMP := RawWord);
WordRamp(DIR := TRUE, E := TRUE, TR := T#0ms, RMP := RawWord);
WordRamp(DIR := TRUE, E := TRUE, TR := T#0ms, RMP := RawWord);
ASSERT_EQUAL(EXPECTED := WORD#65535, ACTUAL := RawWord);

ByteWrapper(SET_HIGH := TRUE, PT := T#0ms, E := TRUE, UP := TRUE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := BYTE#255, ACTUAL := ByteWrapper.OUT);
ASSERT_TRUE(IN := ByteWrapper.HIGH);
ASSERT_FALSE(IN := ByteWrapper.LOW);
ASSERT_FALSE(IN := ByteWrapper.BUSY);

ByteWrapper(SET_HIGH := FALSE, PT := T#0ms, E := TRUE, UP := TRUE, RST := TRUE);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := ByteWrapper.OUT);
ASSERT_FALSE(IN := ByteWrapper.HIGH);
ASSERT_TRUE(IN := ByteWrapper.LOW);

WordWrapper(SET_HIGH := TRUE, PT := T#0ms, E := TRUE, UP := TRUE, RST := FALSE);
ASSERT_EQUAL(EXPECTED := WORD#65535, ACTUAL := WordWrapper.OUT);
ASSERT_TRUE(IN := WordWrapper.HIGH);

SoftRamp(IN := TRUE, VAL := BYTE#120, PT_ON := T#0ms, PT_OFF := T#0ms);
SoftRamp(IN := TRUE, VAL := BYTE#120, PT_ON := T#0ms, PT_OFF := T#0ms);
ASSERT_EQUAL(EXPECTED := BYTE#120, ACTUAL := SoftRamp.OUT);

SoftRamp(IN := FALSE, VAL := BYTE#120, PT_ON := T#0ms, PT_OFF := T#0ms);
SoftRamp(IN := FALSE, VAL := BYTE#120, PT_ON := T#0ms, PT_OFF := T#0ms);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := SoftRamp.OUT);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_generator_waveform_edge_cases_behave
VAR
    Pulse : GEN_PULSE;
    PulseWidth : GEN_PW2;
    RampWave : GEN_RMP;
    SineWave : GEN_SIN;
    SquareWave : GEN_SQR;
END_VAR

Pulse(ENQ := TRUE, PTH := T#0ms, PTL := T#0ms);
ASSERT_TRUE(IN := Pulse.Q);
Pulse(ENQ := TRUE, PTH := T#0ms, PTL := T#0ms);
ASSERT_FALSE(IN := Pulse.Q);

PulseWidth(ENQ := TRUE, TH1 := T#0ms, TL1 := T#0ms, TH2 := T#0ms, TL2 := T#0ms, TS := FALSE);
ASSERT_TRUE(IN := PulseWidth.Q);
PulseWidth(ENQ := TRUE, TH1 := T#0ms, TL1 := T#0ms, TH2 := T#0ms, TL2 := T#0ms, TS := FALSE);
ASSERT_FALSE(IN := PulseWidth.Q);

RampWave(PT := T#0ms, AM := REAL#2.0, OS := REAL#5.0, DL := REAL#0.0);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := RampWave.OUT, DELTA := REAL#0.0001);
ASSERT_FALSE(IN := RampWave.Q);

SineWave(PT := T#0ms, AM := REAL#2.0, OS := REAL#3.0, DL := REAL#0.0);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := SineWave.OUT, DELTA := REAL#0.0001);
ASSERT_TRUE(IN := SineWave.Q);

SquareWave(PT := T#1s, AM := REAL#4.0, OS := REAL#1.0, DC := REAL#0.0, DL := REAL#0.0);
ASSERT_NEAR(EXPECTED := REAL#-1.0, ACTUAL := SquareWave.OUT, DELTA := REAL#0.0001);
ASSERT_FALSE(IN := SquareWave.Q);

SquareWave(PT := T#1s, AM := REAL#4.0, OS := REAL#1.0, DC := REAL#1.0, DL := REAL#0.0);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := SquareWave.OUT, DELTA := REAL#0.0001);
ASSERT_TRUE(IN := SquareWave.Q);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_generator_misc_helpers_compile_and_stabilize
VAR
    NextRampOut : BYTE := BYTE#0;
    FollowRamp : _RMP_NEXT;
    RandomWave : GEN_RDM;
    RandomPulse : GEN_RDT;
    DutyPwm : PWM_DC;
    PulseWidthPwm : PWM_PW;
END_VAR

FollowRamp(E := TRUE, IN := BYTE#10, TR := T#0ms, TF := T#0ms, TL := T#0ms, OUT := NextRampOut);
FollowRamp(E := TRUE, IN := BYTE#10, TR := T#0ms, TF := T#0ms, TL := T#0ms, OUT := NextRampOut);
ASSERT_TRUE(IN := FollowRamp.UP OR FollowRamp.DN OR (NextRampOut >= BYTE#10));

RandomWave(PT := T#0ms, AM := REAL#1.0, OS := REAL#0.0);
ASSERT_TRUE(IN := RandomWave.Q);

RandomPulse(Enable := TRUE, Min_Time_ms := T#0ms, Max_Time_ms := T#0ms, TP_Q := T#0ms);
ASSERT_TRUE(IN := TRUE);

DutyPwm(F := REAL#0.0, DC := REAL#0.5);
ASSERT_FALSE(IN := DutyPwm.Q);

PulseWidthPwm(F := REAL#0.0, PW := T#10ms);
ASSERT_FALSE(IN := PulseWidthPwm.Q);
END_TEST_PROGRAM
