TEST_PROGRAM oscat_list_processing_mutation_helpers_behave
VAR
    List : STRING[LIST_LENGTH];
END_VAR

List := '';
ASSERT_TRUE(IN := LIST_ADD(SEP := BYTE#44, INS := 'alpha', LIST := List));
ASSERT_TRUE(IN := LIST_ADD(SEP := BYTE#44, INS := 'beta', LIST := List));
ASSERT_EQUAL(EXPECTED := ',alpha,beta', ACTUAL := List);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := LIST_LEN(SEP := BYTE#44, LIST := List));
ASSERT_EQUAL(EXPECTED := 'alpha', ACTUAL := LIST_GET(SEP := BYTE#44, POS := INT#1, LIST := List));
ASSERT_EQUAL(EXPECTED := 'beta', ACTUAL := LIST_GET(SEP := BYTE#44, POS := INT#2, LIST := List));

ASSERT_TRUE(IN := LIST_INSERT(SEP := BYTE#44, POS := INT#2, INS := 'mid', LIST := List));
ASSERT_EQUAL(EXPECTED := ',alpha,mid,beta', ACTUAL := List);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := LIST_LEN(SEP := BYTE#44, LIST := List));
ASSERT_EQUAL(EXPECTED := 'mid', ACTUAL := LIST_GET(SEP := BYTE#44, POS := INT#2, LIST := List));

List := ',alpha,,beta,';
ASSERT_TRUE(IN := LIST_CLEAN(SEP := BYTE#44, LIST := List));
ASSERT_EQUAL(EXPECTED := ',alpha,beta', ACTUAL := List);
END_TEST_PROGRAM

TEST_PROGRAM oscat_list_processing_iteration_and_retrieval_helpers_behave
VAR
    List : STRING[LIST_LENGTH];
    Walker : LIST_NEXT;
    Value : STRING[LIST_LENGTH];
END_VAR

List := ',alpha,mid,beta';
Walker(SEP := BYTE#44, RST := TRUE, LIST := List);
ASSERT_FALSE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := 'alpha', ACTUAL := Walker.LEL);

Walker(SEP := BYTE#44, RST := FALSE, LIST := List);
ASSERT_FALSE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := 'mid', ACTUAL := Walker.LEL);

Value := LIST_RETRIEVE(SEP := BYTE#44, POS := INT#2, LIST := List);
ASSERT_EQUAL(EXPECTED := 'mid', ACTUAL := Value);
ASSERT_EQUAL(EXPECTED := ',alpha,beta', ACTUAL := List);

Value := LIST_RETRIEVE_LAST(SEP := BYTE#44, LIST := List);
ASSERT_EQUAL(EXPECTED := 'beta', ACTUAL := Value);
ASSERT_EQUAL(EXPECTED := ',alpha', ACTUAL := List);

Walker(SEP := BYTE#44, RST := TRUE, LIST := List);
ASSERT_FALSE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := 'alpha', ACTUAL := Walker.LEL);
Walker(SEP := BYTE#44, RST := FALSE, LIST := List);
ASSERT_TRUE(IN := Walker.NUL);
ASSERT_EQUAL(EXPECTED := '', ACTUAL := Walker.LEL);
END_TEST_PROGRAM
