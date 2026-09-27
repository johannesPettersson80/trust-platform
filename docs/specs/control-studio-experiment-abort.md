# Project-owned experiment abort policy

Companion to Control Studio specification35, general experiment lifecycle.
This is a truST control protocol contract, not an IEC language deviation.

A version2 loop may declare `experiment.abort` alongside project-scripted
`experiment.activation`. The abort declaration contains `symbol`, `address`,
`iecType:"BOOL"`, `direction:"write"` and `assertedValue:boolean`. The two
bindings must be distinct. Other experiment kinds cannot silently accept it.
The review shows the abort assertion as well as normal activation/restoration.
An already asserted abort is never automatically cleared by starting a test.

The start protocol adds optional
`abort_binding:{address,value_type:"BOOL",asserted_value:boolean}` for
`project_sequence`. The runtime validates both compiled BOOL bindings, absence
of forcing, write access and the initial opposite abort value before any write.
Physical approval covers the complete displayed plan, including this assertion.

Normal duration completion restores the activation value without asserting
abort. User Abort, runtime fault, stale-data failure and other failed terminations
assert the declared abort input before restoring activation. The ST program owns
the resulting actuator policy; e.g.normal completion may hold75% while an abort
latches heater output0. The runtime does not invent a controller-specific policy.

Status optionally reports
`abort_binding:{address,asserted_value,state,observed_scan}` with state
`not_requested`, `pending`, `observed` or `failed`. Restoration remains pending
until a later unforced scan observes both the asserted abort and restored
activation. Missing, mistyped, forced or unobserved data cannot confirm success.
Compatibility: absence of this binding preserves existing experiment behavior.
No hardware qualification follows from simulated/native tests alone.
