TEST_PROGRAM LimitAddAndScaling
ASSERT_EQUAL(
    EXPECTED := INT#7,
    ACTUAL := LimitAdd(AddendA := INT#3, AddendB := INT#4, MaxValue := INT#10)
);
ASSERT_EQUAL(
    EXPECTED := INT#5,
    ACTUAL := LimitAdd(AddendA := INT#3, AddendB := INT#4, MaxValue := INT#5)
);
ASSERT_EQUAL(
    EXPECTED := INT#-2,
    ACTUAL := LimitAdd(AddendA := INT#-3, AddendB := INT#1, MaxValue := INT#100)
);
(* REAL assertions should use tolerance for stable behavior. *)
ASSERT_NEAR(
    EXPECTED := REAL#37.5,
    ACTUAL := ScaleRawToPercent(Raw := INT#375),
    DELTA := REAL#0.01
);
ASSERT_TRUE(IN := LimitAdd(AddendA := INT#2, AddendB := INT#2, MaxValue := INT#5) = INT#4);
ASSERT_FALSE(IN := LimitAdd(AddendA := INT#2, AddendB := INT#2, MaxValue := INT#3) = INT#4);
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK StartStopSequence
VAR
    Dut : StartStopLatch;
END_VAR

(* Cycle 1: idle state, output should stay FALSE. *)
Dut(StartCmd := FALSE, StopCmd := FALSE);
ASSERT_FALSE(Dut.Running);

(* Cycle 2: start command latches Running TRUE. *)
Dut(StartCmd := TRUE, StopCmd := FALSE);
ASSERT_TRUE(Dut.Running);

(* Cycle 3: no input change keeps the latched state. *)
Dut(StartCmd := FALSE, StopCmd := FALSE);
ASSERT_TRUE(Dut.Running);

(* Cycle 4: stop command resets Running to FALSE. *)
Dut(StartCmd := FALSE, StopCmd := TRUE);
ASSERT_FALSE(Dut.Running);
END_TEST_FUNCTION_BLOCK

TEST_PROGRAM ComparisonAssertions
ASSERT_NOT_EQUAL(
    EXPECTED := INT#5,
    ACTUAL := INT#3
);
ASSERT_GREATER(
    VALUE := INT#10,
    BOUND := INT#5
);
ASSERT_LESS(
    VALUE := INT#3,
    BOUND := INT#10
);
ASSERT_GREATER_OR_EQUAL(
    VALUE := INT#5,
    BOUND := INT#5
);
ASSERT_LESS_OR_EQUAL(
    VALUE := INT#5,
    BOUND := INT#10
);
END_TEST_PROGRAM
