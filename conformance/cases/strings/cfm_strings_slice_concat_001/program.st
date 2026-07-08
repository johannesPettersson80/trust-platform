PROGRAM Main
VAR
    text : STRING[16] := 'ABCD';
    second : CHAR := ' ';
    length : INT := INT#0;
    combined : STRING[16] := '';
END_VAR

second := text[2];
length := LEN(text);
combined := CONCAT(text, 'EF');
END_PROGRAM
