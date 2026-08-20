TYPE
    OPENOT_CONTROL_BLOCK_BUFFER : ARRAY[0..87] OF BYTE;
END_TYPE

FUNCTION_BLOCK OPENOT_WriteControlBlockFixture
VAR_OUTPUT
    Buffer : OPENOT_CONTROL_BLOCK_BUFFER;
    Length : UINT;
END_VAR
VAR
    Work : OPENOT_BYTE_BUFFER;
    Index : UINT;
    Ignored : UINT;
END_VAR

FOR Index := UINT#0 TO UINT#87 DO
    Work[Index] := BYTE#16#00;
END_FOR;

Work[0] := BYTE#16#4F;
Work[1] := BYTE#16#4F;
Work[2] := BYTE#16#54;
Work[3] := BYTE#16#32;
Work[4] := BYTE#16#02;
Work[5] := BYTE#16#03;

Ignored := OPENOT_WriteU32LE(Offset := UINT#8, Value := UDINT#7, Buffer := Work);
Ignored := OPENOT_WriteU32LE(Offset := UINT#12, Value := UDINT#4096, Buffer := Work);
Ignored := OPENOT_WriteU32LE(Offset := UINT#16, Value := UDINT#2, Buffer := Work);
Ignored := OPENOT_WriteU64LE(Offset := UINT#24, Value := ULINT#8192, Buffer := Work);
Ignored := OPENOT_WriteU64LE(Offset := UINT#32, Value := ULINT#6144, Buffer := Work);
Ignored := OPENOT_WriteU64LE(Offset := UINT#40, Value := ULINT#12, Buffer := Work);
Ignored := OPENOT_WriteU64LE(Offset := UINT#48, Value := ULINT#3, Buffer := Work);
Ignored := OPENOT_WriteU64LE(Offset := UINT#56, Value := ULINT#5, Buffer := Work);
Ignored := OPENOT_WriteU64LE(Offset := UINT#64, Value := ULINT#7000, Buffer := Work);

Work[72] := BYTE#16#64;
Work[73] := BYTE#16#65;
Work[74] := BYTE#16#66;
Work[75] := BYTE#16#68;
Work[76] := BYTE#16#61;
Work[77] := BYTE#16#73;
Work[78] := BYTE#16#68;
Work[79] := BYTE#16#31;

Work[80] := BYTE#16#6F;
Work[81] := BYTE#16#6C;
Work[82] := BYTE#16#64;
Work[83] := BYTE#16#68;
Work[84] := BYTE#16#61;
Work[85] := BYTE#16#73;
Work[86] := BYTE#16#68;
Work[87] := BYTE#16#31;

FOR Index := UINT#0 TO UINT#87 DO
    Buffer[Index] := Work[Index];
END_FOR;

Length := UINT#88;
END_FUNCTION_BLOCK
