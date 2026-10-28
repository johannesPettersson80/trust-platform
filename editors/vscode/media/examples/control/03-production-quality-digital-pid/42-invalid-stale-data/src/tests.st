(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM OldReadingsAreStale
(* With a 0.5 s limit, 0.5 s passes and 0.6 s or 0.7 s fails, even if the reading still says 35 °C. *)
ASSERT_TRUE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#0.5, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#0.6, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#0.7, MaximumAge := LREAL#0.5));
(* A reading stamped in the future has a negative age and is unusable as well. *)
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#-0.1, MaximumAge := LREAL#0.5));
END_TEST_PROGRAM

TEST_PROGRAM InvalidReadingsAreRejected
(* A fresh reading still fails with bad quality or outside the sensor's 0–80 °C range. *)
ASSERT_FALSE(ReadingUsable(QualityGood := FALSE, Value := LREAL#35.0, Age := LREAL#0.0, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#85.0, Age := LREAL#0.0, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#-1.0, Age := LREAL#0.0, MaximumAge := LREAL#0.5));
ASSERT_TRUE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#0.0, MaximumAge := LREAL#0.5));
END_TEST_PROGRAM
