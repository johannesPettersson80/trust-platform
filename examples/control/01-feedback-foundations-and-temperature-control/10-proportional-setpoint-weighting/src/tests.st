(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM WeightChangesOnlyTheSetpointPart
VAR
    FullTargetResponse : LREAL;
    HalfTargetResponse : LREAL;
    FullMeasurementResponse : LREAL;
    HalfMeasurementResponse : LREAL;
END_VAR

FullTargetResponse := WeightedProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#1.0,
    SetpointChange := LREAL#3.0,
    MeasurementChange := LREAL#0.0
);
HalfTargetResponse := WeightedProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#0.5,
    SetpointChange := LREAL#3.0,
    MeasurementChange := LREAL#0.0
);
FullMeasurementResponse := WeightedProportionalChange(
    ProportionalGain := LREAL#2.0,
    SetpointWeight := LREAL#1.0,
    SetpointChange := LREAL#0.0,
    MeasurementChange := LREAL#-0.5
);
HalfMeasurementResponse := WeightedProportionalChange(
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
