(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM DeadbandIgnoresSmallRequests
(* With a 1-point deadband, 75.5 % leaves the accepted 75 % target; 76 % is accepted. *)
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := AcceptedTarget(Request := LREAL#75.5, LastAccepted := LREAL#75.0, Deadband := LREAL#1.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#76.0, ACTUAL := AcceptedTarget(Request := LREAL#76.0, LastAccepted := LREAL#75.0, Deadband := LREAL#1.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM StictionBreaksAwayAndStopsShort
(* Stem at 75 %, target 80.5 %: F = 10 · 5.5 = 55 N exceeds the 50 N breakaway force. The stem slides
   until F falls to the 20 N moving friction, 2 points short of the target, at 78.5 %. *)
ASSERT_NEAR(EXPECTED := LREAL#55.0, ACTUAL := DriveForce(Stiffness := LREAL#10.0, Target := LREAL#80.5, Stem := LREAL#75.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#78.5, ACTUAL := SlidingStop(Target := LREAL#80.5, Force := LREAL#55.0,
    MovingFriction := LREAL#20.0, Stiffness := LREAL#10.0), DELTA := LREAL#1.0e-12);
(* Reversal to 72 %: F = -65 N breaks away; the stem stops at 74 %. *)
ASSERT_NEAR(EXPECTED := LREAL#-65.0, ACTUAL := DriveForce(Stiffness := LREAL#10.0, Target := LREAL#72.0, Stem := LREAL#78.5), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#74.0, ACTUAL := SlidingStop(Target := LREAL#72.0, Force := LREAL#-65.0,
    MovingFriction := LREAL#20.0, Stiffness := LREAL#10.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
