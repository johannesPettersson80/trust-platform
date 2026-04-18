TEST_PROGRAM oscat_signal_processing_scale_and_range_helpers_behave
ASSERT_NEAR(
    EXPECTED := REAL#11.0,
    ACTUAL := SCALE(X := REAL#5.0, K := REAL#2.0, O := REAL#1.0, MX := REAL#20.0, MN := REAL#0.0),
    DELTA := REAL#0.001
);
ASSERT_NEAR(
    EXPECTED := REAL#20.0,
    ACTUAL := SCALE(X := REAL#15.0, K := REAL#2.0, O := REAL#1.0, MX := REAL#20.0, MN := REAL#0.0),
    DELTA := REAL#0.001
);
ASSERT_NEAR(
    EXPECTED := REAL#100.0,
    ACTUAL := BYTE_TO_RANGE(X := BYTE#16#FF, low := REAL#0.0, high := REAL#100.0),
    DELTA := REAL#0.001
);
ASSERT_NEAR(
    EXPECTED := REAL#100.0,
    ACTUAL := WORD_TO_RANGE(X := WORD#16#FFFF, low := REAL#0.0, high := REAL#100.0),
    DELTA := REAL#0.001
);
ASSERT_EQUAL(
    EXPECTED := BYTE#16#7F,
    ACTUAL := RANGE_TO_BYTE(X := REAL#50.0, low := REAL#0.0, high := REAL#100.0)
);
ASSERT_EQUAL(
    EXPECTED := WORD#16#7FFF,
    ACTUAL := RANGE_TO_WORD(X := REAL#50.0, low := REAL#0.0, high := REAL#100.0)
);
END_TEST_PROGRAM

TEST_PROGRAM oscat_logic_delay_behaves
VAR
    DelayLine : DELAY;
END_VAR

DelayLine(IN := REAL#1.0, N := INT#2, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#2.0, N := INT#2, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#3.0, N := INT#2, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#4.0, N := INT#2, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);

DelayLine(IN := REAL#9.0, N := INT#2, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#9.0, ACTUAL := DelayLine.OUT, DELTA := REAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_processing_trend_helpers_behave
VAR
    TrendDWord : TREND_DW;
END_VAR

TrendDWord(X := DWORD#0);
ASSERT_FALSE(IN := TrendDWord.TU);
ASSERT_FALSE(IN := TrendDWord.TD);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := TrendDWord.D);

TrendDWord(X := DWORD#5);
ASSERT_TRUE(IN := TrendDWord.TU);
ASSERT_FALSE(IN := TrendDWord.TD);
ASSERT_TRUE(IN := TrendDWord.Q);
ASSERT_EQUAL(EXPECTED := DWORD#5, ACTUAL := TrendDWord.D);

TrendDWord(X := DWORD#2);
ASSERT_FALSE(IN := TrendDWord.TU);
ASSERT_TRUE(IN := TrendDWord.TD);
ASSERT_FALSE(IN := TrendDWord.Q);
ASSERT_EQUAL(EXPECTED := DWORD#3, ACTUAL := TrendDWord.D);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_processing_analog_scaling_helpers_behave
VAR
    Reader : AIN1;
    AnalogValue : REAL;
    DigitalValue : DWORD;
    ShiftedDigitalValue : DWORD;
END_VAR

AnalogValue := AIN(in := DWORD#4095, Bits := BYTE#12, sign := BYTE#255, low := REAL#0.0, high := REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := AnalogValue, DELTA := REAL#0.001);

Reader(in := DWORD#4095, sign_bit := INT#255, error_bit := INT#255, error_code_en := FALSE, error_code := DWORD#0, overflow_bit := INT#255, overflow_code_en := FALSE, overflow_code := DWORD#0, Bit_0 := INT#0, Bit_N := INT#11, out_min := REAL#0.0, out_max := REAL#10.0, code_min := DWORD#0, code_max := DWORD#4095, error_output := REAL#-1.0, overflow_output := REAL#99.0);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Reader.out, DELTA := REAL#0.001);
ASSERT_FALSE(IN := Reader.sign);
ASSERT_FALSE(IN := Reader.error);
ASSERT_FALSE(IN := Reader.overflow);

DigitalValue := AOUT(in := REAL#10.0, Bits := BYTE#12, sign := BYTE#255, low := REAL#0.0, high := REAL#10.0);
ASSERT_EQUAL(EXPECTED := DWORD#4095, ACTUAL := DigitalValue);

ShiftedDigitalValue := AOUT1(in := REAL#10.0, Bit_0 := INT#4, Bit_N := INT#15, sign := INT#255, low := REAL#0.0, high := REAL#10.0);
ASSERT_EQUAL(EXPECTED := DWORD#16#FFF0, ACTUAL := ShiftedDigitalValue);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_processing_delay4_and_fade_behave
VAR
    Delay4 : DELAY_4;
    Fader : FADE;
END_VAR

Delay4(in := REAL#1.0);
Delay4(in := REAL#2.0);
Delay4(in := REAL#3.0);
Delay4(in := REAL#4.0);
Delay4(in := REAL#5.0);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := Delay4.out1, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := Delay4.out2, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := Delay4.out3, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := Delay4.out4, DELTA := REAL#0.0001);

Fader(IN1 := REAL#10.0, IN2 := REAL#20.0, F := FALSE, TF := T#0ms, rst := TRUE);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Fader.Y, DELTA := REAL#0.001);

Fader(IN1 := REAL#10.0, IN2 := REAL#20.0, F := TRUE, TF := T#0ms, rst := TRUE);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := Fader.Y, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_processing_filter_and_mux_helpers_behave
VAR
    DWordFilter : FILTER_DW;
    IntFilter : FILTER_I;
    MovingAverageDWord : FILTER_MAV_DW;
    MovingAverageWord : FILTER_MAV_W;
    WordFilter : FILTER_W;
    WeightedFilter : FILTER_WAV;
    Weights : ARRAY[0..15] OF REAL;
END_VAR

Weights[0] := REAL#1.0;

DWordFilter(X := DWORD#25, T := T#0s);
ASSERT_EQUAL(EXPECTED := DWORD#25, ACTUAL := DWordFilter.Y);

IntFilter(X := INT#12, T := T#0s);
ASSERT_EQUAL(EXPECTED := INT#12, ACTUAL := IntFilter.Y);

MovingAverageDWord(X := DWORD#10, N := UINT#1, RST := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#10, ACTUAL := MovingAverageDWord.Y);
MovingAverageDWord(X := DWORD#20, N := UINT#1, RST := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#20, ACTUAL := MovingAverageDWord.Y);

MovingAverageWord(X := WORD#10, N := UINT#1, RST := TRUE);
ASSERT_EQUAL(EXPECTED := WORD#10, ACTUAL := MovingAverageWord.Y);
MovingAverageWord(X := WORD#20, N := UINT#1, RST := FALSE);
ASSERT_EQUAL(EXPECTED := WORD#20, ACTUAL := MovingAverageWord.Y);

WordFilter(X := WORD#250, T := T#0s);
ASSERT_EQUAL(EXPECTED := WORD#250, ACTUAL := WordFilter.Y);

WeightedFilter(X := REAL#2.0, W := Weights, RST := TRUE);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := WeightedFilter.Y, DELTA := REAL#0.0001);
WeightedFilter(X := REAL#5.0, W := Weights, RST := FALSE);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := WeightedFilter.Y, DELTA := REAL#0.0001);

ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := MIX(A := REAL#2.0, B := REAL#6.0, M := REAL#0.25), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#9.0, ACTUAL := MUX_R2(IN0 := REAL#1.0, IN1 := REAL#9.0, A := TRUE), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#7.0, ACTUAL := MUX_R4(IN0 := REAL#1.0, IN1 := REAL#3.0, IN2 := REAL#5.0, IN3 := REAL#7.0, A0 := TRUE, A1 := TRUE), DELTA := REAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_processing_offset_and_scaling_helpers_behave
VAR
    Value : REAL;
    Selector3 : SEL2_OF_3;
    SelectorBool : SEL2_OF_3B;
END_VAR

Value := OFFSET(REAL#1.0, TRUE, FALSE, TRUE, FALSE, FALSE, REAL#2.0, REAL#3.0, REAL#4.0, REAL#5.0, REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#7.0, ACTUAL := Value, DELTA := REAL#0.0001);

Value := OFFSET2(REAL#1.0, TRUE, FALSE, TRUE, FALSE, FALSE, REAL#2.0, REAL#3.0, REAL#4.0, REAL#5.0, REAL#10.0);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := Value, DELTA := REAL#0.0001);

ASSERT_NEAR(EXPECTED := REAL#-8.0, ACTUAL := OVERRIDE_3(X1 := REAL#3.0, X2 := REAL#-8.0, X3 := REAL#2.0, E1 := TRUE, E2 := TRUE, E3 := TRUE), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := SCALE_B(X := BYTE#255, I_LO := BYTE#0, I_HI := BYTE#255, O_LO := REAL#0.0, O_HI := REAL#10.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := SCALE_D(X := DWORD#5, I_LO := DWORD#0, I_HI := DWORD#10, O_LO := REAL#1.0, O_HI := REAL#5.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := SCALE_R(X := REAL#5.0, I_LO := REAL#0.0, I_HI := REAL#10.0, O_LO := REAL#1.0, O_HI := REAL#5.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#6.0, ACTUAL := SCALE_X2(IN1 := TRUE, IN2 := FALSE, K := REAL#1.0, O := REAL#0.0, IN1_MIN := REAL#1.0, IN1_MAX := REAL#5.0, IN2_MIN := REAL#1.0, IN2_MAX := REAL#2.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#9.0, ACTUAL := SCALE_X4(IN1 := FALSE, IN2 := FALSE, IN3 := TRUE, IN4 := TRUE, K := REAL#1.0, O := REAL#0.0, IN1_MIN := REAL#1.0, IN1_MAX := REAL#2.0, IN2_MIN := REAL#1.0, IN2_MAX := REAL#2.0, IN3_MIN := REAL#1.0, IN3_MAX := REAL#4.0, IN4_MIN := REAL#1.0, IN4_MAX := REAL#3.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#8.0, ACTUAL := SCALE_X8(IN1 := FALSE, IN2 := FALSE, IN3 := FALSE, IN4 := FALSE, IN5 := FALSE, IN6 := FALSE, IN7 := FALSE, IN8 := TRUE, K := REAL#1.0, O := REAL#0.0, IN1_MIN := REAL#0.0, IN1_MAX := REAL#1.0, IN2_MIN := REAL#0.0, IN2_MAX := REAL#1.0, IN3_MIN := REAL#0.0, IN3_MAX := REAL#1.0, IN4_MIN := REAL#0.0, IN4_MAX := REAL#1.0, IN5_MIN := REAL#0.0, IN5_MAX := REAL#1.0, IN6_MIN := REAL#0.0, IN6_MAX := REAL#1.0, IN7_MIN := REAL#0.0, IN7_MAX := REAL#1.0, IN8_MIN := REAL#0.0, IN8_MAX := REAL#8.0), DELTA := REAL#0.0001);

Selector3(IN1 := REAL#10.0, IN2 := REAL#11.0, IN3 := REAL#50.0, TOL := REAL#2.0);
ASSERT_NEAR(EXPECTED := REAL#10.5, ACTUAL := Selector3.Y, DELTA := REAL#0.0001);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Selector3.W);
ASSERT_FALSE(IN := Selector3.E);

SelectorBool(IN1 := TRUE, IN2 := TRUE, IN3 := FALSE, TD := T#0ms);
ASSERT_TRUE(IN := SelectorBool.Q);
ASSERT_TRUE(IN := SelectorBool.W);
END_TEST_PROGRAM

TEST_PROGRAM oscat_signal_processing_sample_hold_and_stair_helpers_behave
VAR
    Hold : SH;
    HoldPeriodic : SH_1;
    HoldStats : SH_2;
    Transparent : SH_T;
    Staircase : STAIR2;
    TrendReal : TREND;
END_VAR

Hold(in := REAL#5.0, CLK := TRUE);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := Hold.out, DELTA := REAL#0.0001);
ASSERT_TRUE(IN := Hold.trig);
Hold(in := REAL#7.0, CLK := TRUE);
ASSERT_FALSE(IN := Hold.trig);
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := Hold.out, DELTA := REAL#0.0001);

HoldPeriodic(in := REAL#4.0, PT := T#0ms);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := HoldPeriodic.out, DELTA := REAL#0.0001);
ASSERT_TRUE(IN := HoldPeriodic.trig);

HoldStats(in := REAL#3.0, PT := T#0ms, N := INT#3, disc := INT#0);
HoldStats(in := REAL#6.0, PT := T#0ms, N := INT#3, disc := INT#0);
HoldStats(in := REAL#9.0, PT := T#0ms, N := INT#3, disc := INT#0);
ASSERT_NEAR(EXPECTED := REAL#6.0, ACTUAL := HoldStats.avg, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := HoldStats.low, DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#9.0, ACTUAL := HoldStats.high, DELTA := REAL#0.0001);

Transparent(IN := REAL#8.0, E := TRUE);
ASSERT_NEAR(EXPECTED := REAL#8.0, ACTUAL := Transparent.OUT, DELTA := REAL#0.0001);
Transparent(IN := REAL#9.0, E := FALSE);
ASSERT_NEAR(EXPECTED := REAL#8.0, ACTUAL := Transparent.OUT, DELTA := REAL#0.0001);

ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := STAIR(X := REAL#4.9, STEP_SIZE := REAL#2.0), DELTA := REAL#0.0001);
Staircase(X := REAL#4.9, STEP_SIZE := REAL#2.0);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := Staircase.Y, DELTA := REAL#0.0001);

TrendReal(X := REAL#1.0);
TrendReal(X := REAL#2.0);
ASSERT_TRUE(IN := TrendReal.TU);
ASSERT_FALSE(IN := TrendReal.TD);
ASSERT_TRUE(IN := TrendReal.Q);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := TrendReal.D, DELTA := REAL#0.0001);
END_TEST_PROGRAM
