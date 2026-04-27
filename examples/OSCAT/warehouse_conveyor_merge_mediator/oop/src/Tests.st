TEST_PROGRAM MergeMediatorGrantsPriorityLane
VAR
    Merge : MergeMediator;
END_VAR
Merge.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := FALSE, Lane3Request := TRUE, Lane3Jam := FALSE, DownstreamReady := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Merge.GrantedCount);
END_TEST_PROGRAM

TEST_PROGRAM MergeMediatorSkipsJammedLane
VAR
    Merge : MergeMediator;
END_VAR
Merge.Initialize();
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := TRUE, Lane3Request := FALSE, Lane3Jam := FALSE, DownstreamReady := TRUE));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Merge.JamCount);
END_TEST_PROGRAM

(* Multi-cycle mediator test: drives the same merge through varying
   lane requests and downstream-ready states. Catches bugs where
   GrantedCount fails to grow on subsequent grants or where the
   mediator gates on stale downstream-ready. *)
TEST_PROGRAM MergeMediatorAccumulatesGrantsAcrossScans
VAR
    Merge : MergeMediator;
END_VAR
Merge.Initialize();

(* Scan 1: lane 2 wins *)
Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := FALSE, Lane3Request := TRUE, Lane3Jam := FALSE, DownstreamReady := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Merge.GrantedCount);

(* Scan 2: only lane 1 requesting, lane 1 grants *)
Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := FALSE, Lane2Jam := FALSE, Lane3Request := FALSE, Lane3Jam := FALSE, DownstreamReady := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := Merge.GrantedCount);

(* Scan 3: downstream not ready, no grant even with requests *)
Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := FALSE, Lane3Request := TRUE, Lane3Jam := FALSE, DownstreamReady := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := Merge.GrantedCount);
END_TEST_PROGRAM

(* Multi-call regression: a long burst of grants accumulates one
   GrantedCount per scan when downstream stays ready. Catches edge-
   trigger bugs in the embedded DwordCounter where a missing edge
   reset would freeze the counter at 1. *)
TEST_PROGRAM MergeMediatorIncrementsCounterOncePerGrantedScan
VAR
    Merge : MergeMediator;
    I : INT;
END_VAR
Merge.Initialize();
FOR I := 1 TO 10 DO
    Merge.Update(Lane1Request := FALSE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := FALSE, Lane3Request := FALSE, Lane3Jam := FALSE, DownstreamReady := TRUE);
END_FOR;
ASSERT_EQUAL(EXPECTED := DWORD#10, ACTUAL := Merge.GrantedCount);
END_TEST_PROGRAM

(* Boundary test: no requests at all returns ActiveLaneId = 0 and
   does not increment GrantedCount. *)
TEST_PROGRAM MergeMediatorNoRequestsReturnsZero
VAR
    Merge : MergeMediator;
END_VAR
Merge.Initialize();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Merge.Update(Lane1Request := FALSE, Lane1Jam := FALSE, Lane2Request := FALSE, Lane2Jam := FALSE, Lane3Request := FALSE, Lane3Jam := FALSE, DownstreamReady := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := Merge.GrantedCount);
END_TEST_PROGRAM

(* Boundary test: all three lanes jammed. JamCount = 3, ActiveLaneId
   stays 0, GrantedCount unchanged. *)
TEST_PROGRAM MergeMediatorAllLanesJammedNoGrant
VAR
    Merge : MergeMediator;
END_VAR
Merge.Initialize();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Merge.Update(Lane1Request := TRUE, Lane1Jam := TRUE, Lane2Request := TRUE, Lane2Jam := TRUE, Lane3Request := TRUE, Lane3Jam := TRUE, DownstreamReady := TRUE));
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Merge.JamCount);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := Merge.GrantedCount);
END_TEST_PROGRAM

(* Per-FB isolation: drive a bare InfeedConveyor and verify its
   property surface. Each lane object should keep its own jam,
   request, and grant state without depending on a mediator. *)
TEST_PROGRAM InfeedConveyorReportsItsOwnState
VAR
    Lane : InfeedConveyor;
END_VAR

Lane.Configure(LaneId := INT#7);
ASSERT_EQUAL(EXPECTED := INT#7, ACTUAL := Lane.LaneId);
ASSERT_FALSE(IN := Lane.Requesting);
ASSERT_FALSE(IN := Lane.Jammed);
ASSERT_FALSE(IN := Lane.Granted);

Lane.UpdateLane(Request := TRUE, Jam := FALSE);
ASSERT_TRUE(IN := Lane.Requesting);
ASSERT_FALSE(IN := Lane.Jammed);

Lane.Grant(Enable := TRUE);
ASSERT_TRUE(IN := Lane.Granted);

Lane.UpdateLane(Request := TRUE, Jam := TRUE);
ASSERT_TRUE(IN := Lane.Jammed);
END_TEST_PROGRAM

(* Edge input: lane 2 requesting but jammed, lane 3 requesting and
   healthy — lane 3 wins because lane 2 is skipped and lane 1 is not
   requesting. *)
TEST_PROGRAM MergeMediatorJammedHighPriorityFallsThroughToLowestLane
VAR
    Merge : MergeMediator;
END_VAR
Merge.Initialize();
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Merge.Update(Lane1Request := FALSE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := TRUE, Lane3Request := TRUE, Lane3Jam := FALSE, DownstreamReady := TRUE));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Merge.JamCount);
END_TEST_PROGRAM
