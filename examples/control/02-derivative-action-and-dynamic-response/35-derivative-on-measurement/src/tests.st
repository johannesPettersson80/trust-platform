(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson35SetpointWeightControlsOnlyTheCommandKick
VAR
    FullErrorRequest : LREAL;
    HalfWeightRequest : LREAL;
    MeasurementOnlyRequest : LREAL;
END_VAR
FullErrorRequest := Lesson35FirstRequestedTorque(
    ProportionalGain := LREAL#16.0,
    IntegralGain := LREAL#4.0,
    DerivativeGain := LREAL#7.0,
    DerivativeSetpointWeight := LREAL#1.0,
    SetpointChange := LREAL#0.5,
    CycleTime := LREAL#0.02
);
HalfWeightRequest := Lesson35FirstRequestedTorque(
    ProportionalGain := LREAL#16.0,
    IntegralGain := LREAL#4.0,
    DerivativeGain := LREAL#7.0,
    DerivativeSetpointWeight := LREAL#0.5,
    SetpointChange := LREAL#0.5,
    CycleTime := LREAL#0.02
);
MeasurementOnlyRequest := Lesson35FirstRequestedTorque(
    ProportionalGain := LREAL#16.0,
    IntegralGain := LREAL#4.0,
    DerivativeGain := LREAL#7.0,
    DerivativeSetpointWeight := LREAL#0.0,
    SetpointChange := LREAL#0.5,
    CycleTime := LREAL#0.02
);
ASSERT_NEAR(EXPECTED := LREAL#183.04, ACTUAL := FullErrorRequest, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#95.54, ACTUAL := HalfWeightRequest, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#8.04, ACTUAL := MeasurementOnlyRequest, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := Lesson35ClampTorque(
    RequestedTorque := FullErrorRequest,
    MinimumTorque := LREAL#-10.0,
    MaximumTorque := LREAL#10.0
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#8.04, ACTUAL := Lesson35ClampTorque(
    RequestedTorque := MeasurementOnlyRequest,
    MinimumTorque := LREAL#-10.0,
    MaximumTorque := LREAL#10.0
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
