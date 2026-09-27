(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson84ReceiverPhysicsTuningAndScheduleAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson84ReceiverPressureRate(
        ReceiverRateGain := LREAL#0.005,
        CompressorDelivery := LREAL#50.0,
        PlantDemand := LREAL#50.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.075,
    ACTUAL := Lesson84ReceiverPressureRate(
        ReceiverRateGain := LREAL#0.005,
        CompressorDelivery := LREAL#50.0,
        PlantDemand := LREAL#65.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#3.0,
    ACTUAL := Lesson84CompressorDeliveryRate(
        DeliveryRequest := LREAL#65.0,
        ActualDelivery := LREAL#50.0,
        DeliveryTimeConstant := LREAL#5.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.0,
    ACTUAL := Lesson84DemandFeedforward(
        MeasuredDemand := LREAL#65.0,
        OperatingDemand := LREAL#50.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#40.0,
    ACTUAL := Lesson84PiProportionalGain(
        CharacteristicLinearCoefficient := LREAL#0.04,
        CompressorTimeConstant := LREAL#5.0,
        ReceiverRateGain := LREAL#0.005),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson84PiIntegralGain(
        CharacteristicConstantCoefficient := LREAL#0.001,
        CompressorTimeConstant := LREAL#5.0,
        ReceiverRateGain := LREAL#0.005),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#50.0,
    ACTUAL := Lesson84ReviewedDemand(ExperimentScan := DINT#60),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#65.0,
    ACTUAL := Lesson84ReviewedDemand(ExperimentScan := DINT#61),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#55.0,
    ACTUAL := Lesson84ReviewedDemand(ExperimentScan := DINT#181),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
