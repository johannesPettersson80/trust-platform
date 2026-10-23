VAR_GLOBAL
    probe_depth : INT := INT#1;
END_VAR

FUNCTION ProbeLeaf : INT
ProbeLeaf := INT#1;
END_FUNCTION

FUNCTION ProbeMiddle : INT
ProbeMiddle := ProbeLeaf() + INT#1;
END_FUNCTION

FUNCTION ProbeOuter : INT
ProbeOuter := ProbeMiddle() + INT#1;
END_FUNCTION

PROGRAM Gpio
VAR
    button AT %IX0.0 : BOOL;
    led AT %QX0.0 : BOOL;
    depth_result : INT;
END_VAR
led := button;
CASE probe_depth OF
    2: depth_result := ProbeLeaf() + INT#1;
    3: depth_result := ProbeMiddle() + INT#1;
    4: depth_result := ProbeOuter() + INT#1;
ELSE
    depth_result := INT#1;
END_CASE;
END_PROGRAM

CONFIGURATION BoardProbe
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Gpio;
END_RESOURCE
END_CONFIGURATION
