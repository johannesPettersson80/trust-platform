# Connected P commissioning

This supplies the connected P adapter for the main Apply workflow. Other
controller families and physical commissioning retain their separate contracts.

## Commissioning operation sequence

1. Use the selected model and P configuration from Identify/Tune.
2. Verify runs the matched target-response cases and, for Auto commissioning,
   an additional actual-ST Manual-to-Auto initialization case. The initialization
   case uses the selected plant's operating point, the typed manual command,
   explicit simultaneous reference/mode events and the real candidate gain.
   Check first-output continuity, actual mode, gain and signal bounds.
   Record and check the declared fixed parameter values throughout each target
   response too; an ST overwrite cannot silently qualify a different controller.
3. The selected application mode is part of the reviewed target contract. Auto
   requires its actual-ST initialization verification; Manual staging remains a
   supported technical alternative where applicable. Changing this mode
   invalidates application eligibility, not the model or tuning.
4. Retain exact target, gain, reference, observation duration, transfer tolerance
   and recovery policy before the user's Apply action. The Validate page owns
   the action and required recovery presentation.

5. Start native recording. Stage and confirm the candidate gain in Manual.
6. Read fresh native transfer status. Require compatible typed bindings, the
   verified program/gain and the selected steady operating point. Compute
   `r_track=y+u/Kp`, round to the actual reference type and check its bounds.
   Do not add bias or silently change the controller law.
7. Transfer reference/tracking state and mode through `control.transfer.*`.
   Wait for its actual output-image confirmation. The default numerical
   continuity tolerance is `1e-6 * max(1, outputMaximum-outputMinimum)` in output
   units; retain it in the reviewed record and permit an explicitly configured value.
   An omitted tolerance uses that default; an explicit finite
   value from zero through the declared output span is in output units. Use
   the same tolerance for actual-ST initialization, live entry and recovery.
   Editing it invalidates qualification/review, and Save/reopen preserves it.
8. Apply the reviewed task reference through the same guarded boundary. This
   intentional reference step is distinct from the continuous Auto entry.
9. Observe for the declared duration using the existing native recorder. Assess
   the actual samples against the target deadline, error, process/output bounds,
   quality, mode and transfer evidence. Missing scans or an unconfirmed operation
   cannot pass. Display the recorded response and retain the outcomes.
   Retain the complete native recording, reference-change times and values;
   an editor-polled trace is not a substitute for that evidence.

10. On a passing observation, leave the verified controller running in Auto.
    Rollback reviews a return to Manual tracking the freshly observed output,
    restoration of the previous reference and the retained previous gain set.
    It does not silently reset the process or assume zero is safe.

Cancel must resolve the owned transfer and retain its real outcome; if Auto was
already entered, use the reviewed Manual tracking recovery. Interruption or
failure must retain evidence and distinguish unconfirmed recovery from success.
A lost parameter-application acknowledgement is not proof that staging failed.
Recovery must inspect the same program/binding and retained parameter revisions,
restore a confirmed owned candidate to its previous gain set, and keep recovery
unconfirmed for pending or conflicting values. Never retry Apply to resolve a
lost acknowledgement.
New work cannot bypass unresolved application ownership. Saved evidence is
historical and cannot restore write authority.
When recovery is unconfirmed, Cancel retries recovery on the original endpoint
and program identity, updates the retained outcome and releases dependent
controls only after confirmation. It must not repeat Auto entry or the target step.

A schema-v4 saved result may retain `commissioning`: the native recording,
original model/channel identities, transfer outcomes, final observed parameter
status, and frozen assessment `criteria`. Bound it to16MiB and12000 samples.
Reopening validates the recording and outcome identities and recomputes a claimed
passing assessment from those samples and criteria. It restores recorded evidence, never a runtime client, pending transaction
or Apply approval.
A reopened successful Auto study may offer Rollback only after a fresh review
of the running simulator, matching program/model/binding identities, exact
current candidate revision/values and retained previous gain revision/values.
The original reference must come from confirmed transfer evidence. Explain the
Manual tracking transfer and previous-gain restoration before confirmation.
Changed identity or intervening gain transactions prevent this historical
recovery path; reopening alone must never write or authorize Apply. Once a
fresh rollback is confirmed, its owner survives an unconfirmed recovery: retain
the failure, lock dependent controls and allow Cancel to retry only that Manual
recovery on the original runtime. Do not discard its owner after a failed write. A new Rollback action always
performs a fresh review. Once this campaign has confirmed Manual recovery,
report that it is already recovered; do not reinterpret Rollback as applying
the candidate again. Failed reviews leave no confirmable write review.

## Initial supported scope and acceptance

The connected adapter initially commissions the local simulator using a declared
P law and compiled numeric primary/manual-command types. Legacy missing type
metadata requires configuration; unsupported scaling, state or plant conditions
must be explained before writes. Simulation approval never authorizes physical I/O.

Prove both model entries on ordinary ST projects, including `u0=2,y0=10`, without
handwritten internal JSON or ST-generated analysis. Verify observed Auto,
reference/gain adoption, first-output continuity, target response, recording
retention and guarded rollback in the actual rendered Studio. Test cancellation,
stale program/binding/data, failed observation and failed recovery separately.
No claim that this adapter completes the other controller families or workflows.

## Later Manual staging

Keep older commissioning evidence when the engineer later verifies and stages
another gain in Manual. A fresh Rollback compares current native program,
binding, active values and retained previous values with the commissioning
record. Matching history uses the guarded commissioning recovery; a newer
Manual parameter transaction uses the native Manual parameter rollback review.
Saving/reopening must not confuse the old Auto result with this newer operation.
A native mode/epoch/value conflict still rejects the write at its task boundary.

## Closing an unresolved recovery

If a completed commissioning outcome has unconfirmed recovery, closing or
replacing its editor must attempt the already-owned recovery and retain the
result in that loop's existing saved-result history. Capture the old loop,
model and study before asynchronous work; another editor must not supply data
or receive the old outcome. Reopening that loop waits for its pending history
write. The stored outcome is evidence, never a serialized runtime client or
write approval. On the same running program, show the unresolved recovery,
keep dependent work blocked, and offer a fresh Rollback review/confirmation.
The completed failed recovery may reuse its original confirmed commissioning
metadata for this fresh review; insufficient metadata remains an explicit
unsupported recovery case. Do not pretend closing the panel confirms recovery.

An active Auto campaign that has started must also retain its terminal outcome
when its editor closes: request cancellation, await the existing campaign,
and save its real partial recording and recovery status under the old loop.
Retain plain `gainRecovery` evidence: the original native program/binding/active
revision and gain values, plus the reviewed candidate values. This is evidence
for a fresh native history comparison, not a saved review token. It permits
fresh Manual recovery after an interrupted first campaign even when final
parameter status was unavailable. A confirmed transfer still supplies the
original reference; incomplete metadata must be reported, never invented.
