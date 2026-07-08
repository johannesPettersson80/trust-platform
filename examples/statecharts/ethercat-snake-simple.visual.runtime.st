(*
  Auto-generated runtime entry for ethercat-snake-simple.statechart.json.
  This wrapper is used to run visual editors through the Structured Text debugger path.
*)

PROGRAM PRG_ethercat_snake_simple_VISUAL
fb_ethercat_snake_simple();
END_PROGRAM

CONFIGURATION CFG_ethercat_snake_simple_VISUAL
VAR_GLOBAL
  fb_ethercat_snake_simple : FB_ethercat_snake_simple_STATECHART;
  ld_io_QX0_0 AT %QX0.0 : BOOL;
  ld_io_QX0_1 AT %QX0.1 : BOOL;
  ld_io_QX0_2 AT %QX0.2 : BOOL;
END_VAR
  RESOURCE RES_ethercat_snake_simple_VISUAL ON PLC
    TASK TASK_ethercat_snake_simple_VISUAL(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM PLC_PRG_ethercat_snake_simple WITH TASK_ethercat_snake_simple_VISUAL : PRG_ethercat_snake_simple_VISUAL;
  END_RESOURCE
END_CONFIGURATION
