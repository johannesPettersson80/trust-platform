(*
  Auto-generated from statechart source: ethercat-snake.statechart.json
  Source of truth is the visual file unless migrated fully to ST.
*)
(*
  Auto-generated Statechart companion.
  Source of truth remains the .statechart.json file.
  Generated to support .st-first runtime workflows.
*)

FUNCTION_BLOCK FB_ethercat_snake_STATECHART
VAR_EXTERNAL
  ld_io_QX0_0 : BOOL;
  ld_io_QX0_1 : BOOL;
  ld_io_QX0_2 : BOOL;
  ld_io_QX0_3 : BOOL;
  ld_io_QX0_4 : BOOL;
  ld_io_QX0_5 : BOOL;
  ld_io_QX0_6 : BOOL;
  ld_io_QX0_7 : BOOL;
END_VAR

VAR_OUTPUT
  STATE_INDEX : INT;
  STATE_NAME : STRING[80];
END_VAR
VAR
  _initialized : BOOL := FALSE;
  _state : INT := 0;
  sc_timer_S0_AllOff_TIMER_0 : TON; (* S0_AllOff.TIMER *)
  sc_timer_S1_LED0_On_TIMER_0 : TON; (* S1_LED0_On.TIMER *)
  sc_timer_S2_LED0_1_On_TIMER_0 : TON; (* S2_LED0_1_On.TIMER *)
  sc_timer_S3_LED0_2_On_TIMER_0 : TON; (* S3_LED0_2_On.TIMER *)
  sc_timer_S4_LED0_3_On_TIMER_0 : TON; (* S4_LED0_3_On.TIMER *)
  sc_timer_S5_LED0_4_On_TIMER_0 : TON; (* S5_LED0_4_On.TIMER *)
  sc_timer_S6_LED0_5_On_TIMER_0 : TON; (* S6_LED0_5_On.TIMER *)
  sc_timer_S7_LED0_6_On_TIMER_0 : TON; (* S7_LED0_6_On.TIMER *)
  sc_timer_S8_AllOn_TIMER_0 : TON; (* S8_AllOn.TIMER *)
  sc_timer_S9_LED7_Off_TIMER_0 : TON; (* S9_LED7_Off.TIMER *)
  sc_timer_S10_LED6_Off_TIMER_0 : TON; (* S10_LED6_Off.TIMER *)
  sc_timer_S11_LED5_Off_TIMER_0 : TON; (* S11_LED5_Off.TIMER *)
  sc_timer_S12_LED4_Off_TIMER_0 : TON; (* S12_LED4_Off.TIMER *)
  sc_timer_S13_LED3_Off_TIMER_0 : TON; (* S13_LED3_Off.TIMER *)
  sc_timer_S14_LED2_Off_TIMER_0 : TON; (* S14_LED2_Off.TIMER *)
  sc_timer_S15_LED1_Off_TIMER_0 : TON; (* S15_LED1_Off.TIMER *)
  sc_timer_S16_LED0_Off_TIMER_0 : TON; (* S16_LED0_Off.TIMER *)
END_VAR

(* State encoding *)
(*  0 = S0_AllOff *)
(*  1 = S1_LED0_On *)
(*  2 = S2_LED0_1_On *)
(*  3 = S3_LED0_2_On *)
(*  4 = S4_LED0_3_On *)
(*  5 = S5_LED0_4_On *)
(*  6 = S6_LED0_5_On *)
(*  7 = S7_LED0_6_On *)
(*  8 = S8_AllOn *)
(*  9 = S9_LED7_Off *)
(*  10 = S10_LED6_Off *)
(*  11 = S11_LED5_Off *)
(*  12 = S12_LED4_Off *)
(*  13 = S13_LED3_Off *)
(*  14 = S14_LED2_Off *)
(*  15 = S15_LED1_Off *)
(*  16 = S16_LED0_Off *)

IF NOT _initialized THEN
  _state := 0;
  _initialized := TRUE;
END_IF;

sc_timer_S0_AllOff_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S1_LED0_On_TIMER_0(IN := FALSE, PT := T#1000ms);
sc_timer_S2_LED0_1_On_TIMER_0(IN := FALSE, PT := T#1000ms);
sc_timer_S3_LED0_2_On_TIMER_0(IN := FALSE, PT := T#1000ms);
sc_timer_S4_LED0_3_On_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S5_LED0_4_On_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S6_LED0_5_On_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S7_LED0_6_On_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S8_AllOn_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S9_LED7_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S10_LED6_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S11_LED5_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S12_LED4_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S13_LED3_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S14_LED2_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S15_LED1_Off_TIMER_0(IN := FALSE, PT := T#200ms);
sc_timer_S16_LED0_Off_TIMER_0(IN := FALSE, PT := T#200ms);

CASE _state OF
  0:
    sc_timer_S0_AllOff_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S0_AllOff_TIMER_0.Q THEN
      (* LOG: Transitioning to next state *)
      ld_io_QX0_0 := TRUE;
      _state := 1;
    END_IF;
  1:
    sc_timer_S1_LED0_On_TIMER_0(IN := TRUE, PT := T#1000ms);
    IF sc_timer_S1_LED0_On_TIMER_0.Q THEN
      ld_io_QX0_1 := TRUE;
      _state := 2;
    END_IF;
  2:
    sc_timer_S2_LED0_1_On_TIMER_0(IN := TRUE, PT := T#1000ms);
    IF sc_timer_S2_LED0_1_On_TIMER_0.Q THEN
      ld_io_QX0_2 := TRUE;
      _state := 3;
    END_IF;
  3:
    sc_timer_S3_LED0_2_On_TIMER_0(IN := TRUE, PT := T#1000ms);
    IF sc_timer_S3_LED0_2_On_TIMER_0.Q THEN
      ld_io_QX0_3 := TRUE;
      _state := 4;
    END_IF;
  4:
    sc_timer_S4_LED0_3_On_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S4_LED0_3_On_TIMER_0.Q THEN
      ld_io_QX0_4 := TRUE;
      _state := 5;
    END_IF;
  5:
    sc_timer_S5_LED0_4_On_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S5_LED0_4_On_TIMER_0.Q THEN
      ld_io_QX0_5 := TRUE;
      _state := 6;
    END_IF;
  6:
    sc_timer_S6_LED0_5_On_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S6_LED0_5_On_TIMER_0.Q THEN
      ld_io_QX0_6 := TRUE;
      _state := 7;
    END_IF;
  7:
    sc_timer_S7_LED0_6_On_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S7_LED0_6_On_TIMER_0.Q THEN
      ld_io_QX0_7 := TRUE;
      _state := 8;
    END_IF;
  8:
    sc_timer_S8_AllOn_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S8_AllOn_TIMER_0.Q THEN
      (* LOG: Phase change: Starting turn-off sequence *)
      ld_io_QX0_7 := FALSE;
      _state := 9;
    END_IF;
  9:
    sc_timer_S9_LED7_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S9_LED7_Off_TIMER_0.Q THEN
      ld_io_QX0_6 := FALSE;
      _state := 10;
    END_IF;
  10:
    sc_timer_S10_LED6_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S10_LED6_Off_TIMER_0.Q THEN
      ld_io_QX0_5 := FALSE;
      _state := 11;
    END_IF;
  11:
    sc_timer_S11_LED5_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S11_LED5_Off_TIMER_0.Q THEN
      ld_io_QX0_4 := FALSE;
      _state := 12;
    END_IF;
  12:
    sc_timer_S12_LED4_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S12_LED4_Off_TIMER_0.Q THEN
      ld_io_QX0_3 := FALSE;
      _state := 13;
    END_IF;
  13:
    sc_timer_S13_LED3_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S13_LED3_Off_TIMER_0.Q THEN
      ld_io_QX0_2 := FALSE;
      _state := 14;
    END_IF;
  14:
    sc_timer_S14_LED2_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S14_LED2_Off_TIMER_0.Q THEN
      ld_io_QX0_1 := FALSE;
      _state := 15;
    END_IF;
  15:
    sc_timer_S15_LED1_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S15_LED1_Off_TIMER_0.Q THEN
      ld_io_QX0_0 := FALSE;
      _state := 16;
    END_IF;
  16:
    sc_timer_S16_LED0_Off_TIMER_0(IN := TRUE, PT := T#200ms);
    IF sc_timer_S16_LED0_Off_TIMER_0.Q THEN
      (* LOG: Cycle complete, restarting *)
      ld_io_QX0_0 := TRUE;
      _state := 1;
    END_IF;
END_CASE;

STATE_INDEX := _state;
CASE _state OF
  0: STATE_NAME := 'S0_AllOff';
  1: STATE_NAME := 'S1_LED0_On';
  2: STATE_NAME := 'S2_LED0_1_On';
  3: STATE_NAME := 'S3_LED0_2_On';
  4: STATE_NAME := 'S4_LED0_3_On';
  5: STATE_NAME := 'S5_LED0_4_On';
  6: STATE_NAME := 'S6_LED0_5_On';
  7: STATE_NAME := 'S7_LED0_6_On';
  8: STATE_NAME := 'S8_AllOn';
  9: STATE_NAME := 'S9_LED7_Off';
  10: STATE_NAME := 'S10_LED6_Off';
  11: STATE_NAME := 'S11_LED5_Off';
  12: STATE_NAME := 'S12_LED4_Off';
  13: STATE_NAME := 'S13_LED3_Off';
  14: STATE_NAME := 'S14_LED2_Off';
  15: STATE_NAME := 'S15_LED1_Off';
  16: STATE_NAME := 'S16_LED0_Off';
  ELSE
    STATE_NAME := 'UNKNOWN';
END_CASE;
END_FUNCTION_BLOCK
