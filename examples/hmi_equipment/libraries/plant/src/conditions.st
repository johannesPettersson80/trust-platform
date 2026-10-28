(* Condition sets (specification trust-plant-library §9.1): up to eight conditions with texts, a
   first-out text, and Engineer bypasses that end by themselves. Interlocks and permissives may
   be bypassed; protection may not. These are ordinary process conditions, not certified safety
   functions. *)
NAMESPACE Plant

FUNCTION_BLOCK ABSTRACT FB_ConditionSet
VAR_INPUT
    (* TRUE while the condition is healthy; an unused condition stays TRUE. *)
    Condition1 : BOOL := TRUE;
    Condition2 : BOOL := TRUE;
    Condition3 : BOOL := TRUE;
    Condition4 : BOOL := TRUE;
    Condition5 : BOOL := TRUE;
    Condition6 : BOOL := TRUE;
    Condition7 : BOOL := TRUE;
    Condition8 : BOOL := TRUE;
    Text1 : STRING[80];
    Text2 : STRING[80];
    Text3 : STRING[80];
    Text4 : STRING[80];
    Text5 : STRING[80];
    Text6 : STRING[80];
    Text7 : STRING[80];
    Text8 : STRING[80];
END_VAR
VAR_OUTPUT NON_RETAIN
    (* Every condition healthy or bypassed. *)
    Healthy : BOOL := TRUE;
    Active : BOOL;
    (* The first condition that became unhealthy, held until all are healthy again. *)
    Text : STRING[80];
    FirstOut : INT;
    States : ARRAY[1..8] OF BOOL;
    Bypassed : ARRAY[1..8] OF BOOL;
    BypassActive : BOOL;
    (* The conditions a panel shows: each with a text, and any that is not healthy. *)
    InUse : ARRAY[1..8] OF BOOL;
END_VAR
VAR PUBLIC RETAIN
    BypassTime : TIME := T#1h;
END_VAR
VAR NON_RETAIN
    BypassTimers : ARRAY[1..8] OF TON;
END_VAR

METHOD PROTECTED Evaluate
VAR
    Texts : ARRAY[1..8] OF STRING[80];
    Index : INT;
    AllHealthy : BOOL;
    AnyBypass : BOOL;
END_VAR
    States[1] := Condition1;
    States[2] := Condition2;
    States[3] := Condition3;
    States[4] := Condition4;
    States[5] := Condition5;
    States[6] := Condition6;
    States[7] := Condition7;
    States[8] := Condition8;
    Texts[1] := Text1;
    Texts[2] := Text2;
    Texts[3] := Text3;
    Texts[4] := Text4;
    Texts[5] := Text5;
    Texts[6] := Text6;
    Texts[7] := Text7;
    Texts[8] := Text8;
    AllHealthy := TRUE;
    AnyBypass := FALSE;
    FOR Index := 1 TO 8 DO
        InUse[Index] := Texts[Index] <> '' OR NOT States[Index];
        BypassTimers[Index](IN := Bypassed[Index], PT := BypassTime);
        IF BypassTimers[Index].Q THEN
            Bypassed[Index] := FALSE;
            BypassTimers[Index](IN := FALSE, PT := BypassTime);
        END_IF;
        IF Bypassed[Index] THEN
            AnyBypass := TRUE;
        ELSIF NOT States[Index] THEN
            AllHealthy := FALSE;
            IF FirstOut = 0 THEN
                FirstOut := Index;
            END_IF;
        END_IF;
    END_FOR;
    IF AllHealthy THEN
        FirstOut := 0;
    END_IF;
    Healthy := AllHealthy;
    Active := NOT AllHealthy;
    BypassActive := AnyBypass;
    IF FirstOut = 0 THEN
        Text := '';
    ELSIF Texts[FirstOut] = '' THEN
        Text := CONCAT('Condition ', INT_TO_STRING(FirstOut));
    ELSE
        Text := Texts[FirstOut];
    END_IF;
END_METHOD

(* Bypass one condition for BypassTime; a repeated bypass restarts the time. *)
METHOD PROTECTED BypassCondition : BOOL
VAR_INPUT
    Index : INT;
END_VAR
    BypassCondition := Index >= 1 AND Index <= 8;
    IF BypassCondition THEN
        Bypassed[Index] := TRUE;
        BypassTimers[Index](IN := FALSE, PT := BypassTime);
    END_IF;
END_METHOD

METHOD PROTECTED EndBypass : BOOL
VAR_INPUT
    Index : INT;
END_VAR
    EndBypass := Index >= 1 AND Index <= 8;
    IF EndBypass THEN
        Bypassed[Index] := FALSE;
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

(* Interlocks: wire Active and Text to a device's Interlock and InterlockText. *)
FUNCTION_BLOCK FB_Interlocks EXTENDS FB_ConditionSet IMPLEMENTS IBypass
    Evaluate();

METHOD PUBLIC Bypass : BOOL
VAR_INPUT
    Index : INT;
END_VAR
    Bypass := BypassCondition(Index := Index);
END_METHOD

METHOD PUBLIC RemoveBypass : BOOL
VAR_INPUT
    Index : INT;
END_VAR
    RemoveBypass := EndBypass(Index := Index);
END_METHOD
END_FUNCTION_BLOCK

(* Permissives: wire Healthy and Text to a device's Permissive and PermissiveText. *)
FUNCTION_BLOCK FB_Permissives EXTENDS FB_ConditionSet IMPLEMENTS IBypass
    Evaluate();

METHOD PUBLIC Bypass : BOOL
VAR_INPUT
    Index : INT;
END_VAR
    Bypass := BypassCondition(Index := Index);
END_METHOD

METHOD PUBLIC RemoveBypass : BOOL
VAR_INPUT
    Index : INT;
END_VAR
    RemoveBypass := EndBypass(Index := Index);
END_METHOD
END_FUNCTION_BLOCK

(* Equipment protection: like interlocks, without bypass. *)
FUNCTION_BLOCK FB_Protection EXTENDS FB_ConditionSet
    Evaluate();
END_FUNCTION_BLOCK

END_NAMESPACE
