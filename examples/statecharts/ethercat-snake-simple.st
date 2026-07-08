(*
  Auto-generated from statechart source: ethercat-snake-simple.statechart.json
  Source of truth is the visual file unless migrated fully to ST.
*)
(*
  Auto-generated Statechart companion.
  Source of truth remains the .statechart.json file.
  Generated to support .st-first runtime workflows.
*)

FUNCTION_BLOCK FB_ethercat_snake_simple_STATECHART
VAR_EXTERNAL
  ld_io_QX0_0 : BOOL;
  ld_io_QX0_1 : BOOL;
  ld_io_QX0_2 : BOOL;
END_VAR

VAR_INPUT
  EV_START : BOOL;
  EV_TICK : BOOL;
END_VAR
VAR_OUTPUT
  STATE_INDEX : INT;
  STATE_NAME : STRING[80];
END_VAR
VAR
  _initialized : BOOL := FALSE;
  _state : INT := 0;
END_VAR

(* State encoding *)
(*  0 = Init *)
(*  1 = LED_0 *)
(*  2 = LED_1 *)
(*  3 = LED_2 *)
(*  4 = LED_1_Back *)
(*  5 = LED_0_Back *)

IF NOT _initialized THEN
  _state := 0;
  _initialized := TRUE;
END_IF;

CASE _state OF
  0:
    IF EV_START THEN
      ld_io_QX0_0 := TRUE;
      (* LOG: → LED 0: ●○○ *)
      _state := 1;
    END_IF;
  1:
    IF EV_TICK THEN
      ld_io_QX0_0 := FALSE;
      ld_io_QX0_1 := TRUE;
      (* LOG: → LED 1: ○●○ *)
      _state := 2;
    END_IF;
  2:
    IF EV_TICK THEN
      ld_io_QX0_1 := FALSE;
      ld_io_QX0_2 := TRUE;
      (* LOG: → LED 2: ○○● (reversing) *)
      _state := 3;
    END_IF;
  3:
    IF EV_TICK THEN
      ld_io_QX0_2 := FALSE;
      ld_io_QX0_1 := TRUE;
      (* LOG: ← LED 1: ○●○ *)
      _state := 4;
    END_IF;
  4:
    IF EV_TICK THEN
      ld_io_QX0_1 := FALSE;
      ld_io_QX0_0 := TRUE;
      (* LOG: ← LED 0: ●○○ (cycle complete) *)
      _state := 5;
    END_IF;
  5:
    IF EV_TICK THEN
      ld_io_QX0_0 := FALSE;
      (* LOG: 🔁 Cycle complete, restarting... *)
      ld_io_QX0_1 := TRUE;
      (* LOG: → LED 1: ○●○ *)
      _state := 2;
    END_IF;
END_CASE;

STATE_INDEX := _state;
CASE _state OF
  0: STATE_NAME := 'Init';
  1: STATE_NAME := 'LED_0';
  2: STATE_NAME := 'LED_1';
  3: STATE_NAME := 'LED_2';
  4: STATE_NAME := 'LED_1_Back';
  5: STATE_NAME := 'LED_0_Back';
  ELSE
    STATE_NAME := 'UNKNOWN';
END_CASE;
END_FUNCTION_BLOCK
