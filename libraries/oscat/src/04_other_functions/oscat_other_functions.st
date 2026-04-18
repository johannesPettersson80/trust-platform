(* Initial OSCAT BASIC constant subset ported from OSCAT BASIC 3.33. *)

VAR_GLOBAL CONSTANT
    STRING_LENGTH : INT := 250;
    LIST_LENGTH : INT := 250;
END_VAR

VAR_GLOBAL
    MATH : CONSTANTS_MATH;
    PHYS : CONSTANTS_PHYS;
    LANGUAGE : CONSTANTS_LANGUAGE;
    g_oscat_constants_loaded : BOOL := FALSE;
END_VAR

FUNCTION OSCAT_BASIC_Constants : BOOL
IF NOT g_oscat_constants_loaded THEN
    MATH.PI := 3.141592653589793;
    MATH.PI2 := 6.283185307179586;
    MATH.PI4 := 12.566370614359173;
    MATH.PI05 := 1.570796326794897;
    MATH.PI025 := 0.785398163397448;
    MATH.PI_INV := 0.318309886183791;
    MATH.E := 2.718281828459045;
    MATH.E_INV := 0.367879441171442;
    MATH.SQ2 := 1.4142135623731;
    MATH.FACTS[0] := DINT#1;
    MATH.FACTS[1] := DINT#1;
    MATH.FACTS[2] := DINT#2;
    MATH.FACTS[3] := DINT#6;
    MATH.FACTS[4] := DINT#24;
    MATH.FACTS[5] := DINT#120;
    MATH.FACTS[6] := DINT#720;
    MATH.FACTS[7] := DINT#5040;
    MATH.FACTS[8] := DINT#40320;
    MATH.FACTS[9] := DINT#362880;
    MATH.FACTS[10] := DINT#3628800;
    MATH.FACTS[11] := DINT#39916800;
    MATH.FACTS[12] := DINT#479001600;

    PHYS.C := 299792458.0;
    PHYS.E := 1.60217653E-19;
    PHYS.G := 9.80665;
    PHYS.T0 := -273.15;
    PHYS.RU := 8.314472;
    PHYS.PN := 101325.0;

    LANGUAGE.DEFAULT := 1;
    LANGUAGE.LMAX := 3;
    LANGUAGE.WEEKDAYS[1, 1] := 'Monday';
    LANGUAGE.WEEKDAYS[1, 2] := 'Tuesday';
    LANGUAGE.WEEKDAYS[1, 3] := 'Wednesday';
    LANGUAGE.WEEKDAYS[1, 4] := 'Thursday';
    LANGUAGE.WEEKDAYS[1, 5] := 'Friday';
    LANGUAGE.WEEKDAYS[1, 6] := 'Saturday';
    LANGUAGE.WEEKDAYS[1, 7] := 'Sunday';
    LANGUAGE.WEEKDAYS[2, 1] := 'Montag';
    LANGUAGE.WEEKDAYS[2, 2] := 'Dienstag';
    LANGUAGE.WEEKDAYS[2, 3] := 'Mittwoch';
    LANGUAGE.WEEKDAYS[2, 4] := 'Donnerstag';
    LANGUAGE.WEEKDAYS[2, 5] := 'Freitag';
    LANGUAGE.WEEKDAYS[2, 6] := 'Samstag';
    LANGUAGE.WEEKDAYS[2, 7] := 'Sonntag';
    LANGUAGE.WEEKDAYS[3, 1] := 'Lundi';
    LANGUAGE.WEEKDAYS[3, 2] := 'Mardi';
    LANGUAGE.WEEKDAYS[3, 3] := 'Mercredi';
    LANGUAGE.WEEKDAYS[3, 4] := 'Jeudi';
    LANGUAGE.WEEKDAYS[3, 5] := 'Vendredi';
    LANGUAGE.WEEKDAYS[3, 6] := 'Samedi';
    LANGUAGE.WEEKDAYS[3, 7] := 'Dimanche';
    LANGUAGE.WEEKDAYS2[1, 1] := 'Mo';
    LANGUAGE.WEEKDAYS2[1, 2] := 'Tu';
    LANGUAGE.WEEKDAYS2[1, 3] := 'We';
    LANGUAGE.WEEKDAYS2[1, 4] := 'Th';
    LANGUAGE.WEEKDAYS2[1, 5] := 'Fr';
    LANGUAGE.WEEKDAYS2[1, 6] := 'Sa';
    LANGUAGE.WEEKDAYS2[1, 7] := 'Su';
    LANGUAGE.WEEKDAYS2[2, 1] := 'Mo';
    LANGUAGE.WEEKDAYS2[2, 2] := 'Di';
    LANGUAGE.WEEKDAYS2[2, 3] := 'Mi';
    LANGUAGE.WEEKDAYS2[2, 4] := 'Do';
    LANGUAGE.WEEKDAYS2[2, 5] := 'Fr';
    LANGUAGE.WEEKDAYS2[2, 6] := 'Sa';
    LANGUAGE.WEEKDAYS2[2, 7] := 'So';
    LANGUAGE.WEEKDAYS2[3, 1] := 'Lu';
    LANGUAGE.WEEKDAYS2[3, 2] := 'Ma';
    LANGUAGE.WEEKDAYS2[3, 3] := 'Me';
    LANGUAGE.WEEKDAYS2[3, 4] := 'Je';
    LANGUAGE.WEEKDAYS2[3, 5] := 'Ve';
    LANGUAGE.WEEKDAYS2[3, 6] := 'Sa';
    LANGUAGE.WEEKDAYS2[3, 7] := 'Di';
    LANGUAGE.MONTHS[1, 1] := 'January';
    LANGUAGE.MONTHS[1, 2] := 'February';
    LANGUAGE.MONTHS[1, 3] := 'March';
    LANGUAGE.MONTHS[1, 4] := 'April';
    LANGUAGE.MONTHS[1, 5] := 'May';
    LANGUAGE.MONTHS[1, 6] := 'June';
    LANGUAGE.MONTHS[1, 7] := 'July';
    LANGUAGE.MONTHS[1, 8] := 'August';
    LANGUAGE.MONTHS[1, 9] := 'September';
    LANGUAGE.MONTHS[1, 10] := 'October';
    LANGUAGE.MONTHS[1, 11] := 'November';
    LANGUAGE.MONTHS[1, 12] := 'December';
    LANGUAGE.MONTHS[2, 1] := 'Januar';
    LANGUAGE.MONTHS[2, 2] := 'Februar';
    LANGUAGE.MONTHS[2, 3] := 'Maerz';
    LANGUAGE.MONTHS[2, 4] := 'April';
    LANGUAGE.MONTHS[2, 5] := 'Mai';
    LANGUAGE.MONTHS[2, 6] := 'Juni';
    LANGUAGE.MONTHS[2, 7] := 'Juli';
    LANGUAGE.MONTHS[2, 8] := 'August';
    LANGUAGE.MONTHS[2, 9] := 'September';
    LANGUAGE.MONTHS[2, 10] := 'Oktober';
    LANGUAGE.MONTHS[2, 11] := 'November';
    LANGUAGE.MONTHS[2, 12] := 'Dezember';
    LANGUAGE.MONTHS[3, 1] := 'Janvier';
    LANGUAGE.MONTHS[3, 2] := 'Fevrier';
    LANGUAGE.MONTHS[3, 3] := 'Mars';
    LANGUAGE.MONTHS[3, 4] := 'Avril';
    LANGUAGE.MONTHS[3, 5] := 'Mai';
    LANGUAGE.MONTHS[3, 6] := 'Juin';
    LANGUAGE.MONTHS[3, 7] := 'Juillet';
    LANGUAGE.MONTHS[3, 8] := 'Aout';
    LANGUAGE.MONTHS[3, 9] := 'Septembre';
    LANGUAGE.MONTHS[3, 10] := 'Octobre';
    LANGUAGE.MONTHS[3, 11] := 'Novembre';
    LANGUAGE.MONTHS[3, 12] := 'Decembre';
    LANGUAGE.MONTHS3[1, 1] := 'Jan';
    LANGUAGE.MONTHS3[1, 2] := 'Feb';
    LANGUAGE.MONTHS3[1, 3] := 'Mar';
    LANGUAGE.MONTHS3[1, 4] := 'Apr';
    LANGUAGE.MONTHS3[1, 5] := 'May';
    LANGUAGE.MONTHS3[1, 6] := 'Jun';
    LANGUAGE.MONTHS3[1, 7] := 'Jul';
    LANGUAGE.MONTHS3[1, 8] := 'Aug';
    LANGUAGE.MONTHS3[1, 9] := 'Sep';
    LANGUAGE.MONTHS3[1, 10] := 'Oct';
    LANGUAGE.MONTHS3[1, 11] := 'Nov';
    LANGUAGE.MONTHS3[1, 12] := 'Dec';
    LANGUAGE.MONTHS3[2, 1] := 'Jan';
    LANGUAGE.MONTHS3[2, 2] := 'Feb';
    LANGUAGE.MONTHS3[2, 3] := 'Mrz';
    LANGUAGE.MONTHS3[2, 4] := 'Apr';
    LANGUAGE.MONTHS3[2, 5] := 'Mai';
    LANGUAGE.MONTHS3[2, 6] := 'Jun';
    LANGUAGE.MONTHS3[2, 7] := 'Jul';
    LANGUAGE.MONTHS3[2, 8] := 'Aug';
    LANGUAGE.MONTHS3[2, 9] := 'Sep';
    LANGUAGE.MONTHS3[2, 10] := 'Okt';
    LANGUAGE.MONTHS3[2, 11] := 'Nov';
    LANGUAGE.MONTHS3[2, 12] := 'Dez';
    LANGUAGE.MONTHS3[3, 1] := 'Jan';
    LANGUAGE.MONTHS3[3, 2] := 'Fev';
    LANGUAGE.MONTHS3[3, 3] := 'Mar';
    LANGUAGE.MONTHS3[3, 4] := 'Avr';
    LANGUAGE.MONTHS3[3, 5] := 'Mai';
    LANGUAGE.MONTHS3[3, 6] := 'Jun';
    LANGUAGE.MONTHS3[3, 7] := 'Jul';
    LANGUAGE.MONTHS3[3, 8] := 'Aou';
    LANGUAGE.MONTHS3[3, 9] := 'Sep';
    LANGUAGE.MONTHS3[3, 10] := 'Oct';
    LANGUAGE.MONTHS3[3, 11] := 'Nov';
    LANGUAGE.MONTHS3[3, 12] := 'Dec';
    LANGUAGE.DIRS[1, 0] := 'N';
    LANGUAGE.DIRS[1, 1] := 'NNE';
    LANGUAGE.DIRS[1, 2] := 'NE';
    LANGUAGE.DIRS[1, 3] := 'ENE';
    LANGUAGE.DIRS[1, 4] := 'E';
    LANGUAGE.DIRS[1, 5] := 'ESE';
    LANGUAGE.DIRS[1, 6] := 'SE';
    LANGUAGE.DIRS[1, 7] := 'SSE';
    LANGUAGE.DIRS[1, 8] := 'S';
    LANGUAGE.DIRS[1, 9] := 'SSW';
    LANGUAGE.DIRS[1, 10] := 'SW';
    LANGUAGE.DIRS[1, 11] := 'WSW';
    LANGUAGE.DIRS[1, 12] := 'W';
    LANGUAGE.DIRS[1, 13] := 'WNW';
    LANGUAGE.DIRS[1, 14] := 'NW';
    LANGUAGE.DIRS[1, 15] := 'NNW';
    LANGUAGE.DIRS[2, 0] := 'N';
    LANGUAGE.DIRS[2, 1] := 'NNO';
    LANGUAGE.DIRS[2, 2] := 'NO';
    LANGUAGE.DIRS[2, 3] := 'ONO';
    LANGUAGE.DIRS[2, 4] := 'O';
    LANGUAGE.DIRS[2, 5] := 'OSO';
    LANGUAGE.DIRS[2, 6] := 'SO';
    LANGUAGE.DIRS[2, 7] := 'SSO';
    LANGUAGE.DIRS[2, 8] := 'S';
    LANGUAGE.DIRS[2, 9] := 'SSW';
    LANGUAGE.DIRS[2, 10] := 'SW';
    LANGUAGE.DIRS[2, 11] := 'WSW';
    LANGUAGE.DIRS[2, 12] := 'W';
    LANGUAGE.DIRS[2, 13] := 'WNW';
    LANGUAGE.DIRS[2, 14] := 'NW';
    LANGUAGE.DIRS[2, 15] := 'NNW';
    LANGUAGE.DIRS[3, 0] := 'N';
    LANGUAGE.DIRS[3, 1] := 'NNO';
    LANGUAGE.DIRS[3, 2] := 'NO';
    LANGUAGE.DIRS[3, 3] := 'ONO';
    LANGUAGE.DIRS[3, 4] := 'O';
    LANGUAGE.DIRS[3, 5] := 'OSO';
    LANGUAGE.DIRS[3, 6] := 'SO';
    LANGUAGE.DIRS[3, 7] := 'SSO';
    LANGUAGE.DIRS[3, 8] := 'S';
    LANGUAGE.DIRS[3, 9] := 'SSW';
    LANGUAGE.DIRS[3, 10] := 'SW';
    LANGUAGE.DIRS[3, 11] := 'WSW';
    LANGUAGE.DIRS[3, 12] := 'W';
    LANGUAGE.DIRS[3, 13] := 'WNW';
    LANGUAGE.DIRS[3, 14] := 'NW';
    LANGUAGE.DIRS[3, 15] := 'NNW';

    g_oscat_constants_loaded := TRUE;
END_IF;

OSCAT_BASIC_Constants := TRUE;
END_FUNCTION

FUNCTION STATUS_TO_ESR : ESR_DATA
VAR_INPUT
    status : BYTE;
    adress : STRING(10);
    DT_in : DT;
    TS : TIME;
END_VAR
VAR
    Result : ESR_DATA;
END_VAR

IF status < BYTE#100 THEN
    Result.TYP := BYTE#1;
ELSIF status < BYTE#200 THEN
    Result.TYP := BYTE#2;
ELSE
    Result.TYP := BYTE#3;
END_IF;
Result.ADRESS := adress;
Result.DS := DT_in;
Result.TS := TS;
Result.DATA[0] := status;
STATUS_TO_ESR := Result;
END_FUNCTION

FUNCTION OSCAT_VERSION : DWORD
VAR_INPUT
    IN : BOOL;
END_VAR

IF IN THEN
    OSCAT_VERSION := DWORD#1325462400;
ELSE
    OSCAT_VERSION := DWORD#333;
END_IF;
END_FUNCTION

FUNCTION_BLOCK ESR_MON_B8
VAR_INPUT
    s0, s1, s2, s3, s4, s5, s6, s7 : BOOL;
    DT_in : DT;
END_VAR
VAR_INPUT CONSTANT
    a0, a1, a2, a3, a4, a5, a6, a7 : STRING(10);
END_VAR
VAR_OUTPUT
    ESR_Flag : BOOL;
END_VAR
VAR_IN_OUT
    ESR_Out : ARRAY[0..3] OF ESR_DATA;
END_VAR
VAR
    x0, x1, x2, x3, x4, x5, x6, x7 : BOOL;
    tx : TIME;
    cnt : INT;
END_VAR

tx := DWORD_TO_TIME(T_PLC_MS());
ESR_Flag := FALSE;
ESR_Out[3].TYP := BYTE#0;
ESR_Out[2].TYP := BYTE#0;
ESR_Out[1].TYP := BYTE#0;
ESR_Out[0].TYP := BYTE#0;
cnt := INT#0;

IF s0 <> x0 THEN
    IF s0 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a0;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x0 := s0;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s1 <> x1 AND cnt < INT#4 THEN
    IF s1 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a1;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x1 := s1;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s2 <> x2 AND cnt < INT#4 THEN
    IF s2 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a2;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x2 := s2;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s3 <> x3 AND cnt < INT#4 THEN
    IF s3 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a3;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x3 := s3;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s4 <> x4 AND cnt < INT#4 THEN
    IF s4 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a4;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x4 := s4;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s5 <> x5 AND cnt < INT#4 THEN
    IF s5 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a5;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x5 := s5;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s6 <> x6 AND cnt < INT#4 THEN
    IF s6 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a6;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x6 := s6;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s7 <> x7 AND cnt < INT#4 THEN
    IF s7 THEN
        ESR_Out[cnt].TYP := BYTE#11;
    ELSE
        ESR_Out[cnt].TYP := BYTE#10;
    END_IF;
    ESR_Out[cnt].ADRESS := a7;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    x7 := s7;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK ESR_MON_R4
VAR_INPUT
    R0, R1, R2, R3 : REAL;
    DT_in : DT;
END_VAR
VAR_INPUT CONSTANT
    a0, a1, a2, a3 : STRING(10);
    s0, s1, s2, s3 : REAL;
END_VAR
VAR_OUTPUT
    ESR_Flag : BOOL;
END_VAR
VAR_IN_OUT
    ESR_Out : ARRAY[0..3] OF ESR_DATA;
END_VAR
VAR
    x0, x1, x2, x3 : REAL;
    tx : TIME;
    cnt : INT;
    raw : DWORD;
END_VAR

tx := DWORD_TO_TIME(T_PLC_MS());
ESR_Flag := FALSE;
ESR_Out[3].TYP := BYTE#0;
ESR_Out[2].TYP := BYTE#0;
ESR_Out[1].TYP := BYTE#0;
ESR_Out[0].TYP := BYTE#0;
cnt := INT#0;

IF ABS(R0 - x0) > s0 THEN
    raw := REAL_TO_DW(X := R0);
    ESR_Out[cnt].TYP := BYTE#20;
    ESR_Out[cnt].ADRESS := a0;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    ESR_Out[cnt].DATA[0] := BYTE_OF_DWORD(IN := raw, N := BYTE#0);
    ESR_Out[cnt].DATA[1] := BYTE_OF_DWORD(IN := raw, N := BYTE#1);
    ESR_Out[cnt].DATA[2] := BYTE_OF_DWORD(IN := raw, N := BYTE#2);
    ESR_Out[cnt].DATA[3] := BYTE_OF_DWORD(IN := raw, N := BYTE#3);
    x0 := R0;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF ABS(R1 - x1) > s1 AND cnt < INT#4 THEN
    raw := REAL_TO_DW(X := R1);
    ESR_Out[cnt].TYP := BYTE#20;
    ESR_Out[cnt].ADRESS := a1;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    ESR_Out[cnt].DATA[0] := BYTE_OF_DWORD(IN := raw, N := BYTE#0);
    ESR_Out[cnt].DATA[1] := BYTE_OF_DWORD(IN := raw, N := BYTE#1);
    ESR_Out[cnt].DATA[2] := BYTE_OF_DWORD(IN := raw, N := BYTE#2);
    ESR_Out[cnt].DATA[3] := BYTE_OF_DWORD(IN := raw, N := BYTE#3);
    x1 := R1;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF ABS(R2 - x2) > s2 AND cnt < INT#4 THEN
    raw := REAL_TO_DW(X := R2);
    ESR_Out[cnt].TYP := BYTE#20;
    ESR_Out[cnt].ADRESS := a2;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    ESR_Out[cnt].DATA[0] := BYTE_OF_DWORD(IN := raw, N := BYTE#0);
    ESR_Out[cnt].DATA[1] := BYTE_OF_DWORD(IN := raw, N := BYTE#1);
    ESR_Out[cnt].DATA[2] := BYTE_OF_DWORD(IN := raw, N := BYTE#2);
    ESR_Out[cnt].DATA[3] := BYTE_OF_DWORD(IN := raw, N := BYTE#3);
    x2 := R2;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF ABS(R3 - x3) > s3 AND cnt < INT#4 THEN
    raw := REAL_TO_DW(X := R3);
    ESR_Out[cnt].TYP := BYTE#20;
    ESR_Out[cnt].ADRESS := a3;
    ESR_Out[cnt].DS := DT_in;
    ESR_Out[cnt].TS := tx;
    ESR_Out[cnt].DATA[0] := BYTE_OF_DWORD(IN := raw, N := BYTE#0);
    ESR_Out[cnt].DATA[1] := BYTE_OF_DWORD(IN := raw, N := BYTE#1);
    ESR_Out[cnt].DATA[2] := BYTE_OF_DWORD(IN := raw, N := BYTE#2);
    ESR_Out[cnt].DATA[3] := BYTE_OF_DWORD(IN := raw, N := BYTE#3);
    x3 := R3;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK ESR_MON_X8
VAR_INPUT
    s0, s1, s2, s3, s4, s5, s6, s7 : BYTE;
    DT_in : DT;
    Mode : BYTE := BYTE#3;
END_VAR
VAR_INPUT CONSTANT
    a0, a1, a2, a3, a4, a5, a6, a7 : STRING(10);
END_VAR
VAR_OUTPUT
    ESR_Flag : BOOL;
END_VAR
VAR_IN_OUT
    ESR_Out : ARRAY[0..3] OF ESR_DATA;
END_VAR
VAR
    x0, x1, x2, x3, x4, x5, x6, x7 : BYTE;
    tx : TIME;
    cnt : INT;
END_VAR

tx := DWORD_TO_TIME(T_PLC_MS());
ESR_Flag := FALSE;
ESR_Out[3].TYP := BYTE#0;
ESR_Out[2].TYP := BYTE#0;
ESR_Out[1].TYP := BYTE#0;
ESR_Out[0].TYP := BYTE#0;
cnt := INT#0;

IF s0 <> x0 AND ((s0 < BYTE#100) OR (s0 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s0, adress := a0, DT_in := DT_in, TS := tx);
    x0 := s0;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s1 <> x1 AND cnt < INT#4 AND ((s1 < BYTE#100) OR (s1 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s1, adress := a1, DT_in := DT_in, TS := tx);
    x1 := s1;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s2 <> x2 AND cnt < INT#4 AND ((s2 < BYTE#100) OR (s2 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s2, adress := a2, DT_in := DT_in, TS := tx);
    x2 := s2;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s3 <> x3 AND cnt < INT#4 AND ((s3 < BYTE#100) OR (s3 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s3, adress := a3, DT_in := DT_in, TS := tx);
    x3 := s3;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s4 <> x4 AND cnt < INT#4 AND ((s4 < BYTE#100) OR (s4 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s4, adress := a4, DT_in := DT_in, TS := tx);
    x4 := s4;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s5 <> x5 AND cnt < INT#4 AND ((s5 < BYTE#100) OR (s5 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s5, adress := a5, DT_in := DT_in, TS := tx);
    x5 := s5;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s6 <> x6 AND cnt < INT#4 AND ((s6 < BYTE#100) OR (s6 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s6, adress := a6, DT_in := DT_in, TS := tx);
    x6 := s6;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
IF s7 <> x7 AND cnt < INT#4 AND ((s7 < BYTE#100) OR (s7 < BYTE#200 AND Mode >= BYTE#2) OR (Mode = BYTE#3)) THEN
    ESR_Out[cnt] := STATUS_TO_ESR(status := s7, adress := a7, DT_in := DT_in, TS := tx);
    x7 := s7;
    cnt := cnt + INT#1;
    ESR_Flag := TRUE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK ESR_COLLECT
VAR_INPUT
    ESR_0, ESR_1, ESR_2, ESR_3, ESR_4, ESR_5, ESR_6, ESR_7 : ARRAY[0..3] OF ESR_DATA;
    rst : BOOL;
END_VAR
VAR_IN_OUT
    pos : INT;
END_VAR
VAR_OUTPUT
    ESR_OUT : ARRAY[0..31] OF ESR_DATA;
END_VAR
VAR CONSTANT
    max_in : INT := INT#3;
    max_out : INT := INT#32;
END_VAR
VAR
    cnt : INT;
END_VAR

IF rst THEN
    pos := INT#-1;
ELSE
    FOR cnt := INT#0 TO max_in DO
        IF ESR_0[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_0[cnt];
        END_IF;
        IF ESR_1[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_1[cnt];
        END_IF;
        IF ESR_2[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_2[cnt];
        END_IF;
        IF ESR_3[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_3[cnt];
        END_IF;
        IF ESR_4[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_4[cnt];
        END_IF;
        IF ESR_5[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_5[cnt];
        END_IF;
        IF ESR_6[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_6[cnt];
        END_IF;
        IF ESR_7[cnt].TYP > BYTE#0 THEN
            pos := INC1(X := pos, N := max_out);
            ESR_OUT[pos] := ESR_7[cnt];
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION_BLOCK
