(* Bounded-C corpus, faults: one faulting scan per selected kind (census rows F1 to F4, S10 and
   S11). The test writes fault_kind between scans: 1 division by zero, 2 an index outside the
   array, 3 a NULL dereference, 4 a REAL overflow, 5 the work allowance, 6 a call chain one
   level deeper than the chips admit. Any other value runs the healthy branch. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
    fault_kind : INT := INT#0;
    zero : DINT := DINT#0;
    outside : DINT := DINT#9;
    nothing : REF_TO DINT;
    big : REAL := REAL#1.0E30;
    spins : DINT := DINT#0;
END_VAR

FUNCTION Depth1 : INT
Depth1 := INT#1;
END_FUNCTION

FUNCTION Depth2 : INT
Depth2 := Depth1() + INT#1;
END_FUNCTION

FUNCTION Depth3 : INT
Depth3 := Depth2() + INT#1;
END_FUNCTION

FUNCTION Depth4 : INT
Depth4 := Depth3() + INT#1;
END_FUNCTION

PROGRAM Faults
VAR
    values : ARRAY[1..4] OF DINT;
    result : DINT;
    scaled : REAL;
    depth : INT;
END_VAR
ticks := ticks + DINT#1;
CASE fault_kind OF
    1: result := DINT#100 / zero;
    2: result := values[outside];
    3: result := nothing^;
    4: scaled := big * big;
    5:
        WHILE TRUE DO
            spins := spins + DINT#1;
        END_WHILE;
    6: depth := Depth4();
ELSE
    result := values[1] + DINT#1;
END_CASE;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Faults;
END_RESOURCE
END_CONFIGURATION
