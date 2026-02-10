TEST_PROGRAM TEST_LIMIT_ADD_AND_SCALING
ASSERT_EQUAL(
    EXPECTED := INT#7,
    ACTUAL := LIMIT_ADD(A := INT#3, B := INT#4, MAX_VALUE := INT#10)
);
ASSERT_EQUAL(
    EXPECTED := INT#5,
    ACTUAL := LIMIT_ADD(A := INT#3, B := INT#4, MAX_VALUE := INT#5)
);
ASSERT_EQUAL(
    EXPECTED := INT#-2,
    ACTUAL := LIMIT_ADD(A := INT#-3, B := INT#1, MAX_VALUE := INT#100)
);
(* REAL assertions should use tolerance for stable behavior. *)
ASSERT_NEAR(
    EXPECTED := REAL#37.5,
    ACTUAL := SCALE_RAW_TO_PERCENT(RAW := INT#375),
    DELTA := REAL#0.01
);
ASSERT_TRUE(IN := LIMIT_ADD(A := INT#2, B := INT#2, MAX_VALUE := INT#5) = INT#4);
ASSERT_FALSE(IN := LIMIT_ADD(A := INT#2, B := INT#2, MAX_VALUE := INT#3) = INT#4);
END_TEST_PROGRAM

TEST_FUNCTION_BLOCK TEST_FB_START_STOP_SEQUENCE
VAR
    DUT : FB_START_STOP;
END_VAR

(* Cycle 1: idle state, output should stay FALSE. *)
DUT(StartCmd := FALSE, StopCmd := FALSE);
ASSERT_FALSE(DUT.Running);

(* Cycle 2: start command latches Running TRUE. *)
DUT(StartCmd := TRUE, StopCmd := FALSE);
ASSERT_TRUE(DUT.Running);

(* Cycle 3: no input change keeps the latched state. *)
DUT(StartCmd := FALSE, StopCmd := FALSE);
ASSERT_TRUE(DUT.Running);

(* Cycle 4: stop command resets Running to FALSE. *)
DUT(StartCmd := FALSE, StopCmd := TRUE);
ASSERT_FALSE(DUT.Running);
END_TEST_FUNCTION_BLOCK

TEST_PROGRAM TEST_COMPARISON_ASSERTIONS
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
