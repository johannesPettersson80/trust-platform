(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson67LocalPredictionUsesBothOldStates
VAR
    UpstreamRate : LREAL;
    DownstreamRate : LREAL;
END_VAR
UpstreamRate := CascadeUpstreamRate(StateM := LREAL#0.02, FlowDeviationM3PerS := LREAL#0.001);
DownstreamRate := CascadeDownstreamRate(UpstreamStateM := LREAL#0.02, DownstreamStateM := LREAL#0.0);
ASSERT_NEAR(UpstreamRate, LREAL#0.0012, LREAL#0.000000001);
ASSERT_NEAR(DownstreamRate, LREAL#0.0008, LREAL#0.000000001);
END_TEST_PROGRAM
