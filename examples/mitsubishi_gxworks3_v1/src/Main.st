FUNCTION_BLOCK FB_EdgeBridge
VAR_INPUT
    Signal : BOOL;
END_VAR
VAR_OUTPUT
    Rising : BOOL;
    Falling : BOOL;
END_VAR
VAR
    UpEdge : DIFU;
    DownEdge : DIFD;
END_VAR

UpEdge(CLK := Signal, Q => Rising);
DownEdge(CLK := Signal, Q => Falling);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Bridge : FB_EdgeBridge;
    Pulse : BOOL;
    RiseSeen : BOOL;
    FallSeen : BOOL;
END_VAR

Bridge(Signal := Pulse);
RiseSeen := Bridge.Rising;
FallSeen := Bridge.Falling;
END_PROGRAM
