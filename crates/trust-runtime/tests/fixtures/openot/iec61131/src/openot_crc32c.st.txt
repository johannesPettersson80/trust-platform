TYPE
    OPENOT_BYTE_BUFFER : ARRAY[0..255] OF BYTE;
END_TYPE

FUNCTION OPENOT_Crc32c : DWORD
VAR_INPUT
    Data : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Crc : DWORD;
    ByteValue : DWORD;
    ByteIndex : UINT;
    BitIndex : UINT;
END_VAR

Crc := DWORD#16#FFFFFFFF;

IF Length > UINT#0 THEN
    FOR ByteIndex := UINT#0 TO Length - UINT#1 DO
        ByteValue := BYTE_TO_DWORD(Data[ByteIndex]);
        Crc := Crc XOR ByteValue;

        FOR BitIndex := UINT#0 TO UINT#7 DO
            IF (Crc AND DWORD#16#00000001) <> DWORD#16#00000000 THEN
                Crc := SHR(Crc, UINT#1) XOR DWORD#16#82F63B78;
            ELSE
                Crc := SHR(Crc, UINT#1);
            END_IF;
        END_FOR;
    END_FOR;
END_IF;

OPENOT_Crc32c := NOT Crc;
END_FUNCTION
