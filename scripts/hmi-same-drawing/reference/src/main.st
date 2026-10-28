// Reference program for the same-drawing check (specification 39, §19.4). Every bound variable
// holds the design sample the editor draws for it (§3.4): numbers at 60 % of their range,
// booleans on, and text its variable's name, so the editor and the runtime draw the same values.

PROGRAM Main
VAR
    Temperature : REAL := 60.0;
    Small : REAL := 60.0;
    Pressure : LREAL := 120.0;
    Level : REAL := 60.0;
    Flow : REAL := 60.0;
    Demand : REAL := 60.0;
    FirstShare : REAL := 60.0;
    SecondShare : REAL := 60.0;
    Running : BOOL := TRUE;
    Ready : BOOL := TRUE;
    Message : STRING := 'Message';
END_VAR
END_PROGRAM

CONFIGURATION Reference
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Plant WITH Fast : Main;
    PROGRAM Loop WITH Fast : PidLoop;
    PROGRAM ScreenService WITH Fast : Screens;
END_RESOURCE
END_CONFIGURATION

PROGRAM Screens
VAR Display : Hmi.Display; END_VAR
Display();
END_PROGRAM
