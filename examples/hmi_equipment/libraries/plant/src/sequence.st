(* Sequences (specification trust-plant-library §9.4). FB_Sequence holds the lifecycle; an
   application sequence extends it, calls Execute from its body and writes its steps as a CASE on
   CurrentStep. Its outputs that command equipment are ordinary outputs wired to the devices' Auto inputs.
   Nothing is retained: after any restart a sequence is Idle. *)
NAMESPACE Plant

FUNCTION_BLOCK ABSTRACT FB_Sequence IMPLEMENTS ISequence
VAR_OUTPUT NON_RETAIN
    State : E_SequenceState;
    CurrentStep : INT;
    StepName : STRING[80];
    (* Time in the current step; it stands still while held. *)
    StepTime : TIME;
    (* What the sequence waits for, or why it failed. *)
    Message : STRING[120];
    (* The state in words, and which operator commands apply now. *)
    StateText : STRING[20] := 'Idle';
    CanStart : BOOL := TRUE;
    CanHold : BOOL;
    CanResume : BOOL;
    CanStop : BOOL;
    CanAbort : BOOL;
    CanReset : BOOL;
END_VAR
VAR NON_RETAIN
    Clock : FB_ScanClock;
END_VAR

(* The lifecycle of one scan; TRUE while Running, when the steps should advance. *)
METHOD PROTECTED Execute : BOOL
    Clock();
    IF State = E_SequenceState#Running OR State = E_SequenceState#Stopping THEN
        StepTime := StepTime + Clock.Elapsed;
    END_IF;
    Describe();
    Execute := State = E_SequenceState#Running;
END_METHOD

METHOD PROTECTED Describe
    CASE State OF
        E_SequenceState#Idle: StateText := 'Idle';
        E_SequenceState#Running: StateText := 'Running';
        E_SequenceState#Held: StateText := 'Held';
        E_SequenceState#Stopping: StateText := 'Stopping';
        E_SequenceState#Stopped: StateText := 'Stopped';
        E_SequenceState#Completed: StateText := 'Completed';
        E_SequenceState#Aborted: StateText := 'Aborted';
        E_SequenceState#Failed: StateText := 'Failed';
    END_CASE;
    CanStart := State = E_SequenceState#Idle;
    CanHold := State = E_SequenceState#Running;
    CanResume := State = E_SequenceState#Held;
    CanStop := State = E_SequenceState#Running OR State = E_SequenceState#Held;
    CanAbort := CanStop OR State = E_SequenceState#Stopping;
    CanReset := State = E_SequenceState#Stopped OR State = E_SequenceState#Completed
        OR State = E_SequenceState#Aborted OR State = E_SequenceState#Failed;
END_METHOD

(* Go to a step: its number and a name the operator reads. *)
METHOD PROTECTED GoTo
VAR_INPUT
    Number : INT;
    Name : STRING[80];
END_VAR
    CurrentStep := Number;
    StepName := Name;
    StepTime := T#0s;
    Message := '';
END_METHOD

(* Say what the step waits for. *)
METHOD PROTECTED Wait
VAR_INPUT
    Text : STRING[120];
END_VAR
    Message := Text;
END_METHOD

METHOD PROTECTED Complete
    IF State = E_SequenceState#Running THEN
        State := E_SequenceState#Completed;
        Message := '';
        Describe();
    END_IF;
END_METHOD

METHOD PROTECTED Fail
VAR_INPUT
    Text : STRING[120];
END_VAR
    IF State = E_SequenceState#Running OR State = E_SequenceState#Held
        OR State = E_SequenceState#Stopping THEN
        State := E_SequenceState#Failed;
        Message := Text;
        Describe();
    END_IF;
END_METHOD

(* The application's stop steps are done. *)
METHOD PROTECTED StopDone
    IF State = E_SequenceState#Stopping THEN
        State := E_SequenceState#Stopped;
        Describe();
    END_IF;
END_METHOD

(* Fails the sequence with Text when the current step has lasted longer than Limit. *)
METHOD PROTECTED StepTimeout : BOOL
VAR_INPUT
    Limit : TIME;
    Text : STRING[120];
END_VAR
    StepTimeout := State = E_SequenceState#Running AND StepTime > Limit;
    IF StepTimeout THEN
        Fail(Text := Text);
    END_IF;
END_METHOD

METHOD PUBLIC Start : BOOL
    Start := State = E_SequenceState#Idle;
    IF Start THEN
        State := E_SequenceState#Running;
        GoTo(Number := 0, Name := '');
        Describe();
    END_IF;
END_METHOD

METHOD PUBLIC Hold : BOOL
    Hold := State = E_SequenceState#Running;
    IF Hold THEN
        State := E_SequenceState#Held;
        Describe();
    END_IF;
END_METHOD

METHOD PUBLIC Resume : BOOL
    Resume := State = E_SequenceState#Held;
    IF Resume THEN
        State := E_SequenceState#Running;
        Describe();
    END_IF;
END_METHOD

METHOD PUBLIC Stop : BOOL
    Stop := State = E_SequenceState#Running OR State = E_SequenceState#Held;
    IF Stop THEN
        State := E_SequenceState#Stopping;
        Describe();
    END_IF;
END_METHOD

METHOD PUBLIC Abort : BOOL
    Abort := State = E_SequenceState#Running OR State = E_SequenceState#Held
        OR State = E_SequenceState#Stopping;
    IF Abort THEN
        State := E_SequenceState#Aborted;
        Message := 'Aborted by the operator';
        Describe();
    END_IF;
END_METHOD

METHOD PUBLIC Reset : BOOL
    Reset := State = E_SequenceState#Stopped OR State = E_SequenceState#Completed
        OR State = E_SequenceState#Aborted OR State = E_SequenceState#Failed;
    IF Reset THEN
        State := E_SequenceState#Idle;
        GoTo(Number := 0, Name := '');
        Describe();
    END_IF;
END_METHOD

METHOD PUBLIC GetState : E_SequenceState
    GetState := State;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
