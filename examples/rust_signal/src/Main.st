(* A tank's level sensor and a Modbus slave, each checked in Rust: MEDIAN5 drops the sensor's
   spikes, and MODBUS_CRC checks a serial frame before the program takes its value. *)
PROGRAM Main
VAR
    Readings : ARRAY[0..19] OF REAL := [50.2, 49.8, 50.1, 49.9, 50.0, 50.3, 49.7, 80.0, 50.1, 49.9,
                                        50.2, 49.8, 50.0, 50.1, 20.0, 49.9, 50.2, 49.8, 50.1, 50.0];
    Reading : INT;                 (* the reading the simulated sensor gives now *)
    Scans : INT;                   (* scans since the last reading: one every 5 scans, 0.5 s *)
    Filter : MEDIAN5;
    Raw AT %QD0 : REAL;            (* the sensor's reading *)
    Level AT %QD4 : REAL;          (* the median of its last five readings *)
    Mean AT %QD8 : REAL;           (* their average, for comparison *)
    Frame AT %IW0 : INT;           (* the frame that comes in: 1 as sent, 2 with a bit changed on the line *)
    Previous : INT;                (* the frame of the scan before *)
    Sent : ARRAY[0..31] OF BYTE := [16#01, 16#03, 16#02, 16#00, 16#2A, 16#39, 16#9B];
    Received : ARRAY[0..31] OF BYTE;
    Check : MODBUS_CRC;
    FrameOk AT %QX12.0 : BOOL;     (* the last frame's CRC was right *)
    Register AT %QW14 : INT;       (* the register value of the last frame the program trusted *)
    BadFrames AT %QW16 : INT;      (* frames refused for their CRC *)
END_VAR
(* The sensor: a new reading every 5 scans, which the filter takes once. *)
IF Scans = 0 THEN
    Raw := Readings[Reading];
    Filter(x := Raw);
    Level := Filter.y;
    Mean := Filter.mean;
    IF Reading >= 19 THEN Reading := 0; ELSE Reading := Reading + 1; END_IF;
END_IF;
IF Scans >= 4 THEN Scans := 0; ELSE Scans := Scans + 1; END_IF;

(* The serial line: a frame comes in when Frame changes. *)
IF Frame <> Previous THEN
    IF Frame = 1 OR Frame = 2 THEN
        Received := Sent;
        IF Frame = 2 THEN
            Received[4] := 16#2B;  (* one bit changed on the line: 16#2A became 16#2B *)
        END_IF;
        Check(frame := Received, length := 7);
        FrameOk := Check.ok;
        IF Check.ok THEN
            Register := BYTE_TO_INT(Received[3]) * 256 + BYTE_TO_INT(Received[4]);
        ELSE
            BadFrames := BadFrames + 1;
        END_IF;
    END_IF;
    Previous := Frame;
END_IF;
END_PROGRAM
