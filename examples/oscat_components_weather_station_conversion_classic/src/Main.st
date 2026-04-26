PROGRAM Main
VAR
    ConstantsReady : BOOL;
    WindMetersPerSecond : REAL := REAL#10.0;
    WindDegrees : INT := INT#90;
    BeaufortIndex : INT;
    WindDirection : STRING[3];
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();
BeaufortIndex := MS_TO_BFT(MS := WindMetersPerSecond);
WindDirection := DEG_TO_DIR(DEG := WindDegrees, N := INT#4, L := INT#1);
END_PROGRAM
