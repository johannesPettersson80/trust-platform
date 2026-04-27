FUNCTION_BLOCK ClassicMerge
VAR
    ActiveLaneIdValue : INT;
    JamCountValue : INT;
END_VAR
METHOD PUBLIC Update : INT
VAR_INPUT
    Lane1Request : BOOL;
    Lane1Jam : BOOL;
    Lane2Request : BOOL;
    Lane2Jam : BOOL;
    Lane3Request : BOOL;
    Lane3Jam : BOOL;
    DownstreamReady : BOOL;
END_VAR
ActiveLaneIdValue := INT#0;
JamCountValue := INT#0;
IF Lane2Jam THEN JamCountValue := JamCountValue + INT#1; END_IF;
IF Lane1Jam THEN JamCountValue := JamCountValue + INT#1; END_IF;
IF Lane3Jam THEN JamCountValue := JamCountValue + INT#1; END_IF;
IF DownstreamReady AND Lane2Request AND NOT Lane2Jam THEN
    ActiveLaneIdValue := INT#2;
ELSIF DownstreamReady AND Lane1Request AND NOT Lane1Jam THEN
    ActiveLaneIdValue := INT#1;
ELSIF DownstreamReady AND Lane3Request AND NOT Lane3Jam THEN
    ActiveLaneIdValue := INT#3;
END_IF;
Update := ActiveLaneIdValue;
END_METHOD
PUBLIC PROPERTY JamCount : INT
GET
    JamCount := JamCountValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Merge : ClassicMerge;
    Active : INT;
END_VAR
Active := Merge.Update(Lane1Request := TRUE, Lane1Jam := FALSE, Lane2Request := TRUE, Lane2Jam := FALSE, Lane3Request := FALSE, Lane3Jam := FALSE, DownstreamReady := TRUE);
END_PROGRAM
