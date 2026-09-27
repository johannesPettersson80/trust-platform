(* @trustlsp:runtime-ignore — run with trust-dev test, not as PLC instances. *)
TEST_PROGRAM Lesson3HysteresisRetainsStateInsideBand
ASSERT_TRUE(IN := ColdStorageHysteresisRequest(TRUE, LREAL#2.0, LREAL#2.0, LREAL#1.0));
ASSERT_FALSE(IN := ColdStorageHysteresisRequest(FALSE, LREAL#2.0, LREAL#2.0, LREAL#1.0));
ASSERT_TRUE(IN := ColdStorageHysteresisRequest(FALSE, LREAL#2.5, LREAL#2.0, LREAL#1.0));
ASSERT_FALSE(IN := ColdStorageHysteresisRequest(TRUE, LREAL#1.5, LREAL#2.0, LREAL#1.0));
END_TEST_PROGRAM

TEST_PROGRAM Lesson3DwellRequiresCompletedMinimumTime
ASSERT_FALSE(IN := ColdStorageDwellComplete(TRUE, LREAL#7.9, LREAL#8.0, LREAL#8.0));
ASSERT_TRUE(IN := ColdStorageDwellComplete(TRUE, LREAL#8.0, LREAL#8.0, LREAL#8.0));
ASSERT_FALSE(IN := ColdStorageDwellComplete(FALSE, LREAL#7.9, LREAL#8.0, LREAL#8.0));
ASSERT_TRUE(IN := ColdStorageDwellComplete(FALSE, LREAL#8.0, LREAL#8.0, LREAL#8.0));
END_TEST_PROGRAM
