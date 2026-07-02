CONFIGURATION MotionStarter
TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM P1 WITH MainTask : Main;
VAR_CONFIG
    P1.motion_demo_cycles   AT %MW0   : INT;
    P1.motion_demo_error    AT %MW2   : INT;
    P1.motion_demo_power_on   AT %QX0.0 : BOOL;
    P1.motion_demo_target_ok  AT %QX0.1 : BOOL;
    P1.motion_demo_standstill AT %QX0.2 : BOOL;
END_VAR
END_CONFIGURATION
