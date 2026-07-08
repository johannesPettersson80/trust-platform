(*
  Auto-generated runtime entry for snake-simple-v2.blockly.json.
  This wrapper is used to run visual editors through the Structured Text debugger path.
*)

PROGRAM PRG_snake_simple_v2_VISUAL
fb_snake_simple_v2();
END_PROGRAM

CONFIGURATION CFG_snake_simple_v2_VISUAL
VAR_GLOBAL
  fb_snake_simple_v2 : FB_snake_simple_v2_BLOCKLY;
  ld_io_QX0_0 AT %QX0.0 : BOOL;
  ld_io_QX0_1 AT %QX0.1 : BOOL;
  ld_io_QX0_2 AT %QX0.2 : BOOL;
  ld_io_QX0_3 AT %QX0.3 : BOOL;
  ld_io_QX0_4 AT %QX0.4 : BOOL;
  ld_io_QX0_5 AT %QX0.5 : BOOL;
  ld_io_QX0_6 AT %QX0.6 : BOOL;
  ld_io_QX0_7 AT %QX0.7 : BOOL;
END_VAR
  RESOURCE RES_snake_simple_v2_VISUAL ON PLC
    TASK TASK_snake_simple_v2_VISUAL(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM PLC_PRG_snake_simple_v2 WITH TASK_snake_simple_v2_VISUAL : PRG_snake_simple_v2_VISUAL;
  END_RESOURCE
END_CONFIGURATION
