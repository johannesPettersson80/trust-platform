TEST_PROGRAM ClassicMergeSkipsJammedPriorityLane
VAR
    Merge : ClassicMerge;
END_VAR
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := TRUE, Lane3Request := FALSE, Lane3Jam := FALSE, DownstreamReady := TRUE));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Merge.JamCount);
END_TEST_PROGRAM
