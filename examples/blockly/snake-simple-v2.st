(*
  Auto-generated from blockly source: snake-simple-v2.blockly.json
  Source of truth is the visual file unless migrated fully to ST.
*)

FUNCTION_BLOCK FB_snake_simple_v2_BLOCKLY
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

VAR
  position : INT;
  direction : INT;
END_VAR

(* Generated from Blockly *)

position := 0;
direction := 1;
WHILE TRUE DO
  ld_io_QX0_0 := (position = 0);
  ld_io_QX0_1 := (position = 1);
  ld_io_QX0_2 := (position = 2);
  ld_io_QX0_3 := (position = 3);
  ld_io_QX0_4 := (position = 4);
  ld_io_QX0_5 := (position = 5);
  ld_io_QX0_6 := (position = 6);
  ld_io_QX0_7 := (position = 7);
  IF (direction = 1) THEN
    position := (position + 1);
    IF (position > 7) THEN
      position := 7;
      direction := -1;
    END_IF;
  END_IF;
  IF (direction = -1) THEN
    position := (position - 1);
    IF (position < 0) THEN
      position := 0;
      direction := 1;
    END_IF;
  END_IF;
END_WHILE;

END_FUNCTION_BLOCK
