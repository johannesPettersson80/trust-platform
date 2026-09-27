# Coherent symbolic Control Studio acquisition

This extends Spec 35 runtime scan recording and general-loop setup. It is a
truST acquisition contract, not a change to IEC language semantics.

A recording/inspection binding is `{id, iec_type, address?, symbol?}`. Exactly
one selector is required. `address` keeps the existing explicit non-overlapping
I/O rules. `symbol` names an actual program/FB member, without a manufactured
I/O address. Symbolic reading uses the same numeric/BOOL conversion as addressed reading:
BOOL, signed/unsigned integer types, REAL and LREAL. 64-bit integers must stay
within the exactly representable JSON-number range; otherwise quality is bad.
Gain-write restrictions do not apply to reading process signals.
Signal IDs and resolved selectors must be unique. Mixed addressed/symbolic
sets are supported, at most 32 channels in each set.

Viewer operation `control.signals.inspect` accepts `{loop_revision, bindings,
expected_program_revision?}`. Its response echoes exact bindings and returns
`program_revision`, `loop_revision`, `available`, `scan`, `runtime_time_ns`,
`captured_at_ms`, numeric `values`, `forced`, `quality` and an optional reason.
Quality is good/bad/forced/missing. The first request, or a changed binding set,
returns available:false until the next actual runtime scan. A single bounded
inspection watch is replaced by loop/binding identity; polling never grows a
watch collection. Stale expected program revisions are rejected.

Runtime end-of-scan publication passes the same I/O snapshot, actual variable
storage and runtime timestamp to acquisition. Values are selected directly
from that storage while publishing one coherent scan. The runtime does not
clone all storage or hash the whole program for every scan. Numeric values,
IEC types and forcing are checked at capture time. Bad/missing/forced samples
remain explicit evidence rather than becoming successful measurements.

`control.recording.start` retains the addressed request contract and adds an
optional `program_revision`. Any symbolic binding requires that revision and a
current coherent inspection of the identical selectors. Symbolic recording
uses the same runtime scan publisher and result order. If another observer
replaces the inspection before Start, the extension may retry at most five
Start attempts, each following a fresh coherent inspection pinned to the same
program and selectors. Retry only the explicit pre-start coherence rejection;
unknown errors, transport timeouts and lost acknowledgements do not authorize
another Start. Exhaustion reports unavailable without fabricated samples. Results echo selectors
and program identity and include per-value quality. Existing addressed clients
continue to work and may ignore the additional quality fields.

Program identity reuses the G08 compiled-metadata and runtime-session identity.
Restart, debugger reattachment and bytecode reload invalidate inspection and
interrupt active recordings. Old buffered evidence remains incomplete and
cannot be relabelled as the new program. Changed types and unavailable members
produce missing/bad quality; they never reuse an earlier value.

## Unconfirmed recording Start

If Start loses its acknowledgement after dispatch, retain the already-created
session/ID and attempt Stop for that ID. Do not replay Start. A typed client
failure carries the owned session and any actual stopped recording. Abort the
commissioning sequence before applying gains or entering Auto. Preserve native
sample/quality/completeness data and add the unconfirmed-start reason; a stopped
capture is not a successful requested Start or qualified commissioning run.
If Stop also fails, retain the session as unresolved so Stop/Cancel can retry
that same owner. Neither an empty placeholder nor a new Start substitutes for
that recovery.

## Recording panel close and reopen

Closing a Control Studio panel is not a recording cancellation. A pending Start,
acknowledged recording, or unresolved Stop remains owned by the extension with
its original project, runtime, loop, revision and recording ID. Closing the view
MUST NOT replay Start, release the buffer, replace the owner, or manufacture a
terminal result.

When the same loop is reopened, the extension reconnects the panel to that one
owner and shows its current state: `Starting recording`, `Recording at runtime`,
or the explicit unconfirmed-Start/Stop recovery reason. Stop remains available
for the retained owner where a Stop is still required. A completed returned
record is presented as its retained evidence, with its actual samples and
qualification limits unchanged. Opening another loop while this owner remains
active stays blocked with the matching-loop explanation.

If the extension or runtime lifetime has ended, no in-memory owner is presumed
to survive. The UI reports that recovery is unconfirmed and obtains terminal
evidence only from the runtime's scoped recording status/Stop contract; it never
creates a new recording or invents the missing result.

### Extension-host recovery owner

Before dispatching a recording Start, Control Studio persists one bounded owner
record scoped to the project, selected runtime, loop ID and loop revision. It
contains only the generated recording ID, the exact binding list, the source,
the expected program revision when one was acquired, and whether Start was
acknowledged. It contains no runtime token, endpoint credential, samples or
claimed terminal outcome. A successful Stop with a decoded terminal runtime
record clears that owner; a Start rejection known to occur before buffer
allocation also clears it.

After an extension-host restart, reopening the matching loop loads that owner
as **Recording recovery unconfirmed**. It disables a new Start and offers Stop
only for the retained recording ID. Stop reconnects to the currently selected
runtime and submits the exact retained ID and loop revision. It accepts a
terminal record only when its identity, binding order and program-revision
guards match the owner; then it displays the actual samples and clears the
owner. It never rebuilds a recording with a new ID, substitutes editor samples,
or releases a different buffer.

If the runtime is unavailable, has no matching retained recording, reports a
changed program/binding identity, or cannot return a valid terminal record, the
owner remains and the UI states why recovery is unconfirmed. The operator may
retry Stop after the same runtime is available. A runtime restart therefore
does not produce invented completion evidence or silently authorize a new
recording.
