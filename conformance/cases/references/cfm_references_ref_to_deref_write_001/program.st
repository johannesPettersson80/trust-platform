PROGRAM Main
VAR
    target : INT := INT#5;
    ref_value : REF_TO INT := REF(target);
    observed : INT := INT#0;
    after_write : INT := INT#0;
END_VAR

observed := ref_value^;
ref_value^ := INT#8;
after_write := target;
END_PROGRAM
