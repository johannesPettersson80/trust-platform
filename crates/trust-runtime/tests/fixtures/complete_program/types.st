TYPE Color :
(
    Red := 0,
    Green := 1,
    Blue := 2
);
END_TYPE

NAMESPACE DemoTypes
TYPE Sensor :
STRUCT
    id : DINT;
    label : STRING[32];
    active : BOOL;
    value : REAL;
END_STRUCT
END_TYPE

TYPE ValueArray : ARRAY[0..3] OF DINT; END_TYPE
END_NAMESPACE

TYPE UniformTemporalPacket :
STRUCT
    elapsed AT %B0 : TIME;
    stamp AT %B8 : DT;
    calendarDay AT %B16 : DATE;
    civilClock AT %B24 : TOD;
END_STRUCT
END_TYPE
