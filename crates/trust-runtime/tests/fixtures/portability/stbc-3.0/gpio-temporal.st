(* Current-format board probe. The GPIO and depth oracle is unchanged, but
   neither can pass unless the admitted physical temporal layout is 64-bit. *)
TYPE TemporalOverlay : STRUCT OVERLAP
    elapsed AT %B0 : TIME;
    raw AT %B0 : LINT;
END_STRUCT; END_TYPE

VAR_GLOBAL
    probe_depth : INT := INT#1;
    temporal : TemporalOverlay;
    positive_ticks : LINT;
    temporal_ok : BOOL;
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
temporal.elapsed := T#5s;
positive_ticks := temporal.raw;
temporal.raw := LINT#-5000000000;
temporal_ok := (SIZEOF(TIME) = 8) AND (SIZEOF(DATE) = 8)
    AND (SIZEOF(TOD) = 8) AND (SIZEOF(DT) = 8)
    AND (SIZEOF(TemporalOverlay) = 8)
    AND (positive_ticks = LINT#5000000000)
    AND (temporal.elapsed = T#-5s);
led := button AND temporal_ok;
CASE probe_depth OF
    2: depth_result := ProbeLeaf() + INT#1;
    3: depth_result := ProbeMiddle() + INT#1;
    4: depth_result := ProbeOuter() + INT#1;
ELSE
    depth_result := INT#1;
END_CASE;
IF NOT temporal_ok THEN
    depth_result := INT#-1;
END_IF;
END_PROGRAM

CONFIGURATION BoardProbe
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Gpio;
END_RESOURCE
END_CONFIGURATION
