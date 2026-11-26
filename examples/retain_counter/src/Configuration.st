// The resource runs the part counter once a second.
CONFIGURATION RetainCounter
TASK Count (INTERVAL := T#1s, PRIORITY := 1);
PROGRAM Main WITH Count : PartCounter;
END_CONFIGURATION
