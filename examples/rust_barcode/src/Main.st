(* A packing line's label check: the simulated scanner reads the label that %IW0 names, GS1_LABEL,
   a function block written in Rust, reads its fields, and the program decides whether the box may
   go on. *)
PROGRAM Main
VAR
    Label AT %IW0 : INT;           (* the label the scanner reads: 1 to 4, 0 for none *)
    Labels : ARRAY[1..4] OF STRING[64] := [
        '01095011015300031726123110ABC123',   (* the line's product, in date until 2026-12-31 *)
        '01095011015300031725060010OLD42',    (* the line's product, expired at the end of June 2025 *)
        '01095011015300041726123110ABC124',   (* a GTIN whose check digit is wrong *)
        '01050123456789001726123110XYZ9'];    (* another product *)
    Read : GS1_LABEL;
    Accepted AT %QX0.0 : BOOL;     (* the line's product, in date: the box goes on *)
    Expired AT %QX0.1 : BOOL;      (* past its expiry date *)
    Unreadable AT %QX0.2 : BOOL;   (* not read, or a GTIN whose check digit is wrong *)
    WrongProduct AT %QX0.3 : BOOL; (* another product than the line packs *)
    Problem AT %QW2 : INT;         (* GS1_LABEL's error: 0 when the label was read *)
END_VAR
VAR CONSTANT
    PRODUCT : STRING[14] := '09501101530003';  (* the GTIN this line packs *)
    TODAY : DATE := D#2026-11-20;               (* the day the line runs; a plant reads its clock *)
END_VAR
IF Label >= 1 AND Label <= 4 THEN
    Read(code := Labels[Label]);
    Problem := Read.error;
    Unreadable := Read.error <> 0 OR NOT Read.gtin_ok;
    WrongProduct := NOT Unreadable AND Read.gtin <> PRODUCT;
    Expired := NOT Unreadable AND Read.expiry < TODAY;
    Accepted := NOT Unreadable AND NOT WrongProduct AND NOT Expired;
ELSE
    Problem := 0;
    Unreadable := FALSE;
    WrongProduct := FALSE;
    Expired := FALSE;
    Accepted := FALSE;
END_IF;
END_PROGRAM
