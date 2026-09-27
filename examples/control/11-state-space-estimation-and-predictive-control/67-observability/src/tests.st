(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson69DownstreamHistoryAndNoiseScale
VAR
    Reconstructed : LREAL;
    NoiseError : LREAL;
END_VAR
Reconstructed := UpstreamFromDownstreamHistory(MeasuredDeviationM := LREAL#0.0, SlopeMPerS := LREAL#0.0008);
NoiseError := UpstreamFromDownstreamHistory(MeasuredDeviationM := LREAL#0.005, SlopeMPerS := LREAL#0.010);
ASSERT_NEAR(Reconstructed, LREAL#0.020, LREAL#0.000000001);
ASSERT_NEAR(NoiseError, LREAL#0.255, LREAL#0.000000001);
END_TEST_PROGRAM
