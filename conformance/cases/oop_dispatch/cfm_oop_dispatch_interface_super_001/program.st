INTERFACE ICounter
METHOD Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
END_METHOD
END_INTERFACE

CLASS Counter IMPLEMENTS ICounter
VAR PUBLIC
    value : INT := INT#0;
END_VAR
METHOD PUBLIC Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
value := value + delta;
Inc := value;
END_METHOD
END_CLASS

FUNCTION_BLOCK BaseFb
VAR PUBLIC
    count : INT := INT#10;
END_VAR
METHOD PUBLIC GetCount : INT
GetCount := count;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerivedFb EXTENDS BaseFb
VAR PUBLIC
    extra : INT := INT#3;
END_VAR
METHOD PUBLIC GetCount : INT
GetCount := count + extra;
END_METHOD
METHOD PUBLIC GetSuper : INT
GetSuper := SUPER.GetCount();
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    i : ICounter;
    c : Counter;
    fb : DerivedFb;
    out_iface : INT := INT#0;
    out_direct : INT := INT#0;
    out_override : INT := INT#0;
    out_super : INT := INT#0;
END_VAR

i := c;
out_iface := i.Inc(INT#1);
out_direct := c.Inc(INT#2);
out_override := fb.GetCount();
out_super := fb.GetSuper();
END_PROGRAM
