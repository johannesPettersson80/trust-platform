PROGRAM Main
VAR RETAIN
    retained : INT := INT#1;
END_VAR
VAR NON_RETAIN
    non_retained : INT := INT#2;
END_VAR
VAR
    unspecified : INT := INT#3;
END_VAR

retained := retained + INT#1;
non_retained := non_retained + INT#1;
unspecified := unspecified + INT#1;
END_PROGRAM
