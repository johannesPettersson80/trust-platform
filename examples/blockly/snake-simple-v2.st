(*
  Auto-generated from blockly source: snake-simple-v2.blockly.json
  Source of truth is the visual file unless migrated fully to ST.
*)

FUNCTION_BLOCK FB_snake_simple_v2_BLOCKLY

VAR
  position : INT;
  direction : INT;
END_VAR

(* Generated from Blockly *)

position := 0;
direction := 1;
WHILE TRUE DO
  %QX0.0 := (position = 0);
  %QX0.1 := (position = 1);
  %QX0.2 := (position = 2);
  %QX0.3 := (position = 3);
  %QX0.4 := (position = 4);
  %QX0.5 := (position = 5);
  %QX0.6 := (position = 6);
  %QX0.7 := (position = 7);
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
