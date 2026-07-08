(*
  Auto-generated runtime entry for ethercat-snake.statechart.json.
  This wrapper is used to run visual editors through the Structured Text debugger path.
*)

PROGRAM PRG_ethercat_snake_VISUAL
fb_ethercat_snake();
END_PROGRAM

CONFIGURATION CFG_ethercat_snake_VISUAL
VAR_GLOBAL
  fb_ethercat_snake : FB_ethercat_snake_STATECHART;
  ld_io_QX0_0 AT %QX0.0 : BOOL;
  ld_io_QX0_1 AT %QX0.1 : BOOL;
  ld_io_QX0_2 AT %QX0.2 : BOOL;
  ld_io_QX0_3 AT %QX0.3 : BOOL;
  ld_io_QX0_4 AT %QX0.4 : BOOL;
  ld_io_QX0_5 AT %QX0.5 : BOOL;
  ld_io_QX0_6 AT %QX0.6 : BOOL;
  ld_io_QX0_7 AT %QX0.7 : BOOL;
END_VAR
  RESOURCE RES_ethercat_snake_VISUAL ON PLC
    TASK TASK_ethercat_snake_VISUAL(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM PLC_PRG_ethercat_snake WITH TASK_ethercat_snake_VISUAL : PRG_ethercat_snake_VISUAL;
  END_RESOURCE
END_CONFIGURATION
