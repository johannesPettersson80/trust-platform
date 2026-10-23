PROGRAM Numeric
VAR
    v_sqrt : LREAL;
    v_ln : LREAL;
    v_log : LREAL;
    v_exp : LREAL;
    v_sin : LREAL;
    v_cos : LREAL;
    v_tan : LREAL;
    v_asin : LREAL;
    v_acos : LREAL;
    v_atan : LREAL;
    exact_root : REAL;
    exact_power : LREAL;
    sum : REAL;
    scaled : TIME;
    command : BOOL;
    activations : DINT;
END_VAR
v_sqrt := SQRT(LREAL#2.0);
v_ln := LN(LREAL#2.0);
v_log := LOG(LREAL#10.0);
v_exp := EXP(LREAL#1.0);
v_sin := SIN(LREAL#0.5);
v_cos := COS(LREAL#0.5);
v_tan := TAN(LREAL#0.5);
v_asin := ASIN(LREAL#0.5);
v_acos := ACOS(LREAL#0.5);
v_atan := ATAN(LREAL#1.0);
exact_root := SQRT(REAL#9.0);
exact_power := LREAL#2.0 ** LREAL#3.0;
sum := REAL#1.5 + REAL#2.25;
scaled := T#10ms * REAL#1.5;
command := exact_root = REAL#3.0 AND exact_power = LREAL#8.0 AND sum = REAL#3.75;
activations := activations + DINT#1;
END_PROGRAM
CONFIGURATION NumericConfiguration
RESOURCE Controller ON PLC
TASK Tick (INTERVAL := T#25ms, PRIORITY := 0);
PROGRAM Plant WITH Tick : Numeric;
END_RESOURCE
END_CONFIGURATION
