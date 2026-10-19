(* Bounded-C corpus, results: functions that take and give texts, arrays, structures and
   references (design row 28, "functions with results of every family"): a STRING and a WSTRING
   input and result, an array input and result, a structure result and a REF_TO result. *)
TYPE Triple : ARRAY[1..3] OF DINT;
END_TYPE

TYPE Reading :
STRUCT
    level : DINT;
    label : STRING[12];
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
    shared : DINT := DINT#7;
END_VAR

FUNCTION Greet : STRING[24]
VAR_INPUT
    who : STRING[16];
END_VAR
Greet := CONCAT('hello ', who);
END_FUNCTION

FUNCTION Wide : WSTRING[12]
VAR_INPUT
    tag : WSTRING[8];
END_VAR
Wide := CONCAT(tag, "-ok");
END_FUNCTION

FUNCTION Doubled : Triple
VAR_INPUT
    values : Triple;
END_VAR
VAR
    k : INT;
    product : Triple;
END_VAR
FOR k := INT#1 TO INT#3 DO
    product[k] := values[k] * DINT#2;
END_FOR;
Doubled := product;
END_FUNCTION

FUNCTION Measure : Reading
VAR_INPUT
    amount : DINT;
END_VAR
VAR
    taken : Reading;
END_VAR
taken.level := amount;
taken.label := 'tank';
Measure := taken;
END_FUNCTION

FUNCTION PointShared : REF_TO DINT
PointShared := REF(shared);
END_FUNCTION

PROGRAM Results
VAR
    name : STRING[16] := 'station';
    greeting : STRING[24];
    tag : WSTRING[8] := "Pumpe";
    tagged : WSTRING[12];
    numbers : Triple;
    twice : Triple;
    measurement : Reading;
    through : REF_TO DINT;
    seen : DINT;
END_VAR
ticks := ticks + DINT#1;
greeting := Greet(who := name);
tagged := Wide(tag := tag);
numbers[1] := ticks;
numbers[2] := ticks + DINT#1;
numbers[3] := ticks + DINT#2;
twice := Doubled(values := numbers);
measurement := Measure(amount := ticks);
through := PointShared();
seen := through^;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Results;
END_RESOURCE
END_CONFIGURATION
