(* The truST HMI library (specification 39, §18 Phase 5): the program calls Hmi.Display in a task
   of its choice. After that task the runtime serves the screens: it copies the values they show,
   applies the operators' commands and reads the alarms, so the task's interval and priority are
   the HMI's, and nothing of the HMI runs between scans. *)
NAMESPACE Hmi

FUNCTION_BLOCK Display
VAR_OUTPUT
    (* How often the program has called the block; the runtime serves the screens after each task
       in which it grew. *)
    Calls : UDINT;
END_VAR
    Calls := Calls + 1;
END_FUNCTION_BLOCK

END_NAMESPACE
