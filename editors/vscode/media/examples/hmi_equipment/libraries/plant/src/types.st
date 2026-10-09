(* The Plant library's shared types (specification trust-plant-library §3–§9). *)
NAMESPACE Plant

(* Who commands a device (§3): the program in Auto, the operator in Manual. *)
TYPE E_Mode : (Auto, Manual, OutOfService); END_TYPE

(* A measurement's quality (§7.1). *)
TYPE E_Quality : (Good, Uncertain, Bad); END_TYPE

(* A sequence's state (§9.4). *)
TYPE E_SequenceState : (Idle, Running, Held, Stopping, Stopped, Completed, Aborted, Failed); END_TYPE

END_NAMESPACE
