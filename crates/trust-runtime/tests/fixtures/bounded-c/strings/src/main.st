(* Bounded-C corpus, strings: bounded and unbounded STRING and a bounded WSTRING with every text
   function, conversions to and from text, truncation on assignment, and character reads and
   writes (census rows I1, I2, I8, I9, C9, C12 and C13). The unbounded line is longer than
   23 bytes, so it lives on the heap. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

PROGRAM Strings
VAR
    name : STRING[16] := 'pump';
    line : STRING := '';
    wide : WSTRING[16] := "valve";
    tag : STRING[8];
    text : STRING[40];
    size : INT;
    position : INT;
    number : DINT := DINT#42;
    parsed : DINT;
    reading : REAL := REAL#1.25;
    letter : CHAR;
END_VAR
ticks := ticks + DINT#1;
line := CONCAT(name, '-station-0042-north-wing');
size := LEN(line);
tag := LEFT(line, 4);
tag := RIGHT(IN := line, L := 4);
text := MID(line, 7, 6);
text := INSERT(name, '#', 2);
text := DELETE(line, 8, 1);
text := REPLACE(line, 'P', 1, 1);
position := FIND(line, 'station');
text := DINT_TO_STRING(number + ticks);
parsed := STRING_TO_DINT(text);
text := REAL_TO_STRING(reading);
wide := CONCAT(LEFT(wide, 5), "-1");
letter := name[1];
name[1] := 'P';
wide[1] := "V";
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Strings;
END_RESOURCE
END_CONFIGURATION
