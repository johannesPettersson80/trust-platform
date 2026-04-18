TEST_PROGRAM oscat_array_mutation_helpers_behave
VAR
    Data : ARRAY[0..3] OF REAL;
END_VAR

Data[0] := REAL#-1.0;
Data[1] := REAL#2.0;
Data[2] := REAL#-3.0;
Data[3] := REAL#4.0;

ASSERT_TRUE(IN := _ARRAY_ABS(PT := ADR(Data), SIZE := UINT#16));
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := Data[0], DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := Data[2], DELTA := REAL#0.001);

ASSERT_TRUE(IN := _ARRAY_ADD(PT := ADR(Data), SIZE := UINT#16, X := REAL#1.5));
ASSERT_NEAR(EXPECTED := REAL#2.5, ACTUAL := Data[0], DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#5.5, ACTUAL := Data[3], DELTA := REAL#0.001);

ASSERT_TRUE(IN := _ARRAY_MUL(PT := ADR(Data), SIZE := UINT#16, X := REAL#2.0));
ASSERT_NEAR(EXPECTED := REAL#5.0, ACTUAL := Data[0], DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#11.0, ACTUAL := Data[3], DELTA := REAL#0.001);

ASSERT_TRUE(IN := _ARRAY_INIT(PT := ADR(Data), SIZE := UINT#16, INIT := REAL#7.0));
ASSERT_NEAR(EXPECTED := REAL#7.0, ACTUAL := Data[1], DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#7.0, ACTUAL := Data[3], DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_array_ordering_helpers_behave
VAR
    SortedData : ARRAY[0..4] OF REAL;
    MedianData : ARRAY[0..4] OF REAL;
    ShuffleData : ARRAY[0..3] OF REAL;
    SumBefore : REAL;
    SumAfter : REAL;
END_VAR

SortedData[0] := REAL#3.0;
SortedData[1] := REAL#1.0;
SortedData[2] := REAL#4.0;
SortedData[3] := REAL#1.5;
SortedData[4] := REAL#2.0;

ASSERT_TRUE(IN := _ARRAY_SORT(PT := ADR(SortedData), SIZE := UINT#20));
ASSERT_TRUE(IN := IS_SORTED(PT := ADR(SortedData), SIZE := UINT#20));
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := SortedData[0], DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := SortedData[4], DELTA := REAL#0.001);

MedianData[0] := REAL#12.0;
MedianData[1] := REAL#0.0;
MedianData[2] := REAL#4.0;
MedianData[3] := REAL#7.0;
MedianData[4] := REAL#1.0;
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := _ARRAY_MEDIAN(PT := ADR(MedianData), SIZE := UINT#20), DELTA := REAL#0.001);
ASSERT_TRUE(IN := IS_SORTED(PT := ADR(MedianData), SIZE := UINT#20));

ShuffleData[0] := REAL#1.0;
ShuffleData[1] := REAL#2.0;
ShuffleData[2] := REAL#3.0;
ShuffleData[3] := REAL#4.0;
SumBefore := ARRAY_SUM(PT := ADR(ShuffleData), SIZE := UINT#16);
ASSERT_TRUE(IN := _ARRAY_SHUFFLE(PT := ADR(ShuffleData), SIZE := UINT#16));
SumAfter := ARRAY_SUM(PT := ADR(ShuffleData), SIZE := UINT#16);
ASSERT_NEAR(EXPECTED := SumBefore, ACTUAL := SumAfter, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM oscat_array_statistics_helpers_behave
VAR
    AvgData : ARRAY[0..3] OF REAL;
    GeometricData : ARRAY[0..2] OF REAL;
    HarmonicData : ARRAY[0..2] OF REAL;
    TrendData : ARRAY[0..7] OF REAL;
END_VAR

AvgData[0] := REAL#1.0;
AvgData[1] := REAL#2.0;
AvgData[2] := REAL#3.0;
AvgData[3] := REAL#4.0;
ASSERT_NEAR(EXPECTED := REAL#2.5, ACTUAL := ARRAY_AVG(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := ARRAY_MAX(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := ARRAY_MIN(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#3.0, ACTUAL := ARRAY_SPR(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := ARRAY_SUM(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#1.666667, ACTUAL := ARRAY_VAR(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.002);
ASSERT_NEAR(EXPECTED := REAL#1.290994, ACTUAL := ARRAY_SDV(PT := ADR(AvgData), SIZE := UINT#16), DELTA := REAL#0.002);

GeometricData[0] := REAL#1.0;
GeometricData[1] := REAL#4.0;
GeometricData[2] := REAL#16.0;
ASSERT_NEAR(EXPECTED := REAL#4.0, ACTUAL := ARRAY_GAV(PT := ADR(GeometricData), SIZE := UINT#12), DELTA := REAL#0.002);

HarmonicData[0] := REAL#1.0;
HarmonicData[1] := REAL#2.0;
HarmonicData[2] := REAL#4.0;
ASSERT_NEAR(EXPECTED := REAL#1.714286, ACTUAL := ARRAY_HAV(PT := ADR(HarmonicData), SIZE := UINT#12), DELTA := REAL#0.002);

TrendData[0] := REAL#0.0;
TrendData[1] := REAL#1.0;
TrendData[2] := REAL#4.0;
TrendData[3] := REAL#5.0;
TrendData[4] := REAL#3.0;
TrendData[5] := REAL#4.0;
TrendData[6] := REAL#6.0;
TrendData[7] := REAL#3.0;
ASSERT_NEAR(EXPECTED := REAL#1.5, ACTUAL := ARRAY_TREND(PT := ADR(TrendData), SIZE := UINT#32), DELTA := REAL#0.002);
END_TEST_PROGRAM
