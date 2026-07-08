TYPE Cell :
STRUCT
    value : DINT;
END_STRUCT
END_TYPE

TYPE Row :
STRUCT
    cells : ARRAY[0..1] OF Cell;
END_STRUCT
END_TYPE

TYPE Matrix :
STRUCT
    rows : ARRAY[0..1] OF Row;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    matrix : Matrix;
    row_idx : DINT := DINT#1;
    cell_idx : DINT := DINT#0;
    observed : DINT := DINT#0;
END_VAR

matrix.rows[1].cells[0].value := DINT#41;
matrix.rows[row_idx].cells[cell_idx].value := matrix.rows[row_idx].cells[cell_idx].value + DINT#1;
observed := matrix.rows[1].cells[0].value;
END_PROGRAM
