TEST_PROGRAM PasteurizerTemplateUsesRecipeHooks
VAR
    Milk : MilkPasteurizer;
    Cream : CreamPasteurizer;
    MilkResult : PasteurizerResult;
    CreamResult : PasteurizerResult;
END_VAR

Milk.Initialize();
Cream.Initialize();
MilkResult := Milk.RunTemplate(ProductPresent := TRUE, ProductTemp := REAL#73.0, FlowRate := REAL#120.0, HoldSeconds := INT#15);
CreamResult := Cream.RunTemplate(ProductPresent := TRUE, ProductTemp := REAL#73.0, FlowRate := REAL#120.0, HoldSeconds := INT#15);
ASSERT_TRUE(IN := MilkResult.ForwardDivert);
ASSERT_FALSE(IN := CreamResult.ForwardDivert);
ASSERT_NEAR(EXPECTED := REAL#75.0, ACTUAL := CreamResult.RequiredTemperature, DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := CreamResult.RequiredHoldSeconds);
END_TEST_PROGRAM

TEST_PROGRAM QualityAlarmChainEscalatesToAbortWhenUnhandled
VAR
    Controller : PasteurizerController;
    Result : PasteurizerResult;
END_VAR

Controller.Initialize();
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#68.0, FlowRate := REAL#90.0, HoldSeconds := INT#5, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_FALSE(IN := Result.ForwardDivert);
ASSERT_TRUE(IN := Controller.BatchAborted);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Controller.LastHandlerId);
END_TEST_PROGRAM

TEST_PROGRAM QualityAlarmChainStopsAtOperatorAcknowledge
VAR
    Controller : PasteurizerController;
    Result : PasteurizerResult;
END_VAR

Controller.Initialize();
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#68.0, FlowRate := REAL#90.0, HoldSeconds := INT#5, OperatorAck := TRUE, SupervisorOnline := FALSE);
ASSERT_FALSE(IN := Result.ForwardDivert);
ASSERT_FALSE(IN := Controller.BatchAborted);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Controller.LastHandlerId);
END_TEST_PROGRAM

(* Multi-batch chain regression: drives several different batches
   through one Controller. Catches the same bug class as
   tunnel_washer_chain — handler flags or LastHandlerId leaking
   between events because handlers don't reset their per-call state. *)
TEST_PROGRAM PasteurizerChainHandlesMultipleBatchesIndependently
VAR
    Controller : PasteurizerController;
    Result : PasteurizerResult;
END_VAR

Controller.Initialize();

(* Batch 1: bad temp + supervisor online -> escalates to supervisor (3) *)
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#68.0, FlowRate := REAL#90.0, HoldSeconds := INT#5, OperatorAck := FALSE, SupervisorOnline := TRUE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Controller.LastHandlerId);

(* Batch 2: same bad temp but operator now acks -> stops at handler 2.
   LastHandlerId must reflect the new outcome, not the previous (3). *)
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#68.0, FlowRate := REAL#90.0, HoldSeconds := INT#5, OperatorAck := TRUE, SupervisorOnline := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Controller.LastHandlerId);

(* Batch 3: nothing helps -> escalates to abort (4) *)
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#68.0, FlowRate := REAL#90.0, HoldSeconds := INT#5, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Controller.LastHandlerId);
ASSERT_TRUE(IN := Controller.BatchAborted);
END_TEST_PROGRAM

(* Happy path: in-spec batch forwards directly. *)
TEST_PROGRAM PasteurizerForwardsInSpecBatchWithoutEscalation
VAR
    Controller : PasteurizerController;
    Result : PasteurizerResult;
END_VAR
Controller.Initialize();
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#74.0, FlowRate := REAL#120.0, HoldSeconds := INT#16, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_TRUE(IN := Result.ForwardDivert);
ASSERT_FALSE(IN := Controller.BatchAborted);
END_TEST_PROGRAM

(* LocalRecoveryHandler in isolation: requires temp within 1 C AND flow >= 100. *)
TEST_PROGRAM LocalRecoveryHandlerHandlesNearMissOnly
VAR
    Local : LocalRecoveryHandler;
    Terminal : BatchAbortHandler;
    Event : QualityAlarmEvent;
END_VAR
Local.SetNext(NextHandler := Terminal);

(* Within 1 C of required + flow ok: handles *)
Event.RequiredTemperatureC := REAL#73.0;
Event.TemperatureC := REAL#72.5;
Event.FlowRateLpm := REAL#110.0;
Event.OperatorAck := FALSE;
Event.SupervisorOnline := FALSE;
Local.Handle(Event := Event);
ASSERT_TRUE(IN := Local.Recovered);

(* Temp too far below required: passes through *)
Event.TemperatureC := REAL#68.0;
Local.Handle(Event := Event);
ASSERT_FALSE(IN := Local.Recovered);

(* Temp ok but flow too low: passes through *)
Event.TemperatureC := REAL#72.5;
Event.FlowRateLpm := REAL#90.0;
Local.Handle(Event := Event);
ASSERT_FALSE(IN := Local.Recovered);

(* Boundary: flow exactly 100 - inclusive *)
Event.FlowRateLpm := REAL#100.0;
Local.Handle(Event := Event);
ASSERT_TRUE(IN := Local.Recovered);

(* Boundary: temp exactly Required - 1.0 - inclusive *)
Event.TemperatureC := REAL#72.0;
Event.FlowRateLpm := REAL#100.0;
Local.Handle(Event := Event);
ASSERT_TRUE(IN := Local.Recovered);
END_TEST_PROGRAM

(* OperatorAcknowledgeHandler isolation. *)
TEST_PROGRAM OperatorAcknowledgeHandlerHandlesOnlyWhenAckPresent
VAR
    Operator : OperatorAcknowledgeHandler;
    Terminal : BatchAbortHandler;
    Event : QualityAlarmEvent;
END_VAR
Operator.SetNext(NextHandler := Terminal);

Event.OperatorAck := TRUE;
Operator.Handle(Event := Event);
ASSERT_TRUE(IN := Operator.Acknowledged);

Event.OperatorAck := FALSE;
Operator.Handle(Event := Event);
ASSERT_FALSE(IN := Operator.Acknowledged);
END_TEST_PROGRAM

(* SupervisorEscalateHandler isolation. *)
TEST_PROGRAM SupervisorEscalateHandlerHandlesOnlyWhenSupervisorOnline
VAR
    Supervisor : SupervisorEscalateHandler;
    Terminal : BatchAbortHandler;
    Event : QualityAlarmEvent;
END_VAR
Supervisor.SetNext(NextHandler := Terminal);

Event.SupervisorOnline := TRUE;
Supervisor.Handle(Event := Event);
ASSERT_TRUE(IN := Supervisor.Escalated);

Event.SupervisorOnline := FALSE;
Supervisor.Handle(Event := Event);
ASSERT_FALSE(IN := Supervisor.Escalated);
END_TEST_PROGRAM

(* Local recovery is preferred over operator/supervisor when both could handle. *)
TEST_PROGRAM PasteurizerChainPrefersLocalRecoveryWhenEligible
VAR
    Controller : PasteurizerController;
    Result : PasteurizerResult;
END_VAR
Controller.Initialize();
(* Slightly-off temp (below milk required 72 C) + flow >= 100 + operator
   + supervisor all available. Heat fails so divert triggers and chain
   runs. Order Local -> Operator -> Supervisor -> Abort. Local wins
   because TemperatureC (71.5) >= RequiredTemperatureC - 1.0 (71.0)
   and FlowRateLpm (110) >= 100.0. *)
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#71.5, FlowRate := REAL#110.0, HoldSeconds := INT#16, OperatorAck := TRUE, SupervisorOnline := TRUE);
ASSERT_FALSE(IN := Result.ForwardDivert);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Controller.LastHandlerId);
END_TEST_PROGRAM

(* Recipe variant difference: cream requires 75 C, milk requires 73 C. *)
TEST_PROGRAM PasteurizerCreamRecipeRequiresHigherTemperature
VAR
    Controller : PasteurizerController;
    Result : PasteurizerResult;
END_VAR
Controller.Initialize();

(* 73 C: fine for milk (recipe 1), insufficient for cream (recipe 2) *)
Result := Controller.Update(RecipeId := INT#1, ProductTemp := REAL#73.0, FlowRate := REAL#120.0, HoldSeconds := INT#15, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_TRUE(IN := Result.ForwardDivert);

(* Cream needs 75 - 73 falls into LocalRecovery range only with hold *)
Result := Controller.Update(RecipeId := INT#2, ProductTemp := REAL#73.0, FlowRate := REAL#120.0, HoldSeconds := INT#15, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_FALSE(IN := Result.ForwardDivert);

(* 75 C: cream passes *)
Result := Controller.Update(RecipeId := INT#2, ProductTemp := REAL#76.0, FlowRate := REAL#120.0, HoldSeconds := INT#20, OperatorAck := FALSE, SupervisorOnline := FALSE);
ASSERT_TRUE(IN := Result.ForwardDivert);
END_TEST_PROGRAM
