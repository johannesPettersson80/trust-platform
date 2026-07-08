(*
  Auto-generated from ladder source: ethercat-snake.ladder.json
  Source of truth is the visual file unless migrated fully to ST.
*)
(*
  Auto-generated Ladder Diagram companion (schema v2).
  Source of truth remains the .ladder.json file.
  Generated to support .st-first runtime workflows.
*)

FUNCTION_BLOCK FB_ethercat_snake_LADDER
VAR_EXTERNAL
  StartTrigger : BOOL;
  Step0Active : BOOL;
  Step1Active : BOOL;
  Step2Active : BOOL;
  Step3Active : BOOL;
  Step4Active : BOOL;
  Step5Active : BOOL;
  Step6Active : BOOL;
  Step7Active : BOOL;
  Led0Output : BOOL;
  Led1Output : BOOL;
  Led2Output : BOOL;
  Led3Output : BOOL;
  Led4Output : BOOL;
  Led5Output : BOOL;
  Led6Output : BOOL;
  Led7Output : BOOL;
END_VAR

(* Network 0: rung_step0_set *)
IF StartTrigger AND NOT (Step0Active) THEN
  Step0Active := TRUE;
END_IF;

(* Network 1: rung_step0_output *)
Led0Output := Step0Active;

(* Network 2: rung_step1_set *)
IF Step0Active AND NOT (Step1Active) THEN
  Step1Active := TRUE;
END_IF;
IF Step0Active AND NOT (Step1Active) THEN
  Step0Active := FALSE;
END_IF;

(* Network 3: rung_step1_output *)
Led1Output := Step1Active;

(* Network 4: rung_step2_set *)
IF Step1Active AND NOT (Step2Active) THEN
  Step2Active := TRUE;
END_IF;
IF Step1Active AND NOT (Step2Active) THEN
  Step1Active := FALSE;
END_IF;

(* Network 5: rung_step2_output *)
Led2Output := Step2Active;

(* Network 6: rung_step3_set *)
IF Step2Active AND NOT (Step3Active) THEN
  Step3Active := TRUE;
END_IF;
IF Step2Active AND NOT (Step3Active) THEN
  Step2Active := FALSE;
END_IF;

(* Network 7: rung_step3_output *)
Led3Output := Step3Active;

(* Network 8: rung_step4_set *)
IF Step3Active AND NOT (Step4Active) THEN
  Step4Active := TRUE;
END_IF;
IF Step3Active AND NOT (Step4Active) THEN
  Step3Active := FALSE;
END_IF;

(* Network 9: rung_step4_output *)
Led4Output := Step4Active;

(* Network 10: rung_step5_set *)
IF Step4Active AND NOT (Step5Active) THEN
  Step5Active := TRUE;
END_IF;
IF Step4Active AND NOT (Step5Active) THEN
  Step4Active := FALSE;
END_IF;

(* Network 11: rung_step5_output *)
Led5Output := Step5Active;

(* Network 12: rung_step6_set *)
IF Step5Active AND NOT (Step6Active) THEN
  Step6Active := TRUE;
END_IF;
IF Step5Active AND NOT (Step6Active) THEN
  Step5Active := FALSE;
END_IF;

(* Network 13: rung_step6_output *)
Led6Output := Step6Active;

(* Network 14: rung_step7_set *)
IF Step6Active AND NOT (Step7Active) THEN
  Step7Active := TRUE;
END_IF;
IF Step6Active AND NOT (Step7Active) THEN
  Step6Active := FALSE;
END_IF;

(* Network 15: rung_step7_output *)
Led7Output := Step7Active;

(* Network 16: rung_restart *)
IF Step7Active AND NOT (Step0Active) THEN
  Step0Active := TRUE;
END_IF;
IF Step7Active AND NOT (Step0Active) THEN
  Step7Active := FALSE;
END_IF;

END_FUNCTION_BLOCK
