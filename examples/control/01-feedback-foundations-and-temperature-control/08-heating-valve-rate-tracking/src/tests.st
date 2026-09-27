(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson8AppliedPowerCannotMoveFasterThanActuator
VAR
    Applied : LREAL;
END_VAR

Applied := Lesson8ReachablePosition(
    RequestedPosition := LREAL#80.0,
    PreviousPosition := LREAL#75.0,
    MaximumRate := LREAL#5.0,
    CycleTime := LREAL#0.1
);

ASSERT_TRUE(IN := Applied = LREAL#75.5);
END_TEST_PROGRAM

TEST_PROGRAM Lesson8StoredCorrectionTracksAppliedPosition
VAR
    TrackingCorrection : LREAL;
END_VAR

TrackingCorrection := Lesson8TrackingCorrection(
    RequestedPosition := LREAL#80.0,
    AppliedPosition := LREAL#75.5,
    CycleTime := LREAL#0.1,
    TrackingTime := LREAL#1.0
);

ASSERT_TRUE(IN := TrackingCorrection = LREAL#-0.45);
END_TEST_PROGRAM
