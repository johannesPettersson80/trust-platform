(*
  Auto-generated runtime entry for ethercat-snake.ladder.json.
  This wrapper is used to run visual editors through the Structured Text debugger path.
*)

PROGRAM PRG_ethercat_snake_VISUAL
fb_ethercat_snake();
END_PROGRAM

CONFIGURATION CFG_ethercat_snake_VISUAL
VAR_GLOBAL
  fb_ethercat_snake : FB_ethercat_snake_LADDER;
  StartTrigger AT %MX0.0 : BOOL;
  Step0Active AT %MX1.0 : BOOL;
  Step1Active AT %MX2.0 : BOOL;
  Step2Active AT %MX3.0 : BOOL;
  Step3Active AT %MX4.0 : BOOL;
  Step4Active AT %MX5.0 : BOOL;
  Step5Active AT %MX6.0 : BOOL;
  Step6Active AT %MX7.0 : BOOL;
  Step7Active AT %MX8.0 : BOOL;
  Led0Output AT %QX0.0 : BOOL;
  Led1Output AT %QX0.1 : BOOL;
  Led2Output AT %QX0.2 : BOOL;
  Led3Output AT %QX0.3 : BOOL;
  Led4Output AT %QX0.4 : BOOL;
  Led5Output AT %QX0.5 : BOOL;
  Led6Output AT %QX0.6 : BOOL;
  Led7Output AT %QX0.7 : BOOL;
END_VAR
  RESOURCE RES_ethercat_snake_VISUAL ON PLC
    TASK TASK_ethercat_snake_VISUAL(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM PLC_PRG_ethercat_snake WITH TASK_ethercat_snake_VISUAL : PRG_ethercat_snake_VISUAL;
  END_RESOURCE
END_CONFIGURATION
