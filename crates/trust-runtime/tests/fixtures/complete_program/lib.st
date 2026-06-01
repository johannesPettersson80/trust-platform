NAMESPACE DemoLib
FUNCTION Add : DINT
VAR_INPUT
    A : DINT;
    B : DINT;
END_VAR
Add := A + B;
END_FUNCTION

FUNCTION SwapDint : BOOL
VAR_IN_OUT
    A : DINT;
    B : DINT;
END_VAR
VAR_TEMP
    temp : DINT;
END_VAR
temp := A;
A := B;
B := temp;
SwapDint := TRUE;
END_FUNCTION

FUNCTION_BLOCK FB_Accumulator
VAR_INPUT
    In : DINT;
END_VAR
VAR_OUTPUT
    Sum : DINT;
    StatusByte : BYTE;
    StatusBit : BOOL;
END_VAR
VAR
    status : WORD := WORD#16#1234;
END_VAR
Sum := Sum + In;
StatusBit := status.%X0;
StatusByte := status.%B1;
status.%B0 := BYTE#16#FF;
END_FUNCTION_BLOCK
END_NAMESPACE
