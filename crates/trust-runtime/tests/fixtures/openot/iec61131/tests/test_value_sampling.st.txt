FUNCTION_BLOCK OPENOT_TestValueSampling
VAR_OUTPUT
    Passed : BOOL;
    PeriodicInitialDelta : ULINT;
    PeriodicEarlyDelta : ULINT;
    PeriodicIntervalDelta : ULINT;
    PeriodicChangeDelta : ULINT;
    HysteresisInitialDelta : ULINT;
    HysteresisInsideDelta : ULINT;
    HysteresisUpCrossDelta : ULINT;
    HysteresisInsideAfterCenterDelta : ULINT;
    HysteresisDownCrossDelta : ULINT;
    DeadbandInitialDelta : ULINT;
    DeadbandInsideDelta : ULINT;
    DeadbandCrossDelta : ULINT;
    OnChangeInitialDelta : ULINT;
    OnChangeSameDelta : ULINT;
    OnChangeChangeDelta : ULINT;
END_VAR
VAR
    Producer : OPENOT_Producer;
    BeforeCount : ULINT;
    BaseTime : ULINT := ULINT#1800000000000000000;
END_VAR

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3001,
    ValueReal := REAL#10.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#1,
    SamplingIntervalMs := ULINT#100,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime);
Producer(Execute := FALSE, Op := UINT#6);
PeriodicInitialDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3001,
    ValueReal := REAL#10.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#1,
    SamplingIntervalMs := ULINT#100,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#50000000);
Producer(Execute := FALSE, Op := UINT#6);
PeriodicEarlyDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3001,
    ValueReal := REAL#10.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#1,
    SamplingIntervalMs := ULINT#100,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#100000000);
Producer(Execute := FALSE, Op := UINT#6);
PeriodicIntervalDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3001,
    ValueReal := REAL#11.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#1,
    SamplingIntervalMs := ULINT#100,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#120000000);
Producer(Execute := FALSE, Op := UINT#6);
PeriodicChangeDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3002,
    ValueReal := REAL#10.0,
    DeadbandReal := REAL#1.0,
    SamplingMode := UINT#2,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#200000000);
Producer(Execute := FALSE, Op := UINT#6);
HysteresisInitialDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3002,
    ValueReal := REAL#10.5,
    DeadbandReal := REAL#1.0,
    SamplingMode := UINT#2,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#210000000);
Producer(Execute := FALSE, Op := UINT#6);
HysteresisInsideDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3002,
    ValueReal := REAL#11.1,
    DeadbandReal := REAL#1.0,
    SamplingMode := UINT#2,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#220000000);
Producer(Execute := FALSE, Op := UINT#6);
HysteresisUpCrossDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3002,
    ValueReal := REAL#10.2,
    DeadbandReal := REAL#1.0,
    SamplingMode := UINT#2,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#230000000);
Producer(Execute := FALSE, Op := UINT#6);
HysteresisInsideAfterCenterDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3002,
    ValueReal := REAL#10.0,
    DeadbandReal := REAL#1.0,
    SamplingMode := UINT#2,
    UseSourceTimeInput := TRUE,
    SourceTimeInput := BaseTime + ULINT#240000000);
Producer(Execute := FALSE, Op := UINT#6);
HysteresisDownCrossDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3003,
    ValueReal := REAL#5.0,
    DeadbandReal := REAL#0.5,
    SamplingMode := UINT#0,
    SamplingIntervalMs := ULINT#0);
Producer(Execute := FALSE, Op := UINT#6);
DeadbandInitialDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3003,
    ValueReal := REAL#5.4,
    DeadbandReal := REAL#0.5,
    SamplingMode := UINT#0,
    SamplingIntervalMs := ULINT#0);
Producer(Execute := FALSE, Op := UINT#6);
DeadbandInsideDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3003,
    ValueReal := REAL#5.6,
    DeadbandReal := REAL#0.5,
    SamplingMode := UINT#0,
    SamplingIntervalMs := ULINT#0);
Producer(Execute := FALSE, Op := UINT#6);
DeadbandCrossDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3004,
    ValueReal := REAL#1.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#0,
    SamplingIntervalMs := ULINT#0);
Producer(Execute := FALSE, Op := UINT#6);
OnChangeInitialDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3004,
    ValueReal := REAL#1.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#0,
    SamplingIntervalMs := ULINT#0);
Producer(Execute := FALSE, Op := UINT#6);
OnChangeSameDelta := Producer.PublishedRecordCount - BeforeCount;

BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#3004,
    ValueReal := REAL#2.0,
    DeadbandReal := REAL#0.0,
    SamplingMode := UINT#0,
    SamplingIntervalMs := ULINT#0);
Producer(Execute := FALSE, Op := UINT#6);
OnChangeChangeDelta := Producer.PublishedRecordCount - BeforeCount;

Passed :=
    (PeriodicInitialDelta = ULINT#1) AND
    (PeriodicEarlyDelta = ULINT#0) AND
    (PeriodicIntervalDelta = ULINT#1) AND
    (PeriodicChangeDelta = ULINT#1) AND
    (HysteresisInitialDelta = ULINT#1) AND
    (HysteresisInsideDelta = ULINT#0) AND
    (HysteresisUpCrossDelta = ULINT#1) AND
    (HysteresisInsideAfterCenterDelta = ULINT#0) AND
    (HysteresisDownCrossDelta = ULINT#1) AND
    (DeadbandInitialDelta = ULINT#1) AND
    (DeadbandInsideDelta = ULINT#0) AND
    (DeadbandCrossDelta = ULINT#1) AND
    (OnChangeInitialDelta = ULINT#1) AND
    (OnChangeSameDelta = ULINT#0) AND
    (OnChangeChangeDelta = ULINT#1);
END_FUNCTION_BLOCK
