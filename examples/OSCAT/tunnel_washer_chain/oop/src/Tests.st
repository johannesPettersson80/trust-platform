TEST_PROGRAM WasherChainStopsAtAutoDose
VAR
    Washer : TunnelWasherQualityController;
END_VAR

Washer.Initialize();
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Washer.Evaluate(SoilLevel := REAL#80.0, Conductivity := REAL#2.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Washer.AutoDoseCount);
END_TEST_PROGRAM

TEST_PROGRAM WasherChainFallsThroughToReject
VAR
    Washer : TunnelWasherQualityController;
END_VAR

Washer.Initialize();
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#4.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Washer.RejectCount);
END_TEST_PROGRAM

(* Stale-flag regression test: drives multiple events through one
   Washer instance with different paths. Catches the bug where a
   handler keeps reporting "I handled it" after the first event
   because HandledValue is not reset on entry to Handle. *)
TEST_PROGRAM WasherChainHandlerFlagsResetBetweenEvaluations
VAR
    Washer : TunnelWasherQualityController;
END_VAR

Washer.Initialize();

(* Event 1: AutoDose path - high soil, low conductivity *)
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Washer.Evaluate(SoilLevel := REAL#80.0, Conductivity := REAL#2.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE));

(* Event 2: must fall through to RejectBatch. Without the per-call
   reset of HandledValue, this returns INT#1 because AutoDose.Handled
   is still TRUE from Event 1. *)
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#4.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE));

(* Event 3: ExtendRinse path *)
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#4.0, RinseConductivity := REAL#2.0, OperatorAvailable := FALSE));

(* Event 4: must fall through to Reject again *)
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#4.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE));

(* Counters: 1 auto-dose, 2 rejects across the four events *)
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Washer.AutoDoseCount);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := Washer.RejectCount);
END_TEST_PROGRAM

(* AutoDoseHandler in isolation: tests the AND of two conditions. *)
TEST_PROGRAM AutoDoseHandlerHandlesOnlyHighSoilLowConductivity
VAR
    Handler : AutoDoseHandler;
    Terminal : RejectBatchHandler;
    Event : WasherQualityEvent;
END_VAR
Handler.SetNext(NextHandler := Terminal);

(* Both conditions met: handles *)
Event.SoilLevel := REAL#80.0;
Event.Conductivity := REAL#2.0;
Event.RinseConductivity := REAL#0.5;
Event.OperatorAvailable := FALSE;
Handler.Handle(Event := Event);
ASSERT_TRUE(IN := Handler.Handled);

(* Soil too low: passes through *)
Event.SoilLevel := REAL#50.0;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);

(* Conductivity too high: passes through *)
Event.SoilLevel := REAL#80.0;
Event.Conductivity := REAL#5.0;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);

(* Boundary: soil exactly 70 - strict greater-than required *)
Event.SoilLevel := REAL#70.0;
Event.Conductivity := REAL#2.0;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);

(* Boundary: conductivity exactly 3 - strict less-than required *)
Event.SoilLevel := REAL#80.0;
Event.Conductivity := REAL#3.0;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);
END_TEST_PROGRAM

(* ExtendRinseHandler: triggers on RinseConductivity > 1.5. *)
TEST_PROGRAM ExtendRinseHandlerHandlesAboveRinseThreshold
VAR
    Handler : ExtendRinseHandler;
    Terminal : RejectBatchHandler;
    Event : WasherQualityEvent;
END_VAR
Handler.SetNext(NextHandler := Terminal);

(* Above threshold: handles *)
Event.RinseConductivity := REAL#2.0;
Handler.Handle(Event := Event);
ASSERT_TRUE(IN := Handler.Handled);

(* At threshold (strict greater-than): passes through *)
Event.RinseConductivity := REAL#1.5;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);

(* Below threshold: passes through *)
Event.RinseConductivity := REAL#0.5;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);
END_TEST_PROGRAM

(* OperatorHoldHandler: triggers when OperatorAvailable. *)
TEST_PROGRAM OperatorHoldHandlerHandlesWhenOperatorAvailable
VAR
    Handler : OperatorHoldHandler;
    Terminal : RejectBatchHandler;
    Event : WasherQualityEvent;
END_VAR
Handler.SetNext(NextHandler := Terminal);

Event.OperatorAvailable := TRUE;
Handler.Handle(Event := Event);
ASSERT_TRUE(IN := Handler.Handled);

Event.OperatorAvailable := FALSE;
Handler.Handle(Event := Event);
ASSERT_FALSE(IN := Handler.Handled);
END_TEST_PROGRAM

(* RejectBatchHandler: always handles regardless of event. *)
TEST_PROGRAM RejectBatchHandlerAlwaysHandles
VAR
    Handler : RejectBatchHandler;
    Event : WasherQualityEvent;
END_VAR
Event.SoilLevel := REAL#0.0;
Event.Conductivity := REAL#0.0;
Event.RinseConductivity := REAL#0.0;
Event.OperatorAvailable := FALSE;
ASSERT_TRUE(IN := Handler.Handle(Event := Event));
ASSERT_TRUE(IN := Handler.Handled);

(* Even with all conditions favoring upstream handlers, terminal still handles *)
Event.SoilLevel := REAL#100.0;
Event.Conductivity := REAL#1.0;
Event.RinseConductivity := REAL#5.0;
Event.OperatorAvailable := TRUE;
ASSERT_TRUE(IN := Handler.Handle(Event := Event));
END_TEST_PROGRAM

(* Counter accumulation across many events. *)
TEST_PROGRAM WasherCountersAccumulateCorrectlyAcrossManyEvents
VAR
    Washer : TunnelWasherQualityController;
END_VAR
Washer.Initialize();

(* 3 auto-dose events *)
Washer.Evaluate(SoilLevel := REAL#80.0, Conductivity := REAL#2.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE);
Washer.Evaluate(SoilLevel := REAL#85.0, Conductivity := REAL#1.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE);
Washer.Evaluate(SoilLevel := REAL#90.0, Conductivity := REAL#0.5, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#3, ACTUAL := Washer.AutoDoseCount);

(* 2 reject events *)
Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#5.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE);
Washer.Evaluate(SoilLevel := REAL#30.0, Conductivity := REAL#5.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE);
ASSERT_EQUAL(EXPECTED := DWORD#3, ACTUAL := Washer.AutoDoseCount);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := Washer.RejectCount);

(* ExtendRinse and OperatorHold do not increment AutoDose or Reject *)
Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#5.0, RinseConductivity := REAL#2.0, OperatorAvailable := FALSE);
Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#5.0, RinseConductivity := REAL#0.5, OperatorAvailable := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#3, ACTUAL := Washer.AutoDoseCount);
ASSERT_EQUAL(EXPECTED := DWORD#2, ACTUAL := Washer.RejectCount);
END_TEST_PROGRAM
