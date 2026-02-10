PROGRAM MainProgram
VAR
    Counter : INT;
    CounterLatched : INT;
END_VAR

CounterLatched := Counter;
Counter := Counter + 1;
END_PROGRAM
