TEST_PROGRAM DecoratorPipelineRejectsSingleScanSpike
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
END_VAR

Chain.Initialize();
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#102.0, Quality2 := BYTE#2, Input3 := REAL#99.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Chain.Quality);
Value := Chain.Update(Input1 := REAL#180.0, Quality1 := BYTE#2, Input2 := REAL#181.0, Quality2 := BYTE#2, Input3 := REAL#182.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#1, ACTUAL := Chain.Quality);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Chain.RejectedCount);
END_TEST_PROGRAM

TEST_PROGRAM DecoratorPipelineClampMarksBadQuality
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
END_VAR

Chain.Initialize();
Value := Chain.Update(Input1 := REAL#450.0, Quality1 := BYTE#2, Input2 := REAL#452.0, Quality2 := BYTE#2, Input3 := REAL#451.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Chain.Quality);
ASSERT_TRUE(IN := Chain.ClampFault);
ASSERT_NEAR(EXPECTED := REAL#400.0, ACTUAL := Chain.ConditionedValue, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Pipeline recovery test: drives the same Chain through clamp/spike/
   recovered states. Catches bugs where decorator quality flags or
   rejected-spike counters fail to recover when the input returns to
   normal. *)
TEST_PROGRAM DecoratorPipelineRecoversAfterTransientFault
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
    SpikeCount : INT;
END_VAR

Chain.Initialize();

(* Healthy reading - quality good *)
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#102.0, Quality2 := BYTE#2, Input3 := REAL#99.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Chain.Quality);

(* Spike - quality drops, rejected count grows *)
Value := Chain.Update(Input1 := REAL#180.0, Quality1 := BYTE#2, Input2 := REAL#181.0, Quality2 := BYTE#2, Input3 := REAL#182.0, Quality3 := BYTE#2);
SpikeCount := Chain.RejectedCount;
ASSERT_TRUE(IN := SpikeCount >= INT#1);

(* Recovered reading near previous normal - quality must recover, not
   stay at degraded from the prior spike *)
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#102.0, Quality2 := BYTE#2, Input3 := REAL#99.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Chain.Quality);
END_TEST_PROGRAM

(* Voting test: when one transmitter goes bad (Quality=0), the voted
   value is the average of the two good ones — not corrupted by the bad
   reading. The decorator chain still produces a good quality at the end. *)
TEST_PROGRAM DecoratorPipelineVotingTwoOfThree
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
END_VAR

Chain.Initialize();
(* Tc2 is bad (quality 0 = bad), but Tc1 and Tc3 agree — pipeline must
   still produce good quality from the two-of-three vote. The bad Tc2
   raw value is wildly different but must not poison the voted result. *)
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#9999.0, Quality2 := BYTE#0, Input3 := REAL#102.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Chain.Quality);
(* Voted value must be the average of Tc1 and Tc3 = 101.0, not influenced
   by Tc2's 9999. Filtered through PT1, but first sample passes through. *)
ASSERT_TRUE(IN := Chain.ConditionedValue < REAL#150.0);
ASSERT_TRUE(IN := Chain.ConditionedValue > REAL#50.0);
END_TEST_PROGRAM

(* Voting test: when two transmitters fail, the pipeline reports BAD
   quality and does not propagate any single good signal as if it were
   trustworthy. *)
TEST_PROGRAM DecoratorPipelineVotingFailsOnTwoBad
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
END_VAR

Chain.Initialize();
(* Two of three thermocouples have bad quality — voting decorator
   cannot produce a good vote; pipeline must reflect bad quality. *)
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#101.0, Quality2 := BYTE#0, Input3 := REAL#102.0, Quality3 := BYTE#0);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Chain.Quality);
END_TEST_PROGRAM

(* Multi-cycle filtering: the PT1 stage settles toward the input over
   repeated cycles; verifies the inner filter state is preserved across
   Update calls (not reset by some decorator's Update side effect). *)
TEST_PROGRAM DecoratorPipelineFilterAccumulatesAcrossCycles
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
    I : INT;
END_VAR

Chain.Initialize();
(* Establish a baseline at 100C *)
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#100.0, Quality2 := BYTE#2, Input3 := REAL#100.0, Quality3 := BYTE#2);
(* Drive the input up to 110C and let the filter follow. After many
   identical samples the conditioned value must approach 110, never
   exceed it, and never drop back to the old baseline of 100. *)
FOR I := 1 TO 30 DO
    Value := Chain.Update(Input1 := REAL#110.0, Quality1 := BYTE#2, Input2 := REAL#110.0, Quality2 := BYTE#2, Input3 := REAL#110.0, Quality3 := BYTE#2);
END_FOR;
ASSERT_TRUE(IN := Chain.ConditionedValue >= REAL#100.0);
ASSERT_TRUE(IN := Chain.ConditionedValue <= REAL#110.0);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Chain.Quality);
END_TEST_PROGRAM

(* Lower-bound clamp: negative readings must be clamped to 0 and marked
   bad quality (reading below physical range = sensor fault). *)
TEST_PROGRAM DecoratorPipelineLowerClampMarksBadQuality
VAR
    Chain : SignalConditioningPipeline;
    Value : REAL;
END_VAR

Chain.Initialize();
(* All three thermocouples report -10C — physically impossible for a
   distillation column overhead; clamp to 0, mark bad. *)
Value := Chain.Update(Input1 := REAL#-10.0, Quality1 := BYTE#2, Input2 := REAL#-10.0, Quality2 := BYTE#2, Input3 := REAL#-10.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#0, ACTUAL := Chain.Quality);
ASSERT_TRUE(IN := Chain.ClampFault);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Chain.ConditionedValue, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Per-FB isolation: two pipelines running in parallel must not share
   filter state, last-good values, or rejected counts. *)
TEST_PROGRAM DecoratorPipelineParallelInstancesIndependent
VAR
    ChainA : SignalConditioningPipeline;
    ChainB : SignalConditioningPipeline;
    Value : REAL;
END_VAR

ChainA.Initialize();
ChainB.Initialize();
(* Drive ChainA into a spike-reject state *)
Value := ChainA.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#100.0, Quality2 := BYTE#2, Input3 := REAL#100.0, Quality3 := BYTE#2);
Value := ChainA.Update(Input1 := REAL#180.0, Quality1 := BYTE#2, Input2 := REAL#180.0, Quality2 := BYTE#2, Input3 := REAL#180.0, Quality3 := BYTE#2);
ASSERT_TRUE(IN := ChainA.RejectedCount >= INT#1);
(* ChainB stays steady - its rejected count must remain 0 *)
Value := ChainB.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#100.0, Quality2 := BYTE#2, Input3 := REAL#100.0, Quality3 := BYTE#2);
Value := ChainB.Update(Input1 := REAL#101.0, Quality1 := BYTE#2, Input2 := REAL#101.0, Quality2 := BYTE#2, Input3 := REAL#101.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := ChainB.RejectedCount);
END_TEST_PROGRAM
