(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson10WeightChangesOnlyTargetPart
VAR
    FullTargetResponse : LREAL;
    HalfTargetResponse : LREAL;
    FullMeasurementResponse : LREAL;
    HalfMeasurementResponse : LREAL;
END_VAR

FullTargetResponse := Lesson10ProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#1.0,
    SetpointChange := LREAL#3.0,
    MeasurementChange := LREAL#0.0
);
HalfTargetResponse := Lesson10ProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#0.5,
    SetpointChange := LREAL#3.0,
    MeasurementChange := LREAL#0.0
);
FullMeasurementResponse := Lesson10ProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#1.0,
    SetpointChange := LREAL#0.0,
    MeasurementChange := LREAL#-0.5
);
HalfMeasurementResponse := Lesson10ProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#0.5,
    SetpointChange := LREAL#0.0,
    MeasurementChange := LREAL#-0.5
);

ASSERT_TRUE(IN := FullTargetResponse = LREAL#6.0);
ASSERT_TRUE(IN := HalfTargetResponse = LREAL#3.0);
ASSERT_TRUE(IN := FullMeasurementResponse = LREAL#1.0);
ASSERT_TRUE(IN := HalfMeasurementResponse = LREAL#1.0);
END_TEST_PROGRAM

TEST_PROGRAM Lesson10ValveStillRespectsMovementLimit
ASSERT_TRUE(IN := Lesson10ReachablePosition(
    RequestedPosition := LREAL#81.15,
    PreviousPosition := LREAL#75.0,
    MaximumRate := LREAL#5.0,
    CycleTime := LREAL#0.1
) = LREAL#75.5);
END_TEST_PROGRAM
