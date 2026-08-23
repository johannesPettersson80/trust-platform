# Runtime Engine

**Status:** Implemented architecture. Production runtime executes STBC bytecode through the VM only; helper evaluation remains for bounded const/debug/config flows and the old evaluator internals are test-only.

### 1. Purpose

This document specifies the architecture for a portable Structured Text (ST) runtime capable of executing IEC 61131-3 compliant programs. The initial implementation targets desktop operating systems (Linux, Windows, macOS); embedded support is planned.

### 2. Design Goals

| Goal | Description |
|------|-------------|
| Portability | Single runtime codebase runs on desktop and embedded targets |
| Determinism | Predictable scan cycle execution suitable for automation |
| IEC Compliance | Align task scheduling and execution semantics with IEC 61131-3 Ed.3 |
| Simplicity | Minimal clock abstraction surface |
| Testability | Full runtime testable on desktop without hardware |

### 3. Architecture Overview

```
┌──────────────────────────────────────────────────┐
│               ST Program (Bytecode)              │
└──────────────────────┬───────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────┐
│                 ST Runtime Core                  │
│  ┌────────────┐ ┌──────────────────┐ ┌──────────────┐  │
│  │  Executor  │ │Resource Scheduler│ │ Timer System │  │
│  └────────────┘ └──────────────────┘ └──────────────┘  │
│  ┌────────────────────────────────────────────┐  │
│  │            Process Image                   │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────┬───────────────────────────┘
                       ▼
┌──────────────────────────────────────────────────┐
│                 Clock Trait                      │
└───────────┬──────────────────────┬───────────────┘
            ▼                      ▼
┌───────────────────┐    ┌───────────────────┐
│     StdClock      │    │   ManualClock     │
│ (Linux/Win/Mac)   │    │   (Tests/Sim)     │
└───────────────────┘    └───────────────────┘
```

### 4. Clock Abstraction Layer

#### 4.1 Rationale

The runtime requires only a monotonic clock and a way to sleep until a deadline. Rather than abstracting entire operating systems, we abstract only what the scheduler actually uses. This keeps the abstraction minimal and each clock implementation small.

#### 4.2 Clock Trait Definition

```rust
pub trait Clock: Send + Sync + 'static {
    /// Returns monotonic time for scheduling (nanosecond Duration).
    fn now(&self) -> Duration;

    /// Sleeps until a target time. Used only by real resource threads.
    fn sleep_until(&self, deadline: Duration);

    /// Wake any sleepers (best-effort).
    fn wake(&self) { /* optional */ }
}
```

The runtime scheduler uses a `Clock` for time and pacing. Thread creation and mutexing are handled by Rust’s standard library, keeping the abstraction surface minimal.

#### 4.3 Why Only a Clock

| Operation | Justification |
|-----------|---------------|
| `now` | Required for scan cycle timing and IEC timers (TON, TOF, TP) |
| `sleep_until` | Paces resource cycles in real threads |
| `wake` | Allows clean shutdown of resource threads |

Notably absent: file I/O (bytecode loaded at init), networking (handled separately via I/O abstraction), explicit mutex APIs (runtime uses `RwLock`/`Mutex` internally), dynamic allocation in hot path.

### 5. Clock Implementations

#### 5.1 StdClock (Desktop)

**Targets:** Linux, Windows, macOS

**Implementation:** Uses Rust standard library (`Instant`, `thread::sleep`).

```rust
pub struct StdClock {
    start: Instant,
}

impl Clock for StdClock {
    fn now(&self) -> Duration {
        let elapsed = self.start.elapsed();
        let nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        Duration::from_nanos(nanos)
    }

    fn sleep_until(&self, deadline: Duration) {
        let now = self.now();
        let delta = deadline.as_nanos() - now.as_nanos();
        if delta <= 0 {
            return;
        }
        let delta = u64::try_from(delta).unwrap_or(u64::MAX);
        thread::sleep(std::time::Duration::from_nanos(delta));
    }
}
```

**Justification:** Rust’s standard library already abstracts Linux/Windows/macOS differences. Task priority is enforced by the runtime scheduler; OS thread priority is best-effort only and may be ignored.

#### 5.2 ManualClock (Tests)

Deterministic clock for unit tests and simulation. Time advances explicitly; no real sleeping occurs. Used by scheduler tests and trace reproducibility checks.
Advancement uses saturating signed-nanosecond arithmetic: a delta beyond the
representable `Duration` range clamps to the corresponding bound and never
wraps or panics. Explicit `set_current_time` remains the separate mechanism for
placing a test clock at an earlier instant.

#### 5.2.1 Runtime logical time and non-pacing tick

`StdClock::now` exposes nondecreasing elapsed monotonic time. No strict progress
or minimum resolution is guaranteed between adjacent samples.

`ResourceRunner::tick` is one non-pacing scheduler step. It samples the
injected clock and executes one resource cycle without calling
`Clock::sleep_until`; pacing and deadline sleeps belong to the spawned resource
loop. A `ManualClock` therefore remains fixed until explicitly advanced.

The zero-argument truST compatibility call `TIME()` returns the runtime's
current injected logical elapsed time as a `TIME` value. It does not read civil
wall time. Harness and simulation advances affect the result only through the
runtime or manual clock.

Periodic task readiness likewise uses only the injected clock. A direct tick
with a fixed manual clock does not make the task ready; advancing that clock to
the task deadline makes one activation eligible under the readiness contract
below.

`CURRENT_DT()` is separate. It samples the host `SystemTime` once per call and
returns the UTC Unix timestamp as timezone-naive `DT` ticks with epoch
`1970-01-01T00:00:00Z` and fixed one-millisecond resolution. Positive
sub-millisecond fractions are truncated. Tick `0` through `i64::MAX` are
accepted; a pre-epoch or larger host value returns `RuntimeError::Overflow`.
Local timezone and DST are not applied.

The injected runtime/manual clock, scheduler scaling, simulation time, and
replay time do not control `CURRENT_DT`, and its samples are not made
monotonic. A host-clock rollback may therefore produce a lower later value.
Deterministic replay excludes programs that read `CURRENT_DT` unless the
environment controls the host clock. The complete callable contract is in
`docs/specs/07-standard-functions.md#host-civil-clock-current_dt`.

#### 5.3 Embedded Clock (Planned)

An RTOS-backed clock (e.g., FreeRTOS) is planned for embedded targets. The runtime core remains unchanged; only the `Clock` implementation differs.

### 6. Runtime Components

#### 6.1 Executor

Interprets compiled ST bytecode. Operates on the process image. Pure computation with no platform dependencies.

**Design decisions:**
- Stack-based bytecode VM (simpler than register-based)
- No heap allocation during execution (predictable timing)
- All state in process image (inspectable, serializable)

#### 6.2 Task Manager (Resource Scheduler)

Implements IEC 61131-3 task scheduling and program organization unit (POU) associations (IEC 61131-3 Ed.3, §6.8.2; Tables 62–63).

Each IEC **resource** runs inside a dedicated scheduler loop. The scheduler is executed on an OS thread started via `std::thread::spawn`; IEC tasks are *not* OS threads.

**IEC task model:**
- Tasks are periodic (INTERVAL) or event-driven (SINGLE rising edge). (IEC 61131-3 Ed.3, §6.8.2 a–b)
- If INTERVAL is non-zero, periodic scheduling occurs only while SINGLE is 0. (IEC 61131-3 Ed.3, §6.8.2 b)
- If INTERVAL is zero, no periodic scheduling occurs. (IEC 61131-3 Ed.3, §6.8.2 b)
- PRIORITY establishes scheduling order with 0 as highest priority and larger numbers as lower priority. (IEC 61131-3 Ed.3, §6.8.2 c; Table 63)
- A program with no task association executes once per resource cycle at the lowest priority. (IEC 61131-3 Ed.3, §6.8.2 d)
- A function block instance associated with a task executes only under that task, independent of program evaluation rules. (IEC 61131-3 Ed.3, §6.8.2 e)

**Scheduling policy (implementer choice permitted by IEC 61131-3, §6.8.2 c):**
- Deterministic, non-preemptive, fixed-priority scheduling per resource.
- Ready tasks at the same priority run in FIFO order by longest waiting time.
- Event tasks are edge-detected on the SINGLE input and enqueue one activation per rising edge.

```rust
pub struct TaskConfig {
    pub name: String,
    pub interval: Duration,      // INTERVAL; zero disables periodic scheduling
    pub single: Option<String>,  // SINGLE variable name (event + gating)
    pub priority: u32,           // 0 = highest priority per IEC 61131-3
    pub programs: Vec<ProgramId>,
    pub fb_instances: Vec<ValueRef>,
}

pub struct ResourceRunner<C: Clock + Clone> {
    runtime: Runtime,
    clock: C,
    cycle_interval: Duration,
}

impl<C: Clock + Clone> ResourceRunner<C> {
    pub fn tick(&mut self) -> Result<(), RuntimeError> {
        // single deterministic cycle (tests)
        Ok(())
    }

    pub fn spawn(self, name: &str) -> ResourceHandle<C> {
        // start dedicated OS thread
    }
}
```

**Implementation notes:**
- The SINGLE input is sampled from the current variable state; a transition 0 -> 1 enqueues exactly one activation.
- On task registration, the runtime initializes the previous SINGLE value to avoid a spurious edge on the first cycle.
- Periodic scheduling uses `Clock::now()` and the task interval (nanosecond Duration).
- Task readiness and elapsed-time accounting use only the injected monotonic
  `Clock`; the exact readiness, discontinuity, and overrun contract is defined
  below. Civil wall-clock changes do not make a task ready.
- Inputs are latched at the start of each scheduler cycle; outputs are committed after all ready tasks complete.
- The maximum number of tasks per resource and minimum interval resolution are implementer-specific and are reported by the runtime configuration.
- The resource loop maintains a `RUNNING/FAULT/STOPPED` state and halts on faults.

##### Task readiness and overrun accounting

For each scheduler sample, the portable readiness boundary observes one
`SINGLE` value and one logical `now`. A FALSE-to-TRUE `SINGLE` transition makes
the task due at `now`, updates the saved edge state, and does not advance the
periodic baseline or overrun count. A held TRUE value does not retrigger and
suppresses periodic readiness; a subsequent FALSE sample only rearms the next
rising edge. A non-positive interval does not schedule periodic work. Valid
IEC task configuration uses zero for disabled periodic scheduling and a
positive duration for periodic scheduling; a negative portable metadata value
is conservatively treated as disabled. (IEC 61131-3 Ed.3 section 6.8.2 a-b.)

With `SINGLE` FALSE and a positive interval, a task is not due until `now` is
at least `last_run + interval`. A backward logical-clock sample does not move
`last_run`, create an overrun, or make the task due; periodic readiness resumes
only when a later sample reaches the next deadline relative to the unchanged
baseline. When exactly one interval is due, the returned due time is that
nominal deadline, the missed-interval count is zero, and `last_run` advances to
the sampled `now`.

If a forward jump spans `n > 1` complete intervals, the scheduler emits one
activation rather than replaying work. Its due time remains the first nominal
deadline, the per-sample missed count is `n - 1`, the cumulative overrun count
adds that value with saturation, and `last_run` advances to `now`. Deadline
addition also saturates at the representable signed-nanosecond bound. Event
readiness never changes those periodic counters. These due-time and accounting
rules are truST bookkeeping beyond the IEC task-trigger definition; they make
host-pause, simulation, and test-clock behavior deterministic. They are
specified here as a truST runtime contract, not as an IEC deviation.

##### Resource-runner cycle transaction

One resource tick samples the current injected clock and task inputs, then
executes each due periodic or rising-edge event program once in the
deterministic scheduler order. In the reviewed combined case, the initial
zero-time tick executes neither program, the ten-millisecond tick executes the
periodic program and one rising-edge event activation, and the following
ten-millisecond tick executes only the periodic program after the event input
falls.

A backward manual-clock sample does not replay periodic work or create an
overrun. A later sample that reaches the next deadline resumes from the
unchanged periodic baseline. A forward jump spanning multiple complete
intervals executes the periodic program once and records the remaining
intervals as missed, as defined by the readiness contract above.

If due task execution returns an error, the tick returns that typed error and
latches the runtime fault state. A later tick rejects as
`RuntimeError::ResourceFaulted` until an explicit recovery transition clears or
replaces that state. This focused contract does not assert rollback of work
completed before the fault.

For the same source, initial runtime state, manual-clock samples, and input
transitions, the ordered `RuntimeEvent` vector is reproducible. Event payload
debug formatting, wall-clock timing, OS-thread scheduling, and cross-version
trace compatibility are outside this equality contract.

On Unix, the runtime's graceful-shutdown signal set is exactly `SIGINT` and
`SIGTERM`. Signals outside that set are not translated into a runtime stop by
the installed signal source. Their operating-system disposition is outside
this runtime contract.

#### 6.3 Timer System

Implements IEC 61131-3 timers: TON (on-delay), TOF (off-delay), TP (pulse).

All timers use `Clock::now()` for elapsed time calculation. Timer instances are evaluated when their owning program or task-associated function block executes; no background threads or interrupts are required.

The normative TP, TON, TOF, and `*_LTIME` scan-step state machines are defined
in [Standard Function Blocks, section 5](08-standard-function-blocks.md#5-timer-function-blocks-table-46-figure-15).
The implementation-owned clock, call, preset, and restart boundaries are
recorded in `docs/IEC_DECISIONS.md`. Timer traces observe executed calls only;
they do not claim that outputs change continuously between scan steps.

##### Timer Restart and Time-Base Contract

A newly constructed runtime starts its monotonic time at zero. An in-process
warm or cold restart does not create a new clock epoch: it preserves the
runtime's current monotonic time while reinitializing non-retained timer
instances and task timing baselines at that value. The first executed timer
call after restart contributes zero elapsed time. Subsequent elapsed time is
measured from the preserved post-restart baseline, so a restart cannot make the
runtime clock move backward or charge pre-restart elapsed time to a new timer
instance.

#### 6.4 Process Image

Memory-mapped area for inputs (%I), outputs (%Q), and markers (%M).

```rust
pub struct IoInterface {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
}
```

Sizes are derived from compiled program metadata at load time. On embedded targets, static sizing may be used, but the logical model remains the same.

The process image is owned by a single resource thread; no internal locking is required. Cross-resource data sharing is synchronized through the configuration-level shared globals lock (see 6.7). External I/O exchange (Modbus, etc.) reads/writes to this image at cycle boundaries.

##### Process-image bounds, reads, snapshots, and metadata application

Each `%I`, `%Q`, and `%M` byte area has an independent maximum length of
16 MiB. A resize, bytecode metadata declaration, source binding, or concrete
write whose addressed window exceeds that limit is rejected before allocating
or mutating the affected area. A failed metadata application is transactional:
resource selection, task/program references, all three area lengths, and I/O
bindings are validated before the executable state or process image changes.
The exact-resource and single-legacy-`RESOURCE` selection rules are defined in
section 6.7.1.

Reading an unallocated byte or the unallocated portion of a multi-byte value
returns zero bytes without growing the area. An unallocated fixed-width string
window therefore reads as an empty string. A fixed-width string write whose
payload is larger than its declared byte window returns overflow before
allocation or partial mutation.

For an in-range concrete address, a successful typed write followed by a typed
read of the same address returns the written value. Bit access updates and
reads only the addressed bit within its byte. Word access uses the process
image's little-endian byte representation and returns the same 16-bit value.

An I/O snapshot is produced after output and marker bindings have been
committed for the cycle. Each entry preserves its area, complete address,
optional display name, optional driver/source description, and optional
declared type. Typed snapshot values are decoded according to that declared
type, including `REAL`, `TIME`, and fixed-width `STRING`; a conversion failure
is represented as an entry error rather than a fabricated untyped value.

##### Process-image interface and binding transaction

A new process-image interface has empty `%I`, `%Q`, and `%M` byte areas, no
bindings, and no hierarchical values. Resizing validates all three requested
area lengths before changing any area. Shrinking truncates only the removed
suffix; growing preserves the retained prefix and zero-initializes every new
byte. A failed resize leaves all three areas byte-for-byte unchanged.

Concrete bit, byte, word, double-word, and long-word access uses the selected
area and byte offset. Multi-byte values are little-endian. Bit writes preserve
the other seven bits in the addressed byte. A successful write changes no
other area. A wrong value kind, wildcard address, zero-length byte-string
window, overflowing address calculation, or window beyond the 16 MiB cap is
rejected before allocation or mutation.

A fixed-width string write accepts valid UTF-8 whose byte length does not
exceed the declared nonzero window. It writes the payload then zero-fills the
remaining window, removing bytes from a previous longer value. Reading stops
at the first NUL byte; invalid UTF-8 before that terminator is a type error.
Unallocated portions of an in-range read are zero-filled conceptually without
growing the process image.

Hierarchical addresses are stored outside the flat byte areas and are keyed by
their complete area, size, bit, and path identity. Reading an absent
hierarchical key fails rather than fabricating zero. Writing one hierarchical
key does not allocate a flat process-image byte or alter another hierarchical
key.

Bindings retain insertion order. Snapshot rows are partitioned into input,
output, and memory lists while retaining insertion order within each area.
Named bindings use the declared name as their display name; reference bindings
remain unnamed unless an explicit display name was supplied. Recomputing
binding provenance replaces every row's previous source with the callback
result, including clearing a prior source when the callback returns none. A
wildcard binding appears as unresolved and is never read as a concrete zero.

Start-of-cycle input synchronization is transactional across every eligible
`%I` and `%M` binding. The interface first reads and type-checks every source
value and resolves every destination. Only after the complete batch succeeds
are values committed to program storage. A read, conversion, undefined target,
or null-reference failure leaves all bound program values unchanged. `%Q`
bindings are ignored by this phase.

End-of-cycle output synchronization has the symmetric all-or-nothing rule for
eligible `%Q` and `%M` bindings. It resolves and converts every program value
into a staged process image before replacing output, memory, or hierarchical
state. Any missing value, null reference, conversion failure, bounds failure,
or type mismatch leaves all three interface states unchanged. `%I` bindings
are ignored by this phase.

##### Typed floating-point egress

At end-of-cycle synchronization, typed `%Q` and `%M` bindings declared as
`REAL` or `LREAL` accept only values that are finite at the declared width. A
value converted to `REAL` must also remain finite after basic-single
narrowing. The runtime preflights every eligible output and marker binding
before mutating either process-image area; any rejected value returns an
`IoDriver` error, leaves the entire pending `%Q`/`%M` image unchanged, and
prevents the normal driver-output commit. The runtime does not clamp,
normalize, substitute, or emit non-finite IEEE bits. Configured safe-state
handling after the resulting cycle fault is a separate policy boundary.

#### 6.5 I/O Drivers

I/O exchange is explicit and deterministic: inputs are read into the input image at the start of each resource cycle, and outputs are written after all ready tasks complete.
Marker bindings (`%M`) are synchronized with program storage at both cycle boundaries:
- Start of cycle: `%M` process image -> bound variables (same phase as `%I` input latch).
- End of cycle: bound variables -> `%M` process image (same phase as `%Q` output commit).

```rust
pub trait IoDriver: Send {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError>;
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError>;
    fn health(&self) -> IoDriverHealth { IoDriverHealth::Ok }
}
```

Multiple drivers may be composed (e.g., fieldbus + simulated I/O). The resource scheduler owns the driver(s) and invokes them at cycle boundaries.

Composed drivers retain their configured order. On a successful boundary, each
driver reached by the composition receives exactly one input-read attempt
before task execution and one output-write attempt after task execution. An
earlier driver error short-circuits that boundary. `%I` and `%M` bindings are
loaded before ready tasks run; `%Q` and `%M` bindings are committed after those
tasks finish; the cycle I/O snapshot is emitted only after that commit. Arrays,
structures, and function-block instance bindings use their flattened declared
layout. An unresolved `%I*`, `%Q*`, or `%M*` binding must be resolved by a
matching-area `VAR_CONFIG` entry before runtime construction and is rejected
in disallowed declaration sections or on an area mismatch.

The built-in driver registry exposes the stable canonical names `ethercat`,
`gpio`, `loopback`, `modbus-tcp`, `mqtt`, and `simulated` in sorted unique
order. Accepted aliases resolve to the canonical name before validation or
construction; this includes `sim`/`noop`, `modbus_tcp`, `mqtt-tcp`, and
`ether-cat`/`ecat`. The explicit driver name `none` selects no driver.
Driver lookup trims surrounding whitespace and is ASCII-case-insensitive.
Unknown and empty names are configuration errors. `none` is reserved and
cannot be replaced by a registered driver.

Registry validation and construction are separate boundaries. Validation
invokes the selected driver's parameter validator only; it must not construct
a driver, open a device/socket, start a worker, or perform a protocol
connection. Construction runs only after successful validation and returns the
canonical name in the built specification. A registered alias cannot shadow a
canonical name, and an alias whose target is absent does not create a usable
entry. Canonical enumeration never includes aliases, empty names, or `none`.

Operator-facing I/O provenance uses the same canonicalization and alias table
as driver selection. Internal `%M` bindings always report `Internal memory`.
With no enabled driver, an external binding has no asserted source. With
exactly one enabled driver, its protocol-specific source is shown; disabled
drivers do not affect that choice. With two or more enabled drivers, the
conservative label is `Multiple I/O drivers` because whole-image composition
cannot assign one driver to the row. Modbus labels use the direction-specific
configured base plus the addressed word offset, MQTT labels use the
direction-specific topic, and GPIO labels use the addressed bit number.
Provenance is a configuration projection and never claims a connection,
successful exchange, or healthy hardware.

The GPIO driver accepts `chardev`/`libgpiod` (defaulting to
`/dev/gpiochip0`) and the compatibility `sysfs` backend (defaulting to
`/sys/class/gpio`). Input entries require concrete `%IX` bit addresses and
output entries require concrete `%QX` bit addresses; wildcard, non-bit,
wrong-area, and nested addresses make driver construction fail. The current
construction order does not claim that backend configuration is side-effect
free before this mapping rejection. GPIO read or write failure sets faulted
driver health with the original error. A configured watchdog/fault safe-state
output is written through the GPIO driver and is successful only under the
safe-state confirmation rule below.

GPIO parameters themselves follow these fail-closed rules:

- the parameter root is a table; absent `inputs` or `outputs` means an empty
  list, while a present list and every list entry must have the declared TOML
  shape;
- the backend name is ASCII-case-insensitive: absent, `chardev`, and
  `libgpiod` select the character-device backend, while `sysfs` selects the
  compatibility backend; no other name is accepted;
- every entry has a string `address` and a nonnegative `u32` physical `line`;
  `pin` is a compatibility alias used only when `line` is absent;
- `invert` and output `initial` default to false and accept booleans, integer
  zero/nonzero, or strings `true`/`false`/`1`/`0`;
- input `debounce_ms` defaults to zero and accepts a nonnegative integer or
  unsigned decimal string; and
- one physical line may appear only once across all inputs and outputs, and one
  process-image bit may appear only once within its input or output list.

Parameter validation performs all mapping-shape and collision checks without
opening a GPIO device or exporting a sysfs line. A simple bit address maps byte
and bit directly, with bit numbers restricted to 0 through 7. Internal
process-image helpers return a configuration error for an out-of-range byte or
bit and never panic; writes change only the selected bit.

On input handoff, entries are evaluated in declaration order. Each raw value is
inverted when configured, then debounced, then written to its `%I` bit without
changing other bits. The first debounced sample establishes state. A change
before the configured delay retains the prior state; a change at or after the
delay is accepted. On output handoff, the selected `%Q` bit is inverted when
configured and sent only when it differs from the last successfully written
level. Failed writes are not memoized and are retried on the next handoff.

Any backend, process-image bounds, or malformed sysfs value failure
short-circuits the handoff and records faulted health containing the original
error. A later complete successful handoff restores healthy status. Sysfs input
values are exactly trimmed `0` or `1`; other content is rejected rather than
being interpreted as energized. Synthetic filesystem tests prove parsing and
sysfs file semantics only. They do not prove pin ownership, voltage, wiring,
kernel driver operation, or device-in-the-loop behavior; those claims require
the separately reviewed real-hardware case and artifact.

Process-image drivers must keep scan-cycle methods bounded. Drivers with
blocking wire protocols may own background workers, but the `IoDriver` boundary
remains the cycle handoff: `read_inputs` copies the latest worker snapshot or
returns the configured policy result, and `write_outputs` hands off the latest
desired output without waiting for protocol round trips. Worker health is
projected through the existing `IoDriverHealth` surface rather than a parallel
status model. Output handoff is level/latest-value semantics for `%Q`, not an
edge or pulse delivery guarantee.

##### Modbus default-fault and deadline integrity

The Modbus background worker uses a fixed scan-side deadline for input refresh
and output handoff. Missing that deadline applies the configured `on_error`
policy at the synchronous `IoDriver` boundary:

- With `fault`, an incomplete input refresh sets `Faulted`, records
  `modbus input refresh pending`, returns `IoTransport`, and leaves the caller's
  input bytes unchanged even when an older accepted snapshot is cached.
- With `warn` or `ignore`, the same incomplete input refresh sets `Degraded`,
  records the pending-refresh diagnostic, and returns success. If an accepted
  snapshot exists, it may be copied according to the normal snapshot-shaping
  rules; absence of a snapshot does not invent input bytes.
- With `fault`, an output handoff that remains incomplete at the deadline sets
  `Faulted`, records `modbus output handoff pending`, and returns `IoTransport`.
  With `warn` or `ignore`, it sets `Degraded` and returns success.
- A timed-out output was already accepted into the asynchronous level/latest
  handoff. Deadline reporting does not cancel that pending or in-flight output;
  a later worker result governs its eventual transport outcome.

The positive controls for this deadline contract are a completed default
register loopback through the worker and a runtime safe-state handoff confirmed
by the worker and observed at the Modbus peer. Cold-start coverage must exercise
all three policies before any first snapshot exists and must leave caller bytes
unchanged when no accepted snapshot is available.

These deadline rules do not replace the separate policy projection for a
completed transport operation. They define only the scan-side result while the
requested worker sequence remains incomplete. This is a truST Modbus runtime
contract outside IEC 61131-3, so it is neither an IEC decision nor an IEC
deviation.

Driver error handling is configurable per driver:
- `fault`: return an error and fault the resource.
- `warn`: keep the resource running; driver health becomes **degraded**.
- `ignore`: keep the resource running; error is suppressed (health may still degrade).

Driver health is exposed via `ctl status` and the TUI.

##### Safe-state handoff confirmation

Ordinary scan-output writes use the configured driver error policy and the
level/latest-value worker handoff described above. Applying a configured safe
state has a stronger success boundary:

- The runtime first applies the configured safe values to the output process
  image and then attempts the resulting output image on every configured I/O
  driver.
- A driver's safe-state attempt is confirmed only when `write_outputs` returns
  successfully and the driver's immediately observed health is `Ok`.
  `Degraded` or `Faulted` health means the physical handoff is unconfirmed,
  even when `on_error = "warn"` or `"ignore"` made the ordinary driver call
  return successfully.
- A worker-backed driver whose handoff is queued, reconnecting, timed out, or
  otherwise pending must therefore return a safe-state error at the runtime
  boundary. A later asynchronous delivery does not retroactively turn that
  failed attempt into a confirmed safe state.
- The runtime attempts the safe-state write on the remaining configured
  drivers after one driver fails, but returns an error for the overall
  operation. The first failure identifies the affected driver and cause.
- A deliberate stop with an unconfirmed configured safe state ends in
  `Faulted`, not `Stopped`, and exposes the failure through `last_error`. When
  safe-state application is part of fault handling, the original root fault is
  retained and the reported error/event identifies the safe-state failure.

This confirmation rule does not add an unbounded protocol wait. Worker-backed
drivers retain their existing bounded handoff deadline; exceeding that deadline
is an unconfirmed safe state. With an empty safe-state configuration, the
runtime still performs no physical output write as specified in section 6.6.
When no I/O driver is configured, applying values changes only the in-memory
process image and makes no physical-delivery claim.

The OPC UA client transport-to-worker event handoff is bounded. When its
fixed-capacity queue is full, publishing an additional sample, connection, or
session event returns failure immediately rather than blocking or allocating
an unbounded queue. After the worker drains pending events, the same sink can
accept a later event. A rejected event is not claimed delivered.

###### OPC UA server surface, security, and cold start

The runtime's OPC UA server publishes PLC snapshot variables with CurrentRead
access only. The periodic publisher refreshes those values from runtime
snapshots; client writes are rejected because no transactional write-back path
to PLC storage is defined. A future writable server surface requires an
explicit allowlist, authorization, type validation, scan-boundary application,
and visible write result before it may advertise CurrentWrite.

The server value projection is closed over `BOOL`; signed 16-, 32-, and 64-bit
integers; unsigned 16-, 32-, and 64-bit integers; `REAL`; `LREAL`; and
`STRING`. Each value retains the corresponding OPC UA Boolean, integer, Float,
Double, or String scalar. A declared enum exports its selected variant name as
an OPC UA String. Null, references, time values, arrays, structures, and other
unsupported runtime values are omitted rather than invented as another scalar
type.

The secure server profile defaults to `Basic256Sha256`,
`SignAndEncrypt`, and anonymous access disabled. Policy `None` is valid only
with message mode `None`; a signing or encryption policy requires a compatible
non-`None` mode. Configured authentication and certificate-trust rules are
enforced by the wire server before a client may read a node.

Server construction and listener startup do not depend on the first runtime
snapshot. Before a snapshot exists, nodes have no fabricated ready value.
After a snapshot becomes available, a reference client may read every exported
supported scalar with CurrentRead access. Loopback/reference-client proof
establishes interoperability for the exercised profile only; it does not
establish external-vendor or production-certificate interoperability.

##### Floating-point boundary admission policy

IEC 61131-3 Ed.3, Section 6.4.2.1, Table 10 defines `REAL` and `LREAL`
using the IEC 60559 basic single- and double-width formats and leaves results
involving infinity or not-a-number implementer-specific. Section 6.6.2.5.15,
Table 39 defines `IS_VALID` so program logic can distinguish finite values from
NaN and infinity. Those provisions do not define how an external host or
protocol admits a value into a PLC process image. truST therefore applies the
following fail-closed product contract:

| Boundary | Typed non-finite input | Required rejection effect |
|----------|------------------------|---------------------------|
| Typed `%I` snapshot | `REAL`/`LREAL` IEEE bits decode to NaN or either infinity | Return an I/O error before exposing a typed snapshot value; do not normalize or substitute a value. |
| Modbus `input_points` | An `f32` register value, or its scaled engineering value, is non-finite | Reject the complete mapped read before changing any byte of the caller's input image. Previously accepted process-image bytes remain unchanged. |
| MQTT `input_points` | An `f32` text, JSON, or binary payload, or its scaled engineering value, is non-finite | Reject the complete received payload batch before changing the caller's input image or the driver's last accepted mapped snapshot. A later valid payload must not expose values from the rejected batch. |
| OPC UA client points | A configured `Float`/`REAL` or `Double`/`LREAL` sample is non-finite | Mark the point faulted and retain the previous PLC value, as specified below. |
| ADS client/server points | A scalar or array `REAL`/`LREAL` value is non-finite | Reject before cache acceptance, write queuing, or PLC storage mutation, as specified below. |
| Runtime mesh subscription | Numeric conversion to the configured `REAL`/`LREAL` target is non-finite | Reject before queuing or applying the local update; retain the previous PLC value. |
| `hmi.write` | Text or numeric input for a declared `REAL`/`LREAL` target is non-finite or overflows the declared width | Return a failed response with `error_code = "runtime_non_finite_value"` before queuing the write or changing PLC storage. |
| Managed local debug write/force | Input for a declared `REAL`/`LREAL` address is non-finite or overflows the declared width | Return a failed DAP response and preserve both value and force state, as specified in 6.9.1. |
| File-backed retain save/load | A scalar, array element, or structure field is non-finite | Reject the complete save or load transaction, as specified in 6.7. |
| `simulation.toml` coupling threshold | The configured threshold is non-finite | Reject the configuration before activation, as specified in 6.9.2. |

For a declared `REAL`, validation occurs after any scaling or narrowing to IEC
basic single width; a finite wider intermediate that becomes infinite at that
width is rejected. `LREAL` is validated at basic double width. Rejection never
clamps, normalizes, or substitutes a default. Error-policy handling may decide
whether the resource faults or reports degraded health, but it cannot turn the
rejected value into an accepted process value.

Raw Modbus register mode, raw MQTT byte topics, and integer-address control
operations carry untyped bits and cannot classify an IEEE payload by
themselves. They do not bypass this contract: once those bits cross a declared
`REAL`/`LREAL` binding, the typed process-image snapshot or egress preflight
must reject a non-finite representation. This boundary policy does not change
in-process IEC arithmetic, `IS_VALID`, subnormal, or signed-zero semantics.

**Built-in drivers**

1. **Modbus/TCP**
- Default profile uses **input registers** (FC04) for input image.
- Default profile uses **multiple holding-register writes** (FC16) for output image.
- Explicit `io.toml` options can select `read_coils` (FC01),
  `read_discrete_inputs` (FC02), `read_holding_registers` (FC03),
  `read_input_registers` (FC04), `write_single_coil` (FC05),
  `write_single_register` (FC06), `write_multiple_coils` (FC15), or
  `write_multiple_registers` (FC16).
- Optional `input_points` and `output_points` map individual coil/register
  addresses to process-image offsets with `bool`, `u16`, `i16`, `u32`, `i32`,
  or `f32` types, linear `scale`/`offset`, and Modbus `byte_order`/`word_order`.
- Register payloads are big‑endian (high byte first).
- Coil payloads are Modbus-packed with the first process-image bit in the
  least-significant bit of the first byte.
- Point-map numeric values are stored in the runtime process image as
  little-endian bytes after scaling so `%IW`/`%ID` bindings read the same value
  that the map produced.
- Register quantity is derived from the process image size (`ceil(bytes / 2)`).
- Runtime exchange is worker-backed: scan-cycle reads/writes use bounded
  snapshot/handoff state while Modbus TCP connect/read/write round trips happen
  on the Modbus worker.
- A completed worker read preserves its typed protocol error at the scan-cycle
  boundary. In particular, a Modbus exception remains an `IoAddress` error and
  is not replaced by a generic unavailable-snapshot transport error.
- Function selection is closed over FC01, FC02, FC03, and FC04 for input and
  FC05, FC06, FC15, and FC16 for output. An unknown or direction-incompatible
  function is rejected during configuration.
- A coil point is boolean and does not accept numeric scaling. A register point
  uses a declared supported numeric type; scaling must be finite and invertible
  for the requested direction. Invalid point type, address, function, or
  scaling is rejected before the worker starts.
- A completed transport failure follows `on_error`: `fault` returns the typed
  error and faults health, while `warn` and `ignore` return degraded success.
  This policy never rewrites a Modbus exception into a generic transport error.
- Reconnect uses a non-zero bounded backoff and cannot spin on an unavailable
  endpoint. Dropping the driver requests worker shutdown and returns within the
  bounded driver-drop deadline even when the worker is awaiting its first
  response. This return-time contract does not claim that the detached worker
  thread has already joined; joined worker shutdown remains open proof debt.

###### Modbus declared-width floating-point finiteness

For an `f32`/`REAL` point, finiteness is checked again after scaling or inverse
scaling is narrowed from the internal `f64` calculation to the declared
`f32` width. A finite `f64` result that becomes non-finite at `f32` width is
rejected before process-image mutation or Modbus wire encoding; it must not
become an infinite `REAL` value or register payload. This is a truST Modbus
product and safety contract outside IEC 61131-3, not an IEC decision or
deviation.

###### Modbus and MQTT typed point-map contract

Typed point maps bind one protocol scalar to one bounded process-image scalar.
They do not reinterpret arbitrary trailing bytes, merge overlapping
definitions, or use a partially converted batch as PLC input.

The common scalar set is `bool`, `u16`, `i16`, `u32`, `i32`, and `f32`.
Documented IEC-style aliases (`boolean`, `word`, `int`, `dword`, `dint`, and
`real`) and explicit-width aliases select the same type case-insensitively.
Integer conversion rounds to the nearest integer with half-way values away
from zero, then checks the declared range. F32 conversion must remain finite
after narrowing. Inbound scaling is `engineering = raw * scale + offset`;
outbound scaling is `raw = (engineering - offset) / scale`. Scale must be
finite and non-zero and offset must be finite. Boolean points do not accept
scaling.

Process-image numeric values are little-endian and occupy exactly two or four
bytes according to their declared type. A Boolean occupies one selected bit;
its bit index is `0..=7`, and writes preserve every other bit in the byte.
Every offset-plus-width calculation is checked for arithmetic overflow and
image bounds before mutation. A failed conversion or range check leaves the
complete destination bytes unchanged.

Within one input or output map, two point definitions must not claim the same
protocol identity or overlapping process-image storage. Boolean bits in the
same byte are distinct storage only when their bit indices differ. Modbus
register ranges account for the type's complete one- or two-register width and
the selected function/table. MQTT identities use the exact normalized topic.
An input and output may use the same process-image offset because `%I` and
`%Q` are separate images. Ambiguous duplicates or overlaps are configuration
errors before a worker or transport is started.

For MQTT:

- a point topic is trimmed, non-empty, contains no control character, and is
  an exact publish/delivery topic rather than a `+` or `#` subscription
  filter;
- Boolean maps require `image_bit`; numeric maps reject it;
- payload formats are text, JSON, binary little-endian, and binary big-endian,
  including their documented aliases; omitted format means text;
- text and JSON numeric strings permit surrounding whitespace but must contain
  one complete finite scalar;
- JSON Boolean input accepts a Boolean, number, or recognized Boolean string;
  JSON numeric input accepts a number or numeric string;
- binary Boolean payloads contain exactly one byte and numeric binary payloads
  contain exactly the declared width; extra and truncated bytes are rejected;
  and
- output metric names are trimmed, non-empty, and free of control characters.

For Modbus:

- FC01/FC02 points are Boolean and FC03/FC04 points are numeric; FC05/FC15
  outputs are Boolean and FC06/FC16 outputs are numeric;
- omitted type defaults to Boolean for bit functions and U16 for register
  functions;
- FC06 accepts only one-register U16/I16 values;
- byte order applies inside every 16-bit register and word order applies
  between the two registers of a 32-bit value; both default to big-endian and
  accept the documented `big`/`be` and `little`/`le` aliases; and
- a decoded register scalar consumes exactly the declared register byte count.
  A truncated or oversized buffer is rejected rather than partially decoded.

These point-map rules are truST protocol/product behavior outside IEC
61131-3. Driver error policy may classify the resulting error, but cannot turn
an invalid configuration, malformed scalar, out-of-range value, or partial
batch into accepted PLC data.

2. **MQTT (baseline profile)**
- Topic bridge between broker payloads and process image bytes.
- `topic_in` payload bytes are copied into `%I` at cycle start.
- `%Q` output bytes are published to `topic_out` at cycle end.
- Optional `input_points` and `output_points` map typed scalar MQTT topics to
  process-image offsets with `bool`, `u16`, `i16`, `u32`, `i32`, or `f32`
  values, `text`/`json`/`binary_le`/`binary_be` payloads, and linear
  `scale`/`offset`.
- Numeric typed point values are stored in the runtime process image as
  little-endian bytes; binary MQTT payload endianness applies only to the MQTT
  payload.
- Optional Sparkplug B outbound node profile for typed `output_points`:
  `namespace = "spBv1.0"`, `spec_version = "3.0.0"`, required `group_id`, and
  required `edge_node_id`.
- Sparkplug mode publishes NBIRTH on MQTT session establishment, configures
  NDEATH as the MQTT last will, and publishes NDATA scalar metric payloads for
  typed output points.
- Sparkplug topics are
  `<namespace>/<group_id>/NBIRTH/<edge_node_id>`,
  `<namespace>/<group_id>/NDEATH/<edge_node_id>`, and
  `<namespace>/<group_id>/NDATA/<edge_node_id>`. Birth precedes data for each
  accepted session. Payloads use the Eclipse Tahu Sparkplug B protobuf metric
  representation for the supported scalar types. The configured profile
  requires `group_id`, `edge_node_id`, at least one output point, and no input
  points. Device topics, templates, aliases, and command subscriptions are
  non-goals rather than claimed supported behavior. Structural field-byte or
  substring assertions do not establish reference Tahu interoperability; a
  reference decoder remains required for that proof.
- A plain `mqtt://` endpoint is permitted only for loopback/local authority
  unless the explicit insecure-remote override is set. `mqtts://` implies TLS.
  TLS configuration requires a CA path; client certificate and key are an
  all-or-nothing pair; TLS-only fields are rejected when TLS is disabled.
  An empty ALPN list has no effect when TLS is disabled. Enabling TLS constructs
  the configured CA and optional mTLS transport, but construction alone is not
  evidence of a successful broker handshake.
- Runtime exchange is worker-backed: broker connect/poll/publish and
  reconnection happen on the MQTT worker, while scan-cycle reads/writes use
  bounded snapshot/handoff state.
- When a raw `topic_in` read drains multiple payloads in one worker operation,
  the newest drained payload is the authoritative process-image snapshot.
- Under the default `fault` policy, disconnected or stale MQTT input reads and
  bounded reads with no available snapshot return `IoFreshness`. Completed
  connection failures retain their connection context at the worker boundary;
  they are not replaced by a generic unavailable-snapshot message.
- The same default `fault` policy remains authoritative when a prior input
  snapshot exists but the requested refresh misses its scan deadline, when an
  output handoff misses its scan deadline, and when completed typed-output
  preflight fails, including Sparkplug NDATA preparation. Those failures set
  faulted health and return an error rather than collapsing to degraded
  success. A faulted stale-input read leaves the caller's input buffer
  unchanged. `warn` and `ignore` retain their documented degraded-success
  behavior. The focused stale-snapshot copy lock covers `warn`; an explicit
  `ignore` stale-copy matrix remains open.
  An output deadline does not itself cancel the already queued asynchronous
  output; later publication does not retroactively make the timed-out call
  successful.
- Reconnection is non-blocking; runtime cycle remains deterministic.
- Reconnection uses a non-zero bounded backoff and cannot busy-loop after a
  connection failure. Dropping the driver requests shutdown and returns within
  the bounded driver-drop deadline, including while a session connection is
  pending. This return-time contract does not claim that the detached worker
  thread has already joined; joined worker shutdown remains open proof debt.
- Security baseline rejects insecure remote brokers unless explicitly overridden.
- Sparkplug B non-goals in this profile: command subscriptions, device-level
  DBIRTH/DDATA topics, metric aliases, templates, and store-and-forward.

###### MQTT default-fault and latest-value integrity

The newest raw-payload and default-fault rules above are a truST runtime product
contract outside IEC 61131-3. Broker delivery, worker deadlines, cached
snapshots, and output preflight are not IEC language semantics and do not
belong in `IEC_DECISIONS.md` or `IEC_DEVIATIONS.md`.

3. **EtherCAT (backend v1)**
- Driver name: `ethercat`.
- Deterministic process-image mapping for module-chain profiles (including
  `EK1100` + digital I/O modules such as `EL1008` / `EL2008`).
- The default reviewed module-chain profile is `EK1100`, `EL1008`, and
  `EL2008`, with deterministic input/output byte lengths and channel order.
  An explicit hardware adapter name is accepted as configuration authority;
  it is not proof that the adapter exists.
- Startup discovery diagnostics emit discovered module summary and expected
  process-image sizes.
- Cycle-time health telemetry upgrades driver status to **degraded** when cycle
  read/write exceeds configured warning threshold.
- A read or write failure follows the configured fault/warn/ignore policy,
  except that a discovered process-image size mismatch is always faulted
  because continuing would reinterpret channel layout. Warn and ignore may
  keep the resource running only when the image shape remains valid, and must
  preserve the failure detail in degraded health.
- Non-mock adapters are backed by EtherCrab hardware transport on unix targets.
- Deterministic `adapter = "mock"` mode is available for CI/offline validation.
- Explicit v1 non-goals: no functional safety/SIL claims and no advanced motion
  profile support.

4. **OPC UA client floating-point ingress**
- A `Good` sample for a configured `Float`/`REAL` or `Double`/`LREAL` input
  point is accepted only when the value is finite.
- `NaN`, positive infinity, and negative infinity are rejected before the
  sample becomes an accepted cache value or is written to PLC variable
  storage.
- A rejected sample marks the affected point **faulted**, exposes diagnostic
  detail, and leaves the PLC target unchanged. The runtime does not clamp,
  normalize, or substitute a default value.
- A previously accepted finite value may remain visible for diagnostics, but
  a faulted point does not apply it as a fresh input. A later finite `Good`
  sample may recover through the normal subscription path.
- This rule is limited to OPC UA client scalar ingress. Scalar OPC UA egress
  is governed by the worker/cache integrity contract below. Arrays,
  structures, subnormal values, signed zero, and other protocol/API/retained
  ingress boundaries require their own reviewed contracts.

###### OPC UA client worker, cache, and scalar-output integrity

This is a truST OPC UA product contract outside IEC 61131-3. IEC defines the
PLC data types exposed at the runtime boundary, but it does not define OPC UA
session generations, subscription event authority, write acknowledgements, or
worker shutdown. These rules therefore belong here, not in
`IEC_DECISIONS.md` or `IEC_DEVIATIONS.md`.

The client cache is closed over the configured bindings. A subscription sample
from the active session may mutate cache or PLC input authority only when its
variable name, node ID, declared OPC UA data type, and access mode exactly
match one configured readable point. A current-session sample with an unknown
or mismatched identity is a visible validation fault and must not create a
dynamic point status, replace a configured point's value, or write PLC
storage.

Every connection or recovery candidate has a monotonically increasing local
session generation, and every callback sink created for that candidate carries
that generation. Only events from the active generation may change connection,
point, value, or freshness authority. A delayed sample, disconnect, or
session-closed callback from an older generation is ignored and cannot regress
a newer connected session. Within the active generation, accepted event time
and point freshness never move backward; an older event cannot replace a newer
value or connection/point state.

Each locally captured output intent carries a monotonically increasing write
generation. Capturing a changed finite value atomically replaces the queued
value for that point. A transport completion may remove the queued value or
change its point quality only when the completion still names the exact
generation that was transmitted. Completion of an older value cannot remove a
newer queued value or overwrite its pending or error quality. A correctly
correlated validation rejection terminally removes that exact generation,
faults the affected point visibly, and does not reconnect the otherwise
healthy session.

For scalar `Float`/`REAL` and `Double`/`LREAL` outputs, the value must be finite
at the declared width before it enters the pending-write cache. A non-finite
output cancels any older queued intent for that point, advances the local write
generation, reports an explicit point fault, and is never sent to the
transport. The queued-value baseline is cleared so a later finite value can be
queued normally. The runtime does not clamp, normalize, or substitute a
default.

At the OPC UA wire adapter boundary, a write response is successful only when
it contains exactly one status for every request, in request order, and every
status is `Good`. A missing, extra, or non-good result cannot be
reported to the worker as a successful batch. This is a fail-closed truST
adapter rule; it does not claim that IEC 61131-3 defines OPC UA service-result
semantics.

Worker shutdown or drop must wake an idle interval wait promptly rather than
sleeping for the configured tick interval, attempt transport disconnect, make
the shared connection and readable-point authority non-good, and join the
worker thread. Raw values may remain available for diagnostics. This local
wakeup guarantee does not claim cancellation of a transport call already in
flight.

Whether pending outputs replay after reconnect and how server source
timestamps are ordered remain unresolved product decisions. This contract does
not provide authority for tests that select those behaviors.

###### OPC UA client configuration, trust, and one-shot operation integrity

This is a truST OPC UA product and security contract outside IEC 61131-3.
Configuration, certificate stores, endpoint discovery, browsing, and OPC UA
service-response vectors are not IEC language semantics and therefore do not
belong in `IEC_DECISIONS.md` or `IEC_DEVIATIONS.md`.

An OPC UA client configuration contains at least one connection. Connection
names are trimmed, non-empty, and unique across the complete configuration.
Every connection has a trimmed non-empty `opc.tcp://` endpoint with a
non-empty authority and at least one point. Point variable names are unique
across all connections; variable and node identifiers are trimmed and
non-empty. Node-ID grammar remains a wire-boundary validation because the
parser must remain available when OPC UA wire support is not compiled.

The parser defaults omitted client security policy and mode to `none`/`none`.
Policy and mode spelling is trimmed and case-insensitive, with `-` and `_`
ignored while matching. It accepts only `none`/`none`, `basic256sha256` with
`sign` or `sign_and_encrypt`, and `aes128sha256rsaoaep` with `sign` or
`sign_and_encrypt`. Authentication defaults to anonymous. Anonymous
configuration rejects the presence of either username or password, including
blank values. Username authentication requires both trimmed non-empty fields.
Errors must not echo password contents.

Client polling is at least 10 ms. Timeout is in the closed interval from 1
through 60,000 ms. Every point declares one supported scalar type. The accepted
type spellings are `bool`/`boolean`, `int`/`int16`, `dint`/`int32`,
`lint`/`int64`, `uint`/`uint16`, `udint`/`uint32`, `ulint`/`uint64`,
`real`/`float`/`float32`, `lreal`/`double`/`float64`, and `string`,
matched after trimming and ASCII case folding. `access` and the legacy
`writable` alias are mutually exclusive rather than order-dependent; omission
means read access. Access accepts `read`, `write`, and
`read_write`/`readwrite`/`read-write`, with the same trimming and case-folding
rule. Read and read/write points are supported, while write-only points are
rejected because the runtime requires read evidence before reporting green
status.

Certificate listing, clearing, and explicit rejected-certificate promotion are
closed over the configured client PKI directory. They consider only regular
`.der`, `.pem`, or `.crt` files and return deterministic path order. A
symbolic-link listing/source root or entry observed during inspection is
ignored. A symbolic-link trusted promotion root or per-certificate destination
observed before copy is rejected, and that rejected source remains in place.
Clear removes only the listed trusted files. Explicit trust copies only listed
rejected files into the trusted root and removes only those exact rejected
files after each successful copy. Concurrent filesystem mutation between
inspection and file operation, batch atomicity across multiple promoted
certificates, and the destination policy for two different rejected paths with
the same filename remain unresolved and are not test authority.

For one-shot point reads and writes, the service response must contain exactly
one result for every request before any result is reported as successful. A
short or extra read vector is rejected before returning a value prefix. A
short or extra write-status vector is rejected before reporting batch success,
and every position in an exact-length vector must be `Good`. Result order is
interpreted as request order because the service response carries no separate
point identity at that boundary.

Client error classification is stable and ordered. Certificate failures map
to `cert_untrusted`; browse access, readability, and unknown-node failures map
to `browse_denied`; identity, credential, and security-policy rejection during
activation map to `auth_required`; absence of a matching supported endpoint
maps to `unsupported_security_profile`; and remaining transport failures map
to `endpoint_unreachable`. Classification changes recovery guidance only and
must not expose credentials.

5. **ADS floating-point ingress**
- ADS `REAL`/`LREAL` values entering through client reads/notifications or ADS
  server writes are accepted only when every scalar value is finite. The same
  rule applies to each `REAL`/`LREAL` array element.
- `NaN`, positive infinity, and negative infinity are rejected by the typed ADS
  decoder before a client sample is accepted, a server write is queued, or PLC
  variable storage is modified.
- A rejected client sample reports point-data error quality and leaves the
  previous PLC target unchanged; it does not require the ADS session itself to
  disconnect. A rejected server write returns invalid-data status and queues no
  write.
- ADS client egress applies the same finite-value requirement before a value
  enters the shared write queue or reaches the wire. A rejected scalar or array
  output remains unchanged in PLC storage, cancels any older local pending
  intent for that point, advances its local write generation, reports explicit
  error quality, and clears the queued-value baseline so a later finite value
  can be queued normally. An ADS operation already in flight cannot be
  cancelled, but its completion cannot overwrite the newer rejection state.
- The runtime does not clamp, normalize, or substitute a default value.
  Subnormal values and signed zero are outside this rule.

Protocol roadmap priority after OPC UA baseline:
- First: MQTT
- Next: EtherNet/IP

##### Connector status and discovery truth contract

Connector reporting is an additive supervisory contract over protocol-owned
transport loops. It does not replace `IoDriver`, ADS, OPC UA, Modbus, MQTT, or
EtherCAT execution. Every connector report uses the following closed
vocabularies:

- lifecycle state: `disabled`, `configured`, `starting`, `ready`, `degraded`,
  `reconnecting`, `stale`, `not_ready`, or `faulted`;
- health: `ok`, `degraded`, `faulted`, or `unknown`;
- discovery confidence: `confirmed`, `likely`, `port_reachable`, or
  `unavailable`;
- point quality: `good`, `stale`, `bad`, `unsupported`, `unavailable`,
  `write_pending`, or `write_failed`.

`ready` with `ok` health requires current protocol or driver evidence.
Configuration, a starting worker, a listener without a runtime snapshot, and
transport reachability do not satisfy that requirement. They remain
`configured`, `starting`, `not_ready`, or `unknown` as applicable. A connector
is `stale` when the whole connection is no longer fresh. A single stale point
uses point quality `stale`; the connector may remain usable but must report
degraded point counts instead of collapsing the point and connector states.
ADS reports without a usable runtime snapshot are `not_ready`/`unknown`.
Connected ADS or OPC UA reports are `ready`/`ok` only with no degraded points;
otherwise they are `degraded`/`degraded`. Reconnecting, stale, disabled, and
faulted source states retain their distinct normalized states.

Discovery confidence describes evidence, not desirability or deployment
readiness:

- `confirmed` requires a valid protocol-level exchange. Modbus uses FC43/14
  device identification or an explicitly configured FC03 safe read. A normal
  or protocol-valid Modbus exception response confirms the protocol. MQTT uses
  an accepted CONNACK.
- `likely` requires a protocol-shaped response that does not establish an
  accepted session. An MQTT authentication or authorization rejection is
  `likely` with `auth_required = true`; other valid rejected CONNACK responses
  are also `likely`.
- `port_reachable` proves only that the TCP connection succeeded. It must not
  be rendered or consumed as a confirmed Modbus device or MQTT broker.
- `unavailable` means that no useful discovery evidence was obtained.

Consumers of this contract must reject unknown or missing state, health, and
discovery-confidence values instead of rendering them as healthy. When one
fleet peer supplies an invalid connector report, the peer's raw topology
remains visible and the validation failure is shown to the operator; the bad
report must not make that peer or the other configured peers silently vanish.

MQTT discovery sets `clean_session = true` and sends DISCONNECT immediately
after every received CONNACK, including rejected CONNACK responses. It does not
leave a discovery session active. MQTTS port reachability without a TLS MQTT
exchange remains `port_reachable`.

The Modbus discovery wire probe emits a valid MBAP header with protocol ID
zero, exact transaction ID, unit ID, and PDU length. FC43/14 uses transaction
1; an optional FC03 safe read uses transaction 2 after a fresh bounded
connection. Request PDUs are limited to the Modbus TCP PDU maximum and an
empty or oversized PDU is rejected before writing. A response is protocol
evidence only when transaction ID, protocol ID, length, unit ID, function, and
complete body all match the request. Response length is 2 through 260 bytes;
a normal function response is accepted, and an exception function requires an
explicit exception code. A mismatched unit, truncated exception, malformed
length, unexpected function, or partial body is not Modbus evidence.

The MQTT discovery CONNECT packet uses MQTT 3.1.1, clean-session true,
keepalive zero, no credential or will flags, and a bounded UTF-8 client ID.
Remaining Length uses the canonical shortest MQTT base-128 encoding and is
limited to four bytes and 268,435,455; overlong, non-minimal, truncated, or
overflow encodings reject. CONNACK is exactly packet type `0x20`, remaining
length 2, a session-present flag of zero or one, and return code 0 through 5.
Reserved acknowledgement flags, extra bytes, unknown return codes, and
session-present set on a rejected connection are malformed protocol input, not
`likely` evidence. Valid return code zero is `confirmed`; codes 4 and 5 are
`likely` with authentication required; other valid rejection codes are
`likely` without that flag. A valid CONNACK is followed by DISCONNECT even
when rejected. Malformed input and TCP-only behavior never become protocol
confirmation.

##### Communication discovery scope and symbol browsing

Communication discovery is a bounded authoring operation. A directed Modbus or
MQTT TCP connection reports only `port_reachable` until the protocol probe
rules above earn stronger evidence. A CIDR scan broader than `/24` is rejected
before enumeration. EtherCAT discovery is available only from the runtime host;
another origin receives an empty candidate set and an explicit runtime-origin
warning. Runtime-origin mock discovery may project the configured/discovered
module chain, but it remains mock evidence. OPC UA generic discovery currently
returns an explicit server-setup warning and no candidates. A registered
protocol whose discovery is deferred returns an empty candidate set and a
stable unavailable warning; an unknown protocol is a request error.

`comm.discover` schema version 1 requires a non-empty string protocol. Its
optional scope is an object, origin is `this_host` or `runtime`, and passive
defaults true. Unknown request or scope keys are rejected so misspelled safety
limits cannot silently disappear. A requested non-passive scan still performs
connect/read-only operations and returns the stable warning that active write
probes are unsupported. Timeout defaults to 150 ms and is clamped to the
closed 1 through 2,000 ms range; malformed timeout types are request errors.

Protocol aliases are deterministic: Modbus hyphen/underscore forms select
`modbus_tcp`; OPC UA server and client aliases remain distinct; MQTT broker
selects `mqtt`; ADS client and TwinCAT select `ads`; and truST/runtime/mDNS
aliases select `discovery`. Deferred registered protocols return their
protocol-specific stable title in the unavailable warning. A blank or unknown
protocol never falls through to a generic registered protocol.

A Modbus directed host accepts a host, IPv4 address, or bracketed IPv6 address
with optional nonzero port and defaults to port 502. Its unit ID defaults to 1.
The optional safe FC03 read requires `probe_read_address`; quantity defaults to
1 and is limited to 1 through 125. Supplying quantity without an address, zero,
or a value above 125 rejects the request before a connection. A CIDR is trimmed
and parsed as IPv4/prefix; `/24` is the broadest accepted range. Enumeration is
ascending and canonicalized to the containing network: `/24` through `/30`
exclude network and broadcast, `/31` includes both point-to-point addresses,
and `/32` includes its sole address.

MQTT discovery never performs an undirected network scan. Missing host returns
no candidates plus the stable directed-host warning. A directed host accepts a
host, IPv4 address, or bracketed IPv6 address. An omitted port probes the
distinct resolved addresses at both 1883 and 8883 in deterministic
address/port order; an explicit nonzero port probes only that endpoint.
Schemes, userinfo, paths, queries, fragments, invalid ports, ambiguous
unbracketed IPv6, duplicate socket addresses, and unresolved targets reject
before probing. Port 8883 yields only MQTTS TCP-reachability evidence unless a
TLS MQTT exchange is actually completed.

OPC UA client discovery requires a directed host or endpoint. A bare host
defaults to `opc.tcp://<host>:4840`; an explicit valid port, bracketed IPv6
authority, or path is retained, and a canonical `opc.tcp://` endpoint is
trimmed and retained. Other schemes, userinfo, zero or invalid ports, query or
fragment components, and ambiguous unbracketed IPv6 reject before calling Get
Endpoints. Endpoint discovery failure is an explicit warning and empty result,
not a successful fabricated endpoint.

Every candidate ID is deterministic and contains only ASCII alphanumeric,
hyphen, underscore, period, and the protocol-owned separators added by the
projection. Candidate source and confidence come from the actual observation;
labels and parameters do not upgrade TCP reachability into protocol
confirmation. Candidate order follows deterministic target/source order, and
duplicate observations are collapsed by their protocol identity.

Symbol browsing returns a versioned tree whose `protocol`, `kind`, IDs, paths,
type labels, writable flag, and protocol-specific identifiers are derived from
the owning source:

- Local project browsing returns declared globals in deterministic name order
  and does not claim runtime values.
- EtherCAT channel browsing returns the configured module and channel order
  with exact process-image direction, byte/bit coordinates, and data type.
- Cached ADS browsing canonicalizes the supplied snapshot and returns the same
  deterministic candidate groups and import shape owned by the ADS
  symbol-import contract. The target AMS identity accepts the documented
  `ams_net_id` and `target_net_id` aliases.
- A live ADS upload failure classified as a missing return route returns a
  structured `route_missing` error plus the credential-free route plan. It
  must not fabricate a symbol tree or successful import.
- OPC UA leaf browsing retains the raw node ID and maps a supported scalar data
  type into the exact apply-facing type name. Browse failure retains the
  classified protocol error instead of returning an empty successful tree.

`comm.browse_symbols` schema version 1 requires a non-empty string protocol.
Kind defaults to `symbols`; protocol and kind are trimmed, ASCII-case folded,
and hyphen-normalized before dispatch. Unknown request or target keys reject.
The supported protocol/kind pairs are ADS `symbols`, OPC UA client `nodes`,
EtherCAT `channels`, and local OPC UA server, ADS server, or OpenOT `symbols`.
Every other pair is an explicit request error. Local browsing is selected only
for `symbols` on a local-capable protocol when the target is absent, explicitly
local, or has no remote host; a nonempty remote host never falls through to
project compilation.

Every successful response includes exact schema version, canonical protocol,
canonical kind, and a tree. Empty optional `error`, `route`, `ads_import`, and
warning members are omitted. A structured protocol error contains an empty
tree and no route/import payload. Secrets from targets, cached snapshots, route
plans, and protocol errors never appear in the tree, warnings, or diagnostic
text.

A cached ADS snapshot must use the current snapshot schema, a non-empty trimmed
route name, and valid non-empty dotted symbol paths. Empty segments, leading or
trailing periods, duplicate descriptor identities, and conflicting duplicate
symbol paths reject before tree or import projection. Snapshot order is
canonicalized before both projections. ADS groups and leaves are sorted
lexicographically at every level. Groups carry deterministic path-derived IDs,
`type = group`, children, and no size or writable claim. Leaves retain exact
remote path, ADS source type, byte size, and a writable flag derived only from
the remote Write capability. The tree and `ads_import.snapshot` contain the
same descriptor denominator.

An ADS live target accepts `ip`/`host` and `ams_net_id`/`target_net_id`
aliases. Host and AMS Net ID are trimmed; host is a bare IPv4, IPv6, or DNS
name without scheme, userinfo, path, query, or fragment. AMS Net ID is exactly
six decimal octets in the range 0 through 255. AMS port defaults to 851 and
must be nonzero. Conflicting duplicate aliases reject. Missing target and
snapshot, or an instance ID without the target needed for live browsing,
reject before route or wire activity. Cached browsing performs no route check
or network activity.

OPC UA browsing target, security, authentication, and credential rules are the
same as OPC UA client Test. The derived security profile's anonymous flag must
agree with the normalized auth mode. Each projected node preserves child
order, raw NodeId, browse path, OPC UA type NodeId label, apply-facing scalar
data type, and writable flag recursively; only its UI ID is sanitized.

EtherCAT browsing selects a configured EtherCAT driver by its exact
protocol-qualified instance identity, retaining the original `io.drivers`
index when other protocols precede it. Without an instance it selects the
first configured EtherCAT driver. A stale or cross-protocol identity rejects.
Module and channel order is configuration order; module IDs, paths, model,
slot, channel count, per-channel index, one-byte BOOL type, and ownership are
projected without claiming a live hardware observation.

Reading a project file, generated temporary artifact, cached snapshot, or live
wire response remains an external oracle. A structural response-shape
assertion may map to this contract, but an unresolved file or wire observation
cannot be promoted to proof merely because surrounding assertions are static.

##### EtherCAT unavailable-resource contract

Creating an EtherCAT driver with a non-mock adapter does not require the
adapter to be present, so project construction and runtime startup remain
non-blocking. The first operation that needs the wire attempts bounded hardware
initialization. If the adapter, configuration, or post-allocation hardware
resource is unavailable, the operation returns an error and the connector is
faulted rather than healthy.

EtherCrab process-data storage cannot be safely allocated repeatedly after a
post-allocation initialization failure. Such a failure is therefore terminal
for that driver instance: later reads and writes return the visible
`ethercat hardware unavailable until driver rebuild` error without another
allocation attempt or retry window. Recovery requires constructing a new
driver instance. This terminal rule prevents an unavailable adapter from
causing unbounded allocation, retry, or scan-cycle delay. `adapter = "mock"`
proves deterministic software behavior only and is not evidence that physical
EtherCAT hardware is available or production-ready.

#### 6.6 Fault, Overrun, and Watchdog Handling

The runtime traps execution faults and reports them through a unified fault channel. By default, a fault transitions the resource into a **FAULT** state and halts further task execution until restarted.

Faults include:
- Arithmetic errors (e.g., divide by zero)
- Out-of-bounds accesses
- Invalid type conversions
- FOR loops with a step expression that evaluates to 0 (guarded by bytecode and treated as a runtime fault)
- Task overruns (missed deadlines)

Overrun policy (default): if a periodic task misses its deadline, the missed activation is dropped, the overrun counter increments, and the task is eligible again on the next interval boundary.

**Watchdog policy (production):**
- A watchdog monitors cycle/task execution time.
- If the watchdog timeout elapses, the runtime raises a **FAULT** and halts the resource.
- The output-commit deadline is checked before any physical driver write. If it
  has elapsed, the pending process-image output is not sent to a driver.
- Timeout thresholds, fault actions, debugger interaction, automatic recovery,
  and the physical driver commit boundary are configured by the truST host
  runtime (see §6.9). IEC 61131-3 does not define these host mechanisms, so
  they are product contracts here rather than IEC deviations.
- Default action is **safe_halt**: outputs are set to configured safe values (if provided),
  then the resource halts. For **halt** and **safe_halt**, safe-state outputs are applied
  before halting. If no safe-state output values are configured, fault handling performs
  no physical output write; it must not re-send the pending process-image output as a
  substitute safe state.
- If a pre-commit fault occurs after a pending image has been staged, the
  runtime restores the last physically committed output image before applying
  configured safe values. A partial safe-state map overrides only its named
  addresses; every unconfigured output therefore retains its last committed
  value and cannot leak the failed scan's pending value.

**Debug and resource pause interaction:**
- The watchdog measures active cycle execution, not operator dwell time at a
  debugger statement-boundary pause. Time spent waiting in that paused state is
  excluded from the current cycle's execution and output-commit deadlines.
- A resource pause accepted between cycles does not arm a new watchdog deadline
  and does not start another input, task, retain, or output phase while paused.
- A debugger pause inside a cycle suspends that cycle. It does not start another
  scan or commit outputs while waiting. Resume continues the suspended cycle with
  the same remaining active-execution budget; it does not grant a fresh full
  watchdog interval.
- Pause alone does not apply safe state or clear a fault. A real active-execution
  timeout before or after the paused interval still follows the configured
  watchdog fault action.

These pause rules are a truST host/debugger contract outside the IEC language
execution model, not an IEC deviation.

**Automatic restart policy:**
- A non-panic cycle fault configured with `FaultPolicy::Restart`, or a watchdog
  timeout configured with restart action, enters the same warm-restart storage
  transition defined in section 6.7: `RETAIN` values are preserved while
  `NON_RETAIN` and ordinary initialized storage are reinitialized.
- A contained `ResourcePanic` is never automatically restarted. It remains a
  visible fault so an unexpected unwind cannot be hidden by retry.
- Automatic restart attempts are bounded to three consecutive attempts for the
  same unresolved fault. Exhaustion leaves the resource faulted with a
  diagnosable restart-limit error.
- Consecutive automatic retries use a nonzero monotonic backoff derived from
  the configured cycle interval. The base is clamped to the inclusive range
  from one millisecond through one second, later attempts increase
  exponentially, and the resulting delay never exceeds one second.

**Per-cycle deadline lifecycle:**
- Before executing a cycle with an enabled watchdog, the scheduler arms both
  the execution and output-commit deadlines from the same wall-clock start and
  configured timeout. A disabled watchdog leaves both deadlines unchanged.
- The scheduler restores the deadlines that existed before the cycle after
  normal completion, an ordinary runtime error, or a contained cycle panic.
  Temporary per-cycle deadlines therefore cannot leak into later work.
- Enabled non-positive policy values are normalized before installation as
  specified below. As a defensive helper boundary, a zero timeout that reaches
  deadline construction produces an immediately expired deadline rather than
  disabling enforcement or extending the cycle.

##### Fault-state clear lifecycle

The portable fault holder clears its current fault as one state transition:
`faulted` becomes false and the stored last fault becomes absent. Clearing does
not change the configured `FaultPolicy`. Calling clear while already healthy is
idempotent and leaves the healthy state, absent last fault, and policy
unchanged.

This clear operation is bookkeeping at the portable holder boundary. It does
not restart or resume a resource, apply safe outputs, reset program or retained
storage, prove that the underlying fault was repaired, or authorize a caller to
recover production execution. Those actions remain owned by the host recovery
and control boundaries. This is a truST runtime API contract, not an IEC
deviation.

##### Fault-policy decision projection

The portable fault holder projects its configured `FaultPolicy` to a
`FaultDecision` using this closed table: `Halt` returns action `Halt` with
`apply_safe_state = false`; `SafeHalt` returns action `SafeHalt` with
`apply_safe_state = true`; and `Restart` returns action `Restart` with
`apply_safe_state = false`. The projection reads only the current policy. Its
result is unchanged by whether the holder is healthy or faulted and by the
presence or value of the stored last fault. It does not mutate any of those
observations.

A `FaultDecision` is an instruction to the owning host-control path, not proof
that its action occurred. Producing it does not itself apply safe outputs, halt
or restart a resource, clear a fault, reset storage, consume a retry, or
authorize recovery. Those effects remain governed by the safe-state, automatic
restart, and recovery-control contracts above. This is a truST runtime API
contract, not an IEC deviation.

##### Fault recording and policy replacement

Recording a runtime fault sets the portable holder's `faulted` observation to
true and stores the supplied `RuntimeError` as the exact last fault. A later
record replaces the earlier last fault. Recording does not change the
configured `FaultPolicy`, execute that policy, clear state, apply safe outputs,
or restart a resource.

Replacing `FaultPolicy` stores the supplied policy exactly. It does not change
the current faulted flag or stored last error, including when policy is changed
while the holder is already faulted. Policy replacement returns no evidence
that the corresponding halt, safe-state, or restart action was executed. These
are truST runtime API contracts, not IEC deviations.

##### Watchdog policy normalization and installation

Before an enabled `WatchdogPolicy` is installed, a timeout less than or equal
to zero is normalized to one millisecond so the scheduler cannot receive an
always-expired enabled deadline. A positive enabled timeout is preserved.
Disabled policy timeouts are inert at this boundary and remain unchanged. The
configured `WatchdogAction` is preserved in every case.

`WatchdogSubsystem::set_policy` replaces the previous policy with that exact
normalized value. Installation does not arm a deadline, report a timeout,
apply safe state, halt, or restart a resource; those effects remain owned by
the scheduler and host-control paths. The subsystem decision projection reads
the installed action without mutating the stored policy. These are truST
runtime configuration contracts, not IEC deviations.

##### Portable watchdog and fault configuration model

Configuration token parsing trims surrounding whitespace and compares ASCII
case-insensitively. `WatchdogAction` and `FaultPolicy` accept exactly `halt`,
`safe_halt`, and `restart`; `RetainMode` accepts exactly `none` and `file`.
Every other token returns `RuntimeError::InvalidConfig`. The diagnostic
preserves the complete original, untrimmed text in one of these exact forms:
`invalid watchdog action '<text>'`, `invalid fault policy '<text>'`, or
`invalid retain mode '<text>'`.

The default `WatchdogPolicy` is disabled, has a zero duration, and selects
`SafeHalt`. Both `WatchdogSubsystem::new` and `Default` store that policy.
Their decision projects `Halt` to `(Halt, apply_safe_state = true)`,
`SafeHalt` to `(SafeHalt, true)`, and `Restart` to `(Restart, false)`.
The watchdog `Halt` projection therefore deliberately differs from the
ordinary `FaultPolicy::Halt` projection: a watchdog timeout requires safe
output handling before halt.

Both `FaultSubsystem::new` and `Default` select `FaultPolicy::Halt`, report a
healthy state, and contain no last fault. Reading policy, decision, faulted
state, or last fault does not mutate the holder. `FaultInfo` stores the exact
human-readable reason supplied by the host, and cloning it preserves that
text. Constructing or reading these portable model records does not execute a
halt, safe-state write, restart, retain operation, or recovery action.

#### 6.7 Retain Storage (IEC 61131-3 §6.5.6)

Retentive variables must follow IEC 61131-3 retentive variable rules (§6.5.6, Figure 9). At
startup:

- **Warm restart**: RETAIN variables restore their retained values; NON_RETAIN are initialized.
- **Cold restart**: RETAIN and NON_RETAIN variables are initialized.
- Unqualified variables follow the runtime's retain policy (see `docs/IEC_DECISIONS.md`).
- `VAR_STAT` follows the documented vendor-extension storage rules:
  function statics persist across calls, method statics persist per instance and per method, and
  `PROGRAM`/`FUNCTION_BLOCK`/`CLASS` `VAR_STAT` uses ordinary instance storage.

Retain storage is provided via a pluggable backend:

```rust
pub trait RetainStore: Send {
    fn load(&self) -> Result<RetainImage, RuntimeError>;
    fn store(&self, image: &RetainImage) -> Result<(), RuntimeError>;
}
```

##### Retain snapshot identity and order

The portable retained snapshot receives already-resolved logical names. It
does not canonicalize, case-fold, or otherwise reinterpret a supplied key.
The first insertion of a name appends one entry after all existing entries.
Inserting the same resolved name again replaces only that entry's value at its
original position: snapshot cardinality, sibling values, and sibling order do
not change. A different name remains a distinct entry and appends in call
order.

Configuration/global retained variables keep their resolved logical name.
Retentive variables declared inside a `PROGRAM` use the reserved internal
identity `@program/<program-identity>/<variable-identity>`. The `@program/`
prefix cannot be an IEC identifier, so it cannot collide with a declared
global. Program and variable identities retain their reviewed spelling in the
file and resolve ASCII-case-insensitively on load. Snapshot creation visits
programs in runtime registration order and variables in declaration order.
Program-variable entries participate in the same full-snapshot validation,
migration, orphan reporting, and atomic apply boundary as retained globals.

Insertion stores the complete runtime `Value` without changing its concrete
variant or nested contents. This is a deterministic snapshot-construction
contract: constructing a snapshot from an ordered map, borrowing its values,
and consuming it back into a map preserve the exact key spelling, entry order,
runtime value variants, and nested values without normalization or
reconstruction.
contract, not a validation bypass: declared-type migration, non-finite value
rejection, serialization, and atomic application remain owned by the retain
save/load/apply boundaries below. Successful insertion alone does not mean a
snapshot is valid for persistence or warm reload. IEC 61131-3 Ed.3 section
6.5.6 defines retentive variable behavior but not this internal container
policy; the truST choice is specified here and is not an IEC deviation.

The runtime loads retained values during resource startup and writes them on shutdown and
periodically (policy defined in the runtime configuration). The periodic cadence is
rate-limited and only writes when retained values have changed.

The file-backed retain boundary accepts `REAL` and `LREAL` values only when
they are finite. This rule is recursive through retained arrays and structures
and applies to both serialization and deserialization, including legacy retain
images. A rejected save does not replace the last durable snapshot. A loaded
snapshot containing `NaN`, positive infinity, or negative infinity is rejected
as a whole before any retained global is applied; the runtime does not clamp,
normalize, or substitute a default value. In-process floating-point execution
outside the file-backed retain boundary is unaffected by this rule.

Retain-store failures use the following fail-closed transaction policy. IEC
61131-3 section 6.5.6 defines which variable classes are retentive across a
restart; the storage and failure boundaries below are the truST runtime
contract:

- A missing configured file represents an empty snapshot. Open, read, framing,
  checksum, decode, declared-type migration, and value-compatibility failures
  are errors. A warm startup or warm reload that encounters one of these errors
  fails instead of continuing with a partially restored image.
- The complete loaded snapshot is decoded, canonicalized, and validated before
  any retained global is changed. If any retained entry is invalid, every
  retained global keeps the value it held before the load attempt.
- A save is encoded before it is published and is written through a temporary
  file in the destination directory. Failures before the atomic rename leave
  the previous file unchanged. A rename or parent-directory synchronization
  failure is reported as an error; the runtime does not claim that the new
  snapshot is durable. The in-memory snapshot remains dirty so a later save may
  retry.
- A due periodic save failure fails that scan before physical outputs are
  committed. A requested-stop save failure is exposed as the resource's retain
  error rather than reported as a successful stop.
- Cold restart does not load the retain store and therefore cannot silently
  reinterpret a failed warm-load attempt as cold-start success.

##### Retain codec compatibility and migration

The file-backed retain store preserves the logical snapshot across a successful
store/load round trip. The supported codec preserves exact names, order, and
the concrete values asserted by the compatibility profile: Boolean and integer
scalars, bounded arrays with their inclusive bounds, structures with their type
and field identity, and enumeration values with their declared type and
variant. This includes qualified retained `PROGRAM` variables. A warm restart
loads and applies the saved snapshot after rebuilding program instances; a
cold restart initializes retained variables and does not load the store. A
missing file alone represents an empty snapshot; a path that exists
but is not a readable retain file, or a parent path that cannot contain the
file, returns the underlying retain operation as an error instead of silently
substituting an empty snapshot.

The current checksum-protected format is version 2. Version-1 `STRN` snapshots
remain readable for the value tags supported by that format; a decoded value
materializes with the same logical name and runtime value. Successful
replacement publishes one complete snapshot and leaves no retain temporary
file. Parent-directory synchronization occurs after rename. If that final sync
fails, `store` returns an error and does not claim durability, but a read in the
same filesystem session may observe the already-renamed new snapshot. The
manager keeps the snapshot dirty and permits the next due save to retry without
another mutation notification.

Before applying a loaded snapshot, the runtime resolves every retained name
and stages all compatible migrations:

- a safe declared numeric widening such as `INT` to `DINT` preserves the
  mathematical value;
- a retained `STRING` longer than its current declared bound is canonicalized
  to that bound using the ordinary scalar-preserving string rule;
- compatible enumeration values are rebuilt with the current declared enum
  type and variant identity, including the reviewed case-canonical spelling;
- compatible structures recursively rebuild the current declared structure
  and field identities, and compatible one-dimensional arrays preserve their
  inclusive bounds while recursively rebuilding each element;
- a structure field added by the current declaration receives its declared
  initializer or type default, while a field removed from the declaration is
  dropped; and
- a retained global absent from the current program is dropped as an orphan.

An incompatible nested structure field or array element rejects application;
the runtime does not substitute a default or expose a partially rebuilt
aggregate. These focused rules do not establish arbitrary-depth,
multidimensional, renamed, removed-variant, or event-emission behavior.

Each successful value-shape migration emits `RetainMigrationApplied`; each
dropped orphan emits `RetainOrphanDropped`. These events identify the runtime
transition, not a stable debug-format string. Migration remains transactional:
all entries are validated and staged before any retained target changes, so one
incompatible entry rejects the whole snapshot without exposing earlier staged
values.

**Power-loss guidance:** retained values are only guaranteed to persist if the most recent
snapshot has been flushed to the retain store (i.e., at shutdown or after the save cadence).
Here, shutdown means the explicit requested-stop path. Plain destruction of a
`Runtime` or `TestHarness` is not a graceful-stop request and does not
implicitly publish dirty retained state. Unflushed changes may be lost on
sudden power loss (implementer-specific).

##### Direct restart preparation transaction

An in-process warm restart constructs its complete replacement storage and
instance image before changing live state. If reviewed function-block instance
construction fails, the restart returns an error and preserves the live
executable, storage values, queued debug force, and cycle counter. This direct
restart boundary does not independently specify task schedules, logical time,
I/O bindings, process-image state, or a stable diagnostic string.

##### 6.7.1 Online-change transaction

Online change is a truST runtime extension; IEC 61131-3 does not define its
transport or transaction boundary. A reload request is applied only
at a completed scan boundary. The runtime must prepare every fallible input
needed by the change before replacing live execution state:

- decode and validate the complete bytecode container and materialize its VM
  module;
- validate the selected resource, tasks, and process-image sizes; and
- read, decode, canonicalize, and validate the retained snapshot required by
  the warm-reload policy; and
- construct the complete warm-restart variable and instance image in staged
  storage, including retained program variables and all fallible function-block,
  class, and program instance initialization.

When a caller names a bytecode resource, selection is case-insensitive but
must resolve that exact resource; an unknown name is an error and must not
silently select the primary resource. For compatibility with bytecode emitted
before resource identities were preserved, a module containing exactly one
resource named by the synthetic legacy placeholder `RESOURCE` may satisfy a
named selection. After that compatibility load, the requested name becomes the
runtime's active resource identity so subsequent bytecode encoding cannot
propagate the placeholder. The exception does not apply to multi-resource
modules.

If preparation fails, the request returns an error and leaves the prior
executable module, task schedule, variable storage, process image and bindings,
debug mutations, logical time, cycle counter, and runtime fault/status state
unchanged. The old program remains executable on the next scan.
For the reviewed retained `INT(0..10)` value `100`, this rejection retains the
cause-identifying diagnostic text `outside declared subrange 0..10`.
The reviewed malformed byte sequence `[00, 01, 02, 03]` fails preparation as
the typed `RuntimeError::Bytecode`; no stable message text is implied.

After successful preparation, the commit replaces the executable and resource
metadata as one cycle-boundary operation, restarts at the program entry point,
applies the prepared retained snapshot, rebinds I/O, and returns the new runtime
metadata. Retained variables follow section 6.7; non-retained variables and
function-block instances follow warm-restart initialization. Logical time is
preserved, the scan counter restarts, and queued writes and forces are cleared
under section 6.9.2. A request must never report `reloaded` unless this complete
commit succeeds.

##### 6.7.2 Runtime agent command conformance

The runtime implementation of the Agent API preserves the canonical workspace
identity and validates the requested method and workspace-relative path before
performing an operation. Its project, workspace, diagnostic, formatting,
build, validate, test, runtime, and harness methods return the typed payload
defined by the Agent API contract. Unknown methods and invalid paths return
the stable reviewed error class instead of a false successful payload.
Bounded test and harness loops must terminate at their requested case,
iteration, or deadline boundary.

`runtime.compile_reload` completes diagnosis and compilation before requesting
the live reload. `runtime.reload` rebuilds the selected project before
replacing the live executable. A compilation, preparation, or reload failure
is returned as failure and leaves the current live runtime available; neither
method may report a successful reload until the complete replacement
transaction has committed. These runtime obligations specialize the general
Agent API method contract and do not widen its public method set.

#### 6.8 Runtime Launcher & Deployment (Project Folder)

Production runtimes are started via the CLI (`trust-runtime run`) using a **project folder**
(runtime bundle format) directory. The launcher is responsible for:

- Loading the bytecode program (`program.stbc`).
- Loading runtime configuration (`runtime.toml`).
- Initializing I/O drivers (`io.toml` or system IO config).
- Initializing retain storage (if configured).
- Exposing a control endpoint for local attach/debug.
- Validating bundle version compatibility before execution (internal `bundle.version`).

The launcher **must** run on Linux, Windows, and macOS (desktop targets). Embedded targets may
replace the launcher with platform-specific init systems while preserving the same configuration
and control protocol.

If a project folder omits `io.toml`, the launcher loads the system IO config

This host-side launcher policy is a truST product contract. IEC 61131-3 does
not define hardware-driver selection or OS-level I/O configuration, so the
policy is not an IEC deviation.

##### Windows runtime executable stack link contract

The `trust-runtime` Cargo build script always declares itself as a rebuild
input. For a Windows target it assigns the `trust-runtime` executable an
8 MiB stack: MSVC targets emit `/STACK:8388608`, while other Windows toolchains
emit `-Wl,--stack,8388608`. Non-Windows targets emit no executable stack linker
argument. This is a build-platform contract for the runtime executable; it
does not change PLC task stack limits or establish runtime stack-usage bounds.

##### Runtime bundle builder source and dependency resolution

Without an explicit source override, the runtime bundle builder selects the
project's `src/` directory. A project containing only the legacy `sources/`
directory rejects with the reviewed missing-`src/` diagnostic rather than
silently changing source roots.

The reviewed local dependency graph resolves direct and transitive
`trust-lsp.toml` path dependencies, includes the root and dependency source
files in compilation and inspection, reports the reviewed `LibA`, `LibB`
dependency identities, and produces `program.stbc` after successful build.
For one unchanged reviewed local dependency graph, two consecutive builds
resolve the same dependency and source order and emit byte-identical
`program.stbc` content. This bounded repeatability contract does not establish
determinism for arbitrary registries, symlink graphs, concurrent filesystem
mutation, or cross-platform path representations.

`check_program_stbc` performs the same source discovery, dependency resolution,
compilation, and bytecode sizing as build, reports the reviewed root source and
dependency identity, and does not create or replace `program.stbc`.

Dependency resolution rejects a missing declared path, a cycle in the local
dependency graph, and a requested package version that disagrees with the
dependency manifest. The reviewed diagnostics respectively retain the missing
dependency name, identify a cyclic dependency, and retain the requested
version. Rejection returns no successful build report.

For the reviewed root project, a structure type declared in one source, a
global of that type declared in a second source, and a program accessing the
global structure field in a third source compile together and produce the
bundle artifact while retaining all three source identities.

These focused builder rules do not prove arbitrary dependency ordering,
symlink or concurrent-filesystem behavior, cleanup after partial writes,
external registries, bytecode semantic equivalence, or cross-platform path
behavior.

##### Namespace-qualified multifile runtime assembly

IEC 61131-3 Ed.3 Section 6.9 and Tables 64-66 define namespace membership,
qualified access, and the `USING` directive. The runtime source assembler
preserves those identities across the merged source set:

- a direct function member imported by `USING` is callable when the namespace
  and consuming program come from separate source units;
- namespace-qualified programs are registered by their full name and remain
  executable scan entry points;
- sibling functions, function blocks, classes, and interfaces declared inside
  one namespace are registered by full name, resolve within that namespace,
  and execute as one semantic graph; and
- type and member defaults retain their declaring namespace/`USING` context,
  so a qualified or imported namespaced constant resolves identically across
  source order and from every consuming source; a missing or ambiguous import
  fails before runtime publication; and
- program names are unique under ASCII-case-insensitive comparison across the
  merged source set. A collision rejects assembly, reports the duplicate
  condition, and retains the reviewed original spelling in the diagnostic.

These rules do not imply source-order dependence, arbitrary nested-namespace
import, complete diagnostic prose stability, or registration of declarations
that fail semantic validation. Namespace and `USING` semantics are IEC
behavior; runtime registry identities, scan-entry selection, and the bounded
collision diagnostic are truST integration behavior, not IEC deviations.

##### Runtime CLI entrypoint, diagnostics, and dispatch contract

The `trust-runtime` entrypoint parses one optional top-level command. A
successful parse dispatches that command exactly once to its corresponding
handler; omitting the command invokes the default runtime workflow. Handler
errors return to the entrypoint, which prints one error and exits non-zero
instead of continuing into another command path.

An invalid top-level command may produce one `Did you mean` hint before the
ordinary parser error. The hint selects the first registered command with the
smallest case-sensitive edit distance, only when that distance is at most two.
Global flag tokens are not treated as the mistyped command, surrounding
whitespace is ignored for comparison, and inputs outside the threshold receive
no potentially misleading suggestion.

For commands that accept a project folder, deprecated `--bundle` spellings
remain parser aliases for `--project` during the compatibility window. After a
successful parse, use of either `--bundle <path>` or `--bundle=<path>` emits one
deprecation warning before dispatch. It does not change project creation or
validation semantics.

`run` and `play` accept `vm` as the sole production
`--execution-backend` value. The retired `interpreter` value is rejected during
argument parsing, before handler dispatch, project access, or runtime mutation.

Ordinary handler failures exit with status 1. When a parsed command enables
`--ci`, the entrypoint uses the documented CI error classification, including
command-aware fallback codes. Known configuration and permission failures may
append one actionable tip to the original error text; the tip does not replace
or reclassify the underlying error.

For `ide serve`, the explicit project path, or the current directory when it is
omitted, must be an existing directory. A root `runtime.toml` is the primary
runtime; otherwise the lexicographically first immediate child containing
`runtime.toml` is selected. A selected runtime configuration must load
successfully before the web server binds. A workspace without any runtime
configuration uses the standalone `config-ui` identity so source-only
authoring remains available.

The standalone IDE source registry uses the selected runtime project's normal
recursive, deterministic `.st` and `.pou` build-source discovery, while
presenting paths relative to the selected workspace. Its synthetic control
state is local and nonexecuting: control authentication is disabled, debug
execution is not enabled, and discovery and mesh are disabled. The server
starts in standalone-IDE mode only after this state is constructed. These are
truST host-product rules outside IEC 61131-3 and are neither an IEC decision
nor an IEC deviation.

The deprecated `config-ui serve` alias emits its deprecation warning before
entering this same standalone-IDE validation and startup path; it does not own
a second project-selection or server mode.

##### Runtime conformance case runner

`trust-runtime conformance` consumes an explicit suite directory or the
`conformance` directory below the current working directory. The suite root
must exist as a directory. Case discovery is deterministic and fail-closed:
only the registered category directories are accepted; every case directory
requires `manifest.toml`; the manifest ID and category must match their
directory names; and IDs use
`cfm_<category>_<nonempty-lowercase-token-sequence>_<three-digits>`.
Manifest source paths are relative, contain no parent/root/prefix component,
and must resolve inside their owning case directory.

The runner supports three closed case kinds:

- A `runtime` case requires `cycles > 0`. Every nonempty advance, named-input,
  and direct-input series has exactly one element per cycle. Restart directives
  target `1..=cycles`; `warm`, `hot`, and `fault` use warm retained-state
  behavior, while `cold` and `download` use cold behavior. `skip` and `_`
  preserve the previous step value. Typed inputs use `KIND:VALUE` for BOOL,
  signed and unsigned integers, bit strings, REAL/LREAL, TIME/LTIME, and
  STRING, with the declared runtime width enforced.
- A `compile_error` case succeeds only when compilation fails. Successful
  compilation is a case-execution error and must never be blessed as expected
  failure output.
- A `connector_status_trace` case requires at least one step. Its closed source
  vocabulary is ADS connection/status, OPC UA client/server, MQTT session,
  Modbus, EtherCAT, and generic I/O-driver health. Source and state tokens are
  trimmed, ASCII case-insensitive, and treat hyphens as underscores. Unknown
  tokens and expected state/health mismatches fail the case.

Cases execute in ascending case-ID order. Runtime artifacts contain the
declared observations for every cycle; connector artifacts contain every
projected step. `--update-expected` writes canonical pretty JSON to the
category's expected-artifact path. Verify mode distinguishes a missing or
unreadable expectation from a readable value mismatch, writes actual mismatch
artifacts, and emits stable reason codes. Suites containing only the six legacy
categories use summary profile/version `trust-conformance-v1`/1; any expanded
category selects `trust-conformance-v2`/2. The command writes and prints the
same summary and returns an error whenever any case failed or errored. These
runner and artifact rules are truST product behavior outside IEC 61131-3 and
are neither an IEC decision nor an IEC deviation.

##### Runtime launcher input, validation, and startup contract

`trust-runtime play --project <path>` is the product convenience entry point.
It creates or completes the selected project when the folder is missing or
lacks `runtime.toml` or `program.stbc`; project I/O may instead come from the
system configuration. An existing non-directory project path is rejected
without mutation.

`trust-runtime play` without `--project` is the first-run creation exception to
the general optional-project-resolution contract. It first performs standard
bundle detection. A successful detection selects that project and applies the
same completeness rule above. Only the ordinary no-project-found outcome
creates the default project in the current directory. Current-directory access
failures and system-I/O configuration read, parse, or validation failures are
preserved and abort before project mutation.

All non-filesystem launch arguments, including restart mode and simulation time
scale, must be validated before project detection, creation, or any other
project mutation. A rejected option therefore leaves a previously missing
project path missing. `trust-runtime run` consumes an existing project and does
not provide this creation behavior.

Project-mode startup recursively compiles the `.st` and `.pou` sources beneath
the project's selected source root, case-insensitively by extension, together
with sources from resolved local project dependencies. Legacy `--config`
startup recursively compiles those source types beneath its literal runtime
root. Source discovery must not interpret project or directory names as glob
syntax, must use deterministic path ordering, and must propagate directory and
file-read failures. An empty or invalid source set is a startup error. Startup
without a project or legacy configuration is the IDE-shell mode: it compiles
one synthetic empty `Main` program rooted at `__ide_bootstrap__.st`, exposes no
bundle, and marks the loaded runtime as an IDE shell instead of claiming that a
project was loaded.

Before the first PLC scan, the launcher must successfully complete all startup
preparation: bundle-version validation; source and bytecode validation; runtime,
watchdog, fault, safe-state, telemetry, I/O-driver, and retain configuration;
configured ADS and OPC UA client startup; execution-backend selection; control
endpoint and TLS validation; and enabled control, protocol, discovery, mesh,
historian, persistence, and web-service initialization. Failure in any of these
steps aborts startup; the scan start gate is not opened and the launcher must
not report the PLC as running. Warm startup loads and validates the configured
retain snapshot before execution. Cold startup does not read retained values.
Bundle-version compatibility is checked before watchdog, fault, or other
runtime policy is mutated; an unsupported version leaves the prior runtime
policy unchanged. A disabled ADS or OPC UA client is a startup no-op and must
not construct a transport. Enabling either client without its loaded sidecar
configuration is a startup error detected before transport construction or
connection-state mutation.

Simulation startup rejects a CLI or project time scale of zero. `--simulation`
enables simulation without discarding the project's simulation model; a CLI
time scale greater than one enables simulation and overrides the project time
scale. Otherwise the loaded project simulation configuration remains
authoritative. The settings exposed to control and status surfaces must reflect
the effective runtime configuration and this effective simulation/backend
selection, rather than an independent set of launcher defaults.
A simulated launch exposes a warning that names simulation mode, the effective
time scale, and that the mode is not for live hardware. Production mode emits
no simulation warning.

`trust-runtime validate --ci` validates the same bundle version, TLS, control
authentication, enabled I/O-driver configurations, bytecode container, module,
and selected-resource requirements used by startup. Its versioned JSON success
payload reports only enabled I/O drivers in `io_driver` and `io_drivers`, in
configuration order; disabled configuration entries are not reported as active
runtime drivers.

Runtime structured logging accepts `error`, `warn` (`warning`), `info`,
`debug`, and `trace`, case-insensitively. Empty or unknown configured levels are
configuration errors, not aliases for `info`. Every emitted line is one JSON
object containing a Unix-epoch millisecond timestamp, canonical level, event
name, and event data; events below the configured verbosity are omitted.
SIGINT and SIGTERM request the ordinary bounded runtime stop path so shutdown
and safe-state behavior completes before the launcher emits `runtime_exit` and
returns success.

Operator-facing I/O provenance labels preserve the selected driver direction.
For Modbus, an input address uses the configured input-register base and an
output address uses the configured output-register base, with the addressed
word offset added to that base. For MQTT, an input address names `topic_in` and
an output address names `topic_out`; the opposite-direction topic is never
substituted.

##### 6.8.0 Deprecated workbench command forwarding

During the documented compatibility window, deprecated `trust-runtime`
workbench aliases forward their complete argument vector to `trust-dev` after
printing the removal-not-before warning. Binary resolution uses an explicit
`TRUST_DEV_BIN` value first, then an executable regular file named `trust-dev`
beside the running `trust-runtime` binary, then normal `PATH` lookup. A directory
or non-executable sibling must not shadow a usable PATH command.

The forwarding process preserves a normal child exit code. On Unix, when the
child terminates from signal `n`, the alias exits with the conventional shell
status `128 + n` instead of flattening the failure to generic status 1. Spawn
failures remain normal CLI errors with installation guidance.

##### 6.8.1 Scriptable control client

`trust-runtime ctl` exchanges one newline-delimited JSON request and response
with the selected runtime control endpoint. `--endpoint` overrides the endpoint
from the project runtime configuration. Credential resolution is independent
of endpoint resolution and uses this precedence: explicit `--token`, then
`TRUST_CTL_TOKEN`, then `runtime.control.auth_token` from `--project`. The
project token remains eligible when `--endpoint` overrides only the endpoint;
an explicit or environment token does not require loading the project merely
to resolve credentials.

Each CLI action maps to one control request with numeric `id = 1`, its
registered control operation name in `type`, the resolved token in `auth`, and
only the parameters declared by that action. The client accepts exactly one
JSON response line. An empty or malformed response is a transport/protocol
failure. The response must end with a newline and contain the same non-negative
integer `id` as its request; a missing, noninteger, or mismatched response ID is
a protocol failure. A response with `ok = false`, or without a Boolean `ok`, is
a command failure and must produce a non-zero process exit rather than being
printed as a successful scripted result. A TCP connection attempt is bounded to
500 milliseconds;
after a TCP or Unix connection is established, reads and writes are bounded to
750 milliseconds, and a response line may not exceed 1 MiB.

`config.set` values trim surrounding whitespace and decode case-insensitive
`null`, `true`, and `false`, followed by an in-range signed 64-bit decimal
integer; every other value remains the trimmed string. Human-oriented rendering
happens only after `ok = true`. Status renders
`state=<state> fault=<fault> rt_profile=<profile> rt_active=<bool>`, defaulting
absent fault/realtime fields to `none`, `disabled`, and `false`. Health renders
`ok=<bool>`. An empty task list renders `tasks=0`; otherwise each task renders
one line containing its name, min/average/max/last milliseconds to three
decimal places, and overrun count.
For status, health, and task summaries, a missing or wrong-typed required result
field is a command failure; it is not rendered as raw JSON or replaced with an
invented value.

###### Runtime control request, job, and state-surface contract

Each newline-delimited control request is decoded exactly once. Malformed JSON
returns a negative response without dispatch or mutation. A request type absent
from the registered operation set returns the stable error
`unsupported request`; every registered type routes to its owning handler.
Missing or malformed required parameters return a parameter error rather than
`unsupported request`. A routing behavior lock proves only that the registered
handler boundary was reached; it is not evidence that every operation
succeeded or returned its complete semantic payload.

Asynchronous operations return a nonempty job ID. Job status is `running`,
`complete`, or `failed`; an unknown ID returns a stable machine-readable
not-found error. Terminal success retains its report and terminal failure
retains its reason. Work that cannot run because a required build feature is
absent terminates as failed rather than remaining indefinitely `running`. ADS
client and server Doctor use this same job contract. Synchronous and live ADS
operations that require `ads-wire` fail explicitly when it is unavailable and
must not synthesize discovery, Doctor, route, or import results.

The status and configuration surfaces project one effective runtime state:

- `status.execution_backend`, `status.execution_backend_source`,
  `status.metrics.execution_backend`, and the corresponding `config.get`
  values agree. Omission selects backend `vm` with source `default`.
- Status preserves resource identity, control and simulation mode, cycle and
  profiling metrics, realtime requested and observed state, warnings, errors,
  and every I/O-driver health entry. The default realtime projection is
  disabled with scheduler `other` in both status and `config.get`.
- Aggregate health is false when any I/O driver is faulted and retains the
  faulted driver's stable error detail; runtime readiness alone cannot hide
  that fault.
- `config.set` uses a closed key, type, and value contract, validates
  cross-field security requirements, and rejects invalid input atomically.
  `runtime.execution_backend` is startup-only and cannot be switched through
  live control. This atomicity is validation-before-logical-mutation; crash-
  atomic filesystem replacement is separate verification depth.
- `historian.query` and `historian.alerts` return an `items` array, honor the
  requested limit, and return matching persisted samples or alerts when they
  exist.

Status, health, and task-stat projection is fail-closed and deterministic.
`status` returns an error when runtime settings, control mode, I/O health,
metrics, or realtime-status state is unavailable; it must not substitute
production mode, VM defaults, an empty driver list, zero metrics, or a default
realtime posture for an internal read failure. `tasks.stats` likewise returns
an error when metrics are unavailable. `health` returns an error when I/O
health or realtime status is unavailable.

The aggregate `health.ok` value is true only when the resource is running,
ready, or paused, has no resource fault, has no faulted I/O driver, and has no
realtime error. A degraded I/O driver or realtime warning remains visible but
does not alone make the aggregate false. Every I/O entry contains its exact
name and one of `ok`, `degraded`, or `faulted`; only degraded and faulted
entries contain their stable error detail. Status preserves I/O registration
order. Task statistics are sorted lexically by task name, independent of map
iteration order. Profiling contributors retain contribution-descending order
with their established lexical tie-break.

`events.tail` and `faults` accept either no parameters or a strict object
containing only optional `limit`. The default is 50 and the accepted range is
1 through 1,000 inclusive. Null, a non-object, an unknown key, a non-integer,
a Boolean, or an out-of-range value rejects instead of silently selecting a
default. Both operations return newest entries first and fail when the event
store is unavailable. `events.tail` applies the limit to all events. `faults`
first selects only `RuntimeEvent::Fault` entries and then applies the limit, so
intervening non-fault events cannot reduce the requested number of faults.
`SafeStateFailed` remains its own event type and is not silently relabeled as
the originating resource fault.

Historian query parameters are strict objects. `historian.query` permits only
`variable`, `since_ms`, and `limit`; `historian.alerts` permits only `limit`.
An omitted object uses limits 250 and 200 respectively. Query limit is in the
range 1 through 5,000 inclusive and alert limit is in the range 1 through
1,000 inclusive. `since_ms` is an exactly representable nonnegative integer.
When present, `variable` is a string that is non-empty after trimming; the
trimmed exact path is used for matching. Unknown fields, null, wrong types,
fractional values, empty variable names, and out-of-range limits reject with
`invalid params` rather than relying on the historian's defensive internal
clamp. A disabled historian still returns `historian disabled` and does not
claim that an invalid query was executed.

`config.set` helper validation is exact and side-effect-free. Boolean fields
accept only JSON booleans. Positive-integer fields accept an exactly
representable signed 64-bit JSON integer in the range 1 through `i64::MAX`;
zero, negative, fractional, Boolean, string, container, and larger unsigned
values reject. Non-empty strings trim Unicode whitespace and reject all-empty
input.

String arrays preserve order, trim every entry, and reject a non-array,
non-string entry, or empty trimmed entry. String maps require an object, trim
both keys and values, reject empty trimmed keys or values and non-string
values, and emit deterministic key order. Empty arrays and objects are valid
where the owning setting permits an empty collection.

Runtime-cloud WAN rules are a strict array of objects containing exactly
`action` and `target`; link-preference rules contain exactly `source`, `target`,
and `transport`. Missing, null, wrong-typed, empty, or unknown fields reject
with the owning key and zero-based entry index. All strings are trimmed.
Transport uses the closed runtime-cloud vocabulary and canonicalizes the
Modbus hyphen/underscore alias. Rule order is preserved.

Validation errors have the stable prefix
`invalid config value for '<key>':`. They may identify an entry or unknown
field but do not serialize sibling values. Helper rejection performs no state
mutation and cannot append an item to either the updated or restart-required
result.

Live `config.set` requires an object; an absent payload and a non-object payload
are distinct errors. An empty object is a successful no-op with empty
`updated` and `restart_required` arrays. The handler stages the complete
effective settings, control token, control mode, and debug-enabled state,
validates every key and all final-state cross-field requirements, and only then
commits any of them. An unknown key, startup-only key, type/value error,
cross-field error, unavailable lock, or scheduler-update rejection leaves all
four state domains unchanged.

The final state may use `web.auth = token` only with a non-empty final
`control.auth_token`. This applies both when token mode is selected and when an
existing token is cleared. A TCP control endpoint that requires authentication
never accepts a null token. Supplying a new token and selecting token web auth
in one object is order-independent. Token and mesh-token values are trimmed,
never returned, and never included in success or error text.

`updated` contains each accepted request key exactly once in canonical lexical
order. `restart_required` is the lexical subset whose effective runtime service
cannot change in place: retain mode; web, discovery, mesh, runtime-cloud, and
control-mode settings. Log level, watchdog settings, fault policy, retain save
interval, control token, and debug-enabled state are live updates and do not
claim restart. A rejected request returns neither list.

`config.get` returns the complete effective key projection without credential
values. Control and mesh credentials expose only presence; control additionally
exposes length when present and null length when absent. Enum-like values use
the same canonical lowercase or snake-case spelling accepted by configuration:
watchdog actions, fault policy, retain mode, control mode, mesh role,
runtime-cloud profile/transport, realtime scheduler, and execution backend.
Derived startup/backend-source, simulation, observability, and credential
presence fields are read-only observations rather than implicit authorization
for `config.set`.

###### Runtime execution-backend selection and construction contract

The host-runtime execution-backend selector trims surrounding whitespace and
matches ASCII case-insensitively. The only accepted value is `vm`, which selects
`BytecodeVm`. Empty or unknown values reject with an invalid-configuration
error that names `runtime.execution_backend` and preserves the rejected text.
The retired `interpreter` value rejects with the distinct migration guidance
that it is no longer supported for production runtimes and that `vm` must be
used.

A freshly constructed `Runtime` selects `BytecodeVm`, has zero elapsed logical
time and zero completed cycles, and exposes the default date/time profile with
epoch tick zero and one-millisecond resolution. Selecting `BytecodeVm` is valid
before a bytecode module has been loaded. For a runtime built from accepted
source, the first VM cycle may materialize the module lazily; that cycle must
complete successfully and expose the source program's resulting state.
When a metrics sink is attached, the snapshot after the reviewed VM cycle
reports `BytecodeVm` as the effective execution backend.

This contract does not authorize another production backend, cross-version
bytecode compatibility, arbitrary source semantics, optimized-backend parity,
or a general performance claim.

###### Runtime historian recording and export contract

An enabled historian in `all` mode records the reviewed global BOOL, integer,
floating-point, and string values with their runtime types. An accepted capture
at time `t` establishes the sample-interval boundary: a second request before
`t + sample_interval_ms` returns zero and adds no sample, while a request at or
after that boundary records the current snapshot. Queries for the reviewed
paths return the recorded values in capture order.

In `allowlist` mode, exact configured names and the reviewed `retain.*` prefix
are recorded; unmatched globals are absent. A file-backed historian reloads
the reviewed persisted sample when a new service instance opens the same
history path. The Prometheus renderer includes the reviewed runtime uptime and
fault totals together with historian sample and alert totals when the historian
snapshot is supplied.

For the reviewed single-variable `above` alert, two consecutive samples above
the threshold satisfy `debounce_samples = 2` and emit one `Triggered` event.
The next below-threshold sample emits one `Cleared` event. A configured file
hook receives exactly one line for each emitted transition, for two lines in
the reviewed trigger/clear sequence.

These cases do not establish arbitrary value conversion, wildcard grammar
beyond the reviewed exact and `retain.*` forms, concurrent writers, crash
durability, retention limits, arbitrary alert expressions or hook transports,
or the complete Prometheus schema.

###### Runtime metrics window and profiling contract

Runtime profiling starts enabled. While enabled, a recorded call is grouped by
its `kind:name` identity, increments its call count, and contributes its elapsed
time to the per-cycle average. A profiling snapshot retains the reviewed call
identities and ranks the reviewed program call ahead of the smaller
function-block call by average cycle contribution.

Disabling profiling clears accumulated call entries and top contributors and
ignores calls recorded while disabled. Re-enabling profiling starts a clean
collection, and a subsequently recorded call appears as the sole reviewed
entry. For the reviewed non-wrapped cycle window containing 5, 10, 15, 20, and
25 milliseconds, the snapshot reports five samples, a median of at least
10 milliseconds, a p95 not below the median, and a maximum of 25 milliseconds.

Cycle, task, and call durations are represented in milliseconds as finite
`f64` values derived from non-negative monotonic durations. The first sample
sets minimum, maximum, average, and last to the same value. Later samples update
minimum and maximum, set last to the newest value, and compute the arithmetic
mean over every accepted sample. An unrecorded statistics object exposes zero
for every duration field and zero samples/calls. Recording a zero duration is a
real sample, not an absent value.

Cycle percentile reporting uses a rolling window of the most recent 512 cycle
durations. The 513th sample evicts only the oldest sample. Snapshots sort a copy
of the retained values and do not mutate collection order or future rollover.
For a sorted population of `n`, p50, p95, and p99 use index
`round((n - 1) * q)` for quantile `q`; quantiles below zero clamp to zero and
above one clamp to one. An empty window reports zero percentiles, zero maximum,
and zero samples. Every non-empty snapshot reports the exact retained sample
count, p50 <= p95 <= p99 <= maximum, and finite values.

Task identities are exact and independent. A recorded task duration creates or
updates only that task. A positive missed-activation count increments both the
runtime-wide overrun counter and that task's overrun counter with saturating
`u64` arithmetic. A zero missed count is a no-op and does not materialize an
otherwise absent task. Fault counts also saturate. Snapshot task rows are
sorted by task name so identical runtime state has deterministic JSON and
control-surface projection regardless of hash-map insertion order.

Call identities are the exact `kind:name` pair. Each call row reports its own
minimum, maximum, mean, last duration, call count, and total call time divided
by the number of recorded cycles as `avgCycleMs`. When no cycle has been
recorded, the denominator is one so a recorded call remains visible, while its
cycle percentages are zero because no cycle budget exists. Multiple calls with
the same identity aggregate; different identities never merge.

Top contributors contain at most five call identities, ordered by descending
`avgCycleMs` and then ascending identity key for ties. `cyclePct` is
`avgCycleMs / cycle.avgMs * 100`; `lastCyclePct` is
`call.lastMs / cycle.lastMs * 100`. A zero cycle denominator yields zero rather
than NaN or infinity. Percentages are not clamped to 100 because nested or
overlapping instrumentation may legitimately sum beyond one cycle budget.

Setting profiling from enabled to disabled clears all accumulated call rows and
contributors. Calls while disabled are ignored. Repeatedly setting the already
disabled state remains empty; repeatedly setting the already enabled state
preserves current samples. Re-enabling after a disabled interval begins with an
empty profile and does not reconstruct ignored calls. Cycle, task, fault, and
overrun metrics are independent of the profiling toggle.

A metrics snapshot is observational: it does not reset counters, consume
samples, reorder retained window state, or alter profiling enablement.
Successive snapshots without new records preserve all metric values except
that monotonic uptime may increase. The default and explicitly selected
execution backend are reported exactly.

These rules establish the in-memory accounting and snapshot contract. They do
not claim wall-clock accuracy, lock-free concurrent collection,
instrumentation completeness, a stable observer-overhead budget, or a general
execution-time guarantee.

###### Runtime terminal console contract

The optional `trust-runtime ui` terminal console is an operator-facing
projection of the versioned control protocol. It does not own a second runtime
state model and it does not make a successful control operation out of a
failed, malformed, or unavailable control response. These are truST product
rules outside IEC 61131-3.

Endpoint selection is deterministic. An explicit endpoint is parsed and used
without requiring a bundle. Otherwise the selected bundle supplies the
endpoint and optional configured authentication token. An explicit token has
precedence over the environment and bundle; a nonempty environment token has
precedence over the bundle. Empty token text is absence, not a credential.
Every request is one newline-delimited JSON object and carries the selected
token in `auth`. The refresh transaction requests, in order, `status`,
`tasks.stats`, `io.list`, `events.tail` with limit 20, and `config.get`, using
distinct request IDs 1 through 5. Transport or decoding failure aborts that
refresh; the console retains the last complete snapshot instead of installing
a partial one.

Response projection has closed fallback behavior:

- status requires an object result and a string state; absent optional
  counters and timings are zero, the resource is `resource`, the fault is
  `none`, the control mode is `unknown`, and simulation defaults to production
  at scale one;
- task, I/O, and event projections preserve response order and return an empty
  list when `result` is absent or not an array;
- I/O string values remain unquoted while non-string JSON scalars use their
  JSON spelling;
- event levels `fault` and `error` map to Fault, `warn` and `warning` map to
  Warn, and other or absent levels map to Info;
- settings require a result object and use the documented runtime defaults.
  Negative or unrepresentable cycle intervals and simulation scales are
  rejected as absent and defaulted rather than narrowed.

Console configuration is advisory. An absent, unreadable, or malformed
`runtime.toml`, an absent console table, and a layout containing no recognized
panel names all select the built-in layout. Recognized layout names are
case-insensitive, retain declared order, and discard unknown entries. A
positive `console.refresh_ms` overrides the CLI default only when the caller
left that default unchanged; zero and negative refresh values are invalid and
do not create a busy loop.

Prompt editing is UTF-8 safe: the cursor always denotes a character boundary,
left/right/backspace operate on one complete Unicode scalar, and no accepted
key sequence may panic. Activation positions the cursor at the end of the
initial text; deactivation clears transient history navigation. Empty and
whitespace-only commands are not retained. History traversal saturates at the
oldest entry, advances toward newer entries, and clears the input after the
newest entry. Suggestion traversal wraps in both directions, and selecting a
suggestion records and executes that exact slash-prefixed command.

Read-only mode accepts only observation and exit interaction. `/` reports
read-only state without activating a prompt, and `q` exits. It must not issue
pause, resume, stepping, restart, shutdown, I/O, configuration, or other
mutating requests. Beginner mode exposes only its documented beginner command
set and disables direct debug controls. A confirmation performs an action only
for case-insensitive `y` or `yes`; every other response cancels. Escape and
control-C leave a normal inactive prompt without executing the pending
operation.

The cycle sparkline retains at most 120 finite nonnegative samples, rounded to
tenths of a millisecond with zero rendered as the minimum visible bucket.
Absent, NaN, infinite, or negative cycle values do not become fabricated large
samples. Alerts retain at most five newest entries. Event alert deduplication
uses the complete event identity (code, timestamp, level, and message), so a
new occurrence reusing a code is not hidden; informational events do not
produce alerts. Watch evaluation preserves watch-list order and classifies a
missing result as `unknown`, a protocol error as `error: <message>`, and a
transport failure as `unavailable`.

Command and menu selection is deterministic. Command names are the documented
lowercase tokens, optional leading `/` is accepted, numeric menus are
one-based, and invalid or out-of-range choices retain the current mode with an
explicit error. Navigation wraps over the currently eligible entries. I/O
Read may select any listed point; Set, Force, and Unforce offer output points
only. Boolean values receive the TRUE/FALSE chooser, while other values enter
the explicit value prompt. A local force marker is added or removed only after
the corresponding control response succeeds. `unforce all` retains markers
whose release failed and reports the failure; it must not claim that all
forces were released after a partial failure.

Settings distinguish live acceptance from durable project mutation. Boolean
inputs accept the documented true/false aliases; TOML scalar parsing trims
text and preserves Boolean, signed integer, or string type. Nested TOML update
creates missing tables but rejects traversing a scalar. A project-backed
setting is reported saved only after the candidate TOML validates and the file
write succeeds. A control-backed setting is reported saved only after the
control response is successful; when both live and durable updates are
required, either failure is visible and success is not fabricated. Restart
menus open only for an accepted change that actually requires restart.

These unit-level rules prove console parsing and state transitions. They do not
replace an interactive terminal smoke test for rendering, terminal restoration,
or a live runtime control connection.

The role and transport rules in section 6.9.4 govern these handlers. A pairing
token retains its stored role; an unauthenticated network connection receives
Viewer authority; and a local mode-0600 Unix socket without configured
authentication may receive Admin authority. Sufficient role permits a request
to reach its handler but does not override a configured HMI read-only posture.

###### Runtime HMI descriptor, live-state, and control contract

The runtime HMI schema is the authority for every exposed widget and process
binding. A schema row has a stable widget ID and path, declared type, current
quality, writable flag, and any configured display, unit, range, alarm, or
layout metadata. Descriptor overrides may change presentation without changing
the stable identity of an unchanged binding. Unknown paths, unknown widgets,
type mismatches, and invalid limits are reported as binding errors; they are not
silently dropped or coerced.

HMI values are read directly from the runtime snapshot and preserve the
declared runtime type, timestamp, and quality. The in-process read and write
ports use the same typed contract as the JSON control surface but do not
serialize and parse an intermediate JSON request. A write is admitted only
when all of the following are true:

- HMI writes are enabled and the runtime is not in HMI read-only mode;
- the target resolves by stable widget ID, canonical path, or configured alias;
- the resolved target is in the explicit write allowlist and the schema marks
  it writable; and
- the value passes the same declared-type, subrange, bounded-string, and finite
  floating-point checks as other runtime writes.

An admitted write is queued for the cycle boundary and audited through the HMI
policy path. A rejected write queues nothing and leaves runtime storage
unchanged. The queue-processing work remains bounded by the configured cycle
budget.

Descriptor updates are atomic at the schema boundary. A valid descriptor or
scaffold reset increments the schema revision and becomes visible without a
runtime restart. Rapid successive file changes must not deadlock the watcher.
An unreadable or invalid descriptor retains the last good schema and exposes
the validation failure instead of publishing a partial schema. When descriptor
files are absent, the runtime may publish the deterministic inferred layout.

The live event stream emits changed values only, includes stable widget IDs and
the active schema revision, and deduplicates identical alarm payloads. Trend
downsampling preserves the requested time window and its first, last, minimum,
and maximum bounds. Alarm state uses the configured alarm label and follows the
ordered lifecycle raise, acknowledge, clear, and history retention. A configured
deadband requires the value to re-enter the clear window before the alarm
clears. When HMI persistence is configured, restart restores only the bounded
trend window and alarm history owned by that configuration; it does not expand
the configured per-series limit. HMI control failures use a structured error
payload with a stable code; clients do not need to parse diagnostic prose.

###### Runtime control audit and debug projection

Every accepted or denied control operation attempts to emit its audit record.
If the audit sink is unavailable, the request retains its normal semantic
result and audit identity, and the runtime emits an `AuditDropped` event; it
must not pretend durable audit succeeded. A request for a compiled-out or
disabled feature returns the stable code `feature_disabled` and emits a
`FeatureDisabled` event.

Debug variables and evaluation expose the declared public type name and value,
not internal instance IDs. A stale frame, unknown variable reference, unknown
evaluation name, or unavailable/poisoned I/O snapshot fails closed with a
negative response. These failures do not return an empty successful projection
or mutate debug or runtime state.

###### Runtime control I/O and variable mutation contract

The attach-mode JSON control surface uses strict parameter objects for debug
mutation. `io.write` and `io.force` require exactly string fields `address` and
`value`; `io.unforce` requires exactly string field `address`. `eval` requires
exactly string field `expr`; `set` requires exactly string fields `target` and
`value`; `var.force` requires exactly string fields `target` and `value`; and
`var.unforce` requires exactly string field `target`. Missing parameters use
the stable `missing params` failure. Null, a non-object, a missing field, a
wrong-typed field, or an unknown field rejects as `invalid params`. Rejection
queues no write and changes no force.

Control I/O addresses use the canonical uppercase `%I`, `%Q`, or `%M` grammar
after trimming surrounding whitespace. A mutation address is concrete:
wildcards reject. Flat addresses must fit the 16,777,216-byte area, including
the complete width of byte, word, double-word, and long-word accesses.
Hierarchical addresses retain every declared path segment and bit coordinate.
The same address eligibility applies to write, force, and release; release
cannot use an address that write or force would reject merely because no new
value is supplied.

`io.list` returns the cached snapshot directly and fails with
`no snapshot available` before the first captured scan. `io.read` returns
`{"snapshot": null}` for that legitimate pre-scan state. If the snapshot lock
is unavailable, both operations fail with `I/O snapshot unavailable`; they do
not collapse an internal failure into a pre-scan state. A returned snapshot is
one coherent cached scan, including only the force marks captured with that
scan.

The attach-mode scalar text grammar is deliberately small. After trimming,
`TRUE` and `FALSE` match ASCII-case-insensitively and produce Boolean values.
A base-ten signed integer accepted exactly by the `i64` domain produces a
`LINT`; optional leading sign and leading zeroes are accepted, while an empty
string, separators, base prefixes, decimal points, exponent notation, quoted
text, and out-of-range integers reject as `unsupported value`. This admission
grammar does not bypass target-width or declared-type checks when the queued
mutation is applied at a scan boundary.

An accepted `io.write` appends one one-shot request in arrival order. Multiple
writes to the same address remain distinct ordered requests. An accepted
`io.force` creates an active force; forcing the same exact address again
replaces its value in place without duplicating or reordering the force.
`io.unforce` removes only the exact matching force and is idempotent when no
such force exists.

The bounded `eval` operation trims `expr`, rejects an empty name, and performs
an exact snapshot lookup: global storage first, then retained storage. It is a
name read, not general expression execution. An absent snapshot returns
`no snapshot available`; an unknown name returns `unknown identifier`.
Successful values use the stable runtime debug representation.

Variable mutation targets use exact lowercase prefixes. `set` admits
`global:<name>` and `retain:<name>` only. Force and release additionally admit
`instance:<id>:<name>`, where `id` consists only of ASCII decimal digits in
the inclusive `u32` range. Surrounding whitespace on the complete target is
not ignored because it changes the prefix; whitespace around the name after
the delimiter is trimmed. Every name must remain non-empty. Instance names
retain all text after the second delimiter, after surrounding whitespace is
removed.

An accepted `set` queues a one-shot variable write for the next scan. A later
pending write to the same exact target replaces the earlier value in place;
different targets retain arrival order. Force uses the same target identity
and scalar grammar. `var.forced` returns active variable forces in stable
insertion order with canonical targets `global:<name>`, `retain:<name>`, or
`instance:<normalized-u32>:<name>` and stable debugger-formatted values.
Re-forcing one target replaces its value without duplication. Release removes
only the exact target and is idempotent.

##### 6.8.1a Offline communication authoring client

`trust-runtime comm` is a scriptable local authoring surface. Each accepted
subcommand delegates to exactly one matching offline communication operation
and emits that operation's JSON result:

- `schema` optionally limits the schema request to the selected protocol;
- `topology` reads the selected project and reports its offline fleet topology;
- `apply` requires `--params` to decode as a JSON object, forwards the selected
  protocol and action, and includes `instance_id` only when one was supplied;
- `discover` forwards the selected protocol, origin, passive flag, and only the
  optional scope fields that were supplied by the caller;
- `browse-symbols` accepts an optional JSON-object target, optional cached JSON
  snapshot, instance, and connection name, and selects project-aware browsing
  only when a project was supplied;
- `opcua-trust list` reports the selected client PKI directory and its trusted
  server certificates, while `opcua-trust clear` removes those trusted
  certificates and reports the number removed.

`comm discover` defaults to passive read-only discovery when `--passive` is
omitted and preserves omitted host, adapter, and other optional scope fields as
absent. `comm browse-symbols` defaults `kind` to `symbols` and preserves omitted
project, snapshot, and instance inputs as absent.

Malformed JSON, a non-object `--params` or target, an unreadable or malformed
snapshot file, and any delegated operation failure are command failures. They
must not be printed as successful communication results. These offline CLI
rules are truST host-product behavior outside IEC 61131-3 and are neither an
IEC decision nor an IEC deviation.

The default runtime template consumed by offline topology and apply operations
must itself satisfy current runtime configuration validation. An unset optional
mesh authentication token is omitted; the template must not serialize an empty
secret value that the parser rejects even while mesh is disabled.

###### Communication schema, capabilities, and Test

Communication schema and capability responses use schema version 4. Protocol
filtering returns only the requested protocol. Every schema entry has a stable
protocol ID, category, apply mode, configuration owner, multi-instance
capability, and typed field definitions. Secret fields are marked as secret
metadata, have no secret default, and never serialize a credential value.

The schema registry is a closed, deterministic product contract. Its protocol
order is `modbus_tcp`, `mqtt`, `ethercat`, `gpio`, `simulated`, `loopback`,
`opcua`, `opcua_client`, `openot`, `discovery`, `mesh`, `realtime_t0`,
`runtime_cloud`, `ads_server`, and `ads`. Protocol IDs and action IDs are
unique. Filtering trims surrounding whitespace, folds ASCII case, and treats
hyphens as underscores; an unknown string selects no protocols, while a
non-string protocol filter is malformed and must not silently return the
unfiltered registry.

Each protocol has non-empty title, purpose, availability, primary category,
configuration home, apply mode, lifecycle effect, and at least one category
containing its primary category. The closed configuration homes are
`io.toml`, `runtime.toml`, `opcua_client.toml`, and `ads.toml`; every protocol
must route to the apply implementation that owns that home. The closed action
vocabulary is `add`, `edit`, `upsert`, `remove`, `disable`, `discover`,
`browse_symbols`, `test`, `doctor`, and `route_script`. Actions are unique per
protocol. `supports_test` is true exactly when `test` is advertised; a schema
must not expose one half of a usable Test control.

Field IDs are non-empty and unique within a protocol. Labels and help are
non-empty. The closed field-type vocabulary is `string`, `secret`, `boolean`,
`number`, `enum`, `endpoint`, `path`, `json_array`, and `json_object`, and a
default has the matching JSON shape. Required fields have a non-null default.
Only `secret` fields carry `secret = true`; their default is always null and
they are never required. Enum options are non-empty, unique strings containing
the default. Number validation uses a complete integer range with `min <= max`
and a default within that range. Endpoint validation is either `host_port` or
`socket_addr`. A visibility predicate references another field in the same
protocol and its comparison value is valid for that controlling field.

Schema defaults and apply validation are one contract, not separate sources of
truth. Every directly applicable default set is accepted by its owning
validator. Every advertised integer bound, enum option set, endpoint shape,
required string, array shape, and object shape is enforced by that same apply
route. ADS-client and OPC-UA-client connection arrays are intentionally empty
authoring seeds: while `enabled` is true, they are incomplete until the user
selects at least one connection and point/node, and the apply route must reject
activation without that selection.

Configured `io.toml` instances retain stable protocol-qualified IDs and appear
only under their matching protocol. Instance parameters are an observation
projection, not a credential retrieval surface: secret-valued keys such as
passwords, tokens, and private keys are omitted or replaced with a non-secret
presence marker before serialization. Schema filtering and configured-instance
projection must never cause a persisted credential to appear in a response,
diagnostic, or display name.

The version-4 capability registry contains exactly the 15 registered
communication protocols. Build availability, platform availability,
configuration state, and operational evidence are distinct. Every protocol
shown by default on the current platform must have a built backend; an unknown
platform marker fails closed instead of silently removing that requirement.

A communication Test request may succeed at the control-envelope level while
returning `result.ok = false` for a completed negative protocol test.
Unsupported Test operations return `supported = false` and must not appear as
usable controls.

`comm.test` requires a non-empty string protocol and an object-valued `params`;
an absent request or protocol fails at the control-envelope boundary, while a
present non-object `params` value returns a structured `params` field error
before protocol work rather than being reinterpreted as an empty target.
Protocol normalization follows the communication-schema rules. The supported set is exactly Modbus TCP,
MQTT, OPC UA client, simulated I/O, and loopback I/O. Simulated and loopback
return successful `local_driver` evidence without opening a socket. EtherCAT
and GPIO return a stable hardware-health explanation with `supported = false`.
Every other registered or unknown protocol returns the stable unsupported
result without network activity, evidence, or field errors.

Modbus and MQTT Test are bounded TCP-reachability probes, not protocol
handshakes. Their positive detail and evidence therefore claim only that the
resolved port accepted a TCP connection. Modbus accepts a host, IPv4 address,
or bracketed IPv6 address with an optional port and defaults an omitted port
to 502. MQTT accepts the `mqtt://` and `tcp://` cleartext schemes with default
port 1883 and the `mqtts://` and `ssl://` TLS schemes with default port 8883.
An explicit valid port is retained. Empty hosts, zero or invalid ports,
userinfo, paths, queries, fragments, unsupported schemes, and ambiguous
unbracketed IPv6 are field errors before DNS or connection activity. A
resolution failure is a field error; a resolved but refused or timed-out
connection is a completed negative Test with target, resolved address, and
effective timeout evidence.

The Test timeout defaults to 500 ms and is bounded to 1 through 5,000 ms.
Zero and values above the maximum clamp to the nearest bound. Negative,
fractional, string, object, array, and boolean timeout values are field errors;
they must not silently select the default. The effective bounded timeout is
reported in evidence. Address resolution and connection attempts share that
single total budget and try resolved addresses in deterministic resolver order
until one succeeds or the budget expires.

OPC UA client Test accepts `endpoint_url` or the `host` alias. A bare host
defaults to `opc.tcp://<host>:4840`; an explicit port or path is retained
under `opc.tcp://`, and an already canonical URL is byte-preserved after
trimming. Security policy and mode use the OPC UA configuration vocabulary.
Authentication is `anonymous` or the `username`, `user_name`, and `user`
aliases; username authentication requires non-empty trimmed username and
password values. The trust-server-certificate flag defaults false. A target
validation failure is field-specific and starts no handshake. A completed
handshake failure retains its stable OPC UA error classification and
credential-free endpoint/security evidence.

Any non-empty nested `password`, `auth_token`, `token`, `secret`, or
`client_secret` value requires the exact server-observed
`trusted_same_host` credential channel. Empty secret strings do not create a
credential. A blocked Test has no evidence or protocol error, never echoes
request parameters, and returns only a secret-presence-safe field error.

Communication capability `health` and fleet topology runtime, endpoint, and
link status are versioned product-projection vocabularies, not the
connector-report `health` vocabulary in
[Connector Status](23-connector-status.md). Capability health is one of
`not_in_build`, `not_configured`, `simulate`, `runtime_unreachable`,
`connected`, `degraded`, `error`, or `configured_policy`. Consumers must
interpret values within their response type and reject unknown values rather
than treating them as healthy.

The capability response order is `ads`, `ads_server`, `opcua`,
`opcua_client`, `modbus_tcp`, `mqtt`, `openot`, `discovery`, `mesh`,
`realtime_t0`, `runtime_cloud`, `ethercat`, `gpio`, `simulated`, and
`loopback`. Capability IDs, health values, platform values, and next-action
kinds are closed serialized vocabularies. Platform is `unix` for OpenOT and
EtherCAT, `linux` for GPIO and realtime T0, and absent for protocols without a
platform restriction.

Capability booleans and health are coherent. `operational = true` requires
`built = true`, `configured = true`, and `health = connected`; `connected`
likewise requires all three booleans true. `not_configured` requires
`configured = false` and `operational = false`. `not_in_build` requires
`built = false` and `operational = false`, while `configured` may remain true
because persisted configuration and build availability are independent.
`configured_policy` requires `configured = true` and `operational = false`.
Configured connectors with unavailable live evidence report `degraded` or
`configured_policy`, never `connected`.

An I/O capability matches its canonical driver name case-insensitively and
accepts the schema spelling alias with hyphens and underscores normalized.
When multiple live instances belong to one protocol, the projection is
conservative across the complete matching set: any faulted instance yields
`error`; otherwise any degraded instance yields `degraded`; only an all-healthy
non-empty set yields `connected`. Detail text must not hide a worse matching
instance merely because a healthier instance was registered first.

ADS client capability maps `healthy` to `connected`, `faulted` to `error`,
`disabled` to `not_configured`, and `degraded`, `not_ready`, or `unknown` to
`degraded`. OPC UA client capability is `not_configured` without configured
connections, `degraded` when configured status is unavailable,
`configured_policy` before a live read exists, `error` if any connection is
faulted, `connected` only when every connection is connected with zero
degraded points, and otherwise `degraded`. Build-disabled projections retain
the independent configured fact but are never operational.

The next-action projection is deterministic. Connected health uses `none`;
not-in-build uses `get_build_with_feature`; simulation uses
`switch_to_online`; runtime-unreachable uses `open_runtime_pane`;
not-configured and configured-policy use `setup`; and degraded or error uses
`diagnose_ads` for ADS and `setup` for other protocols. Action labels are
non-empty.

Read-only communication audit details are recursively credential-free.
Credential key matching is ASCII-case-insensitive and covers `password`,
`auth_token`, `token`, `credential`, `credentials`, `secret`, and
`client_secret` at every object depth, including objects inside arrays.
Non-secret protocol, target, scope, snapshot-metadata, and selection fields are
preserved. Audit sanitization never replaces a secret with its value, embeds it
in detail text, or relies on the caller placing credentials at a particular
depth.

###### Communication apply mutation and security

Server-observed transport identity determines whether a credential channel is
trusted; a caller cannot self-assert trusted status. Secret-bearing input on an
untrusted channel is rejected before mutation. Responses, diagnostics, and
audit records may report secret-field presence but never secret values.
Authorization and complete request validation precede mutation. Field errors
are stable and cause no configuration write to be attempted.

Dry-run performs the same validation and lifecycle planning without writing or
deleting files and without returning a secret-bearing snippet. Add, upsert,
edit, remove, and disable preserve unrelated instances, project comments
outside rewritten driver/safe-state items, and safe-state values unless an
operation explicitly owns that data. Edit requires
one exact selected instance. Instance IDs are protocol-qualified; missing,
stale, cross-protocol, and wrong-slot IDs fail closed. Repeating an identical
edit is byte-identical. Disable retains the selected instance with
`enabled = false` so topology can show it as disabled.

A valid single-driver file may migrate to canonical multi-driver form when
another driver is added. Empty, comment-only, and safe-state-only files are
valid authoring inputs, but successful output must satisfy runtime activation
validation. Removing the final project driver preserves the project file,
project comments outside rewritten driver/safe-state items, and exact
safe-state values, writes an explicit `driver = "none"` with empty parameters,
and must not activate system-I/O fallback. A later Add replaces that sentinel
while preserving the retained project policy.

Runtime-backed protocols update `runtime.toml` and their owned sidecars as one
validated authoring operation. Audit records include operation, protocol,
result, dry-run state, and secret presence without secret values.

The runtime-file authoring boundary is project-confined and transactional.
Every configured sidecar path is trimmed, must be relative to the selected
project root, and must remain below that root after lexical normalization and
existing-parent canonicalization. Absolute paths, empty paths, `..` escape,
and symlink escape are rejected before any directory or file is changed.
`runtime.toml` and every owned ADS or OPC UA sidecar are rendered completely
in memory, parsed by their production configuration parsers, and staged before
publication. A successful operation replaces the complete output set; a
failure staging, validating, creating, writing, syncing, renaming, or removing
any member preserves the complete pre-operation set. It must never publish a
new `runtime.toml` that points to an absent, stale, or partially written
sidecar.

Dry-run and Validate perform the same normalization, path-confinement,
rendering, and production-parser checks but create no directories, temporary
files, sidecars, or runtime file. Add, Edit, and Upsert are byte-idempotent for
identical normalized input. For protocols with an `enabled` field, Disable
retains the existing owned configuration and changes only that field; Runtime
Cloud, whose absent section denotes disabled policy, removes its cloud
section. Remove deletes the runtime section and its owned sidecar in the same
transaction; a missing sidecar is an idempotent success. Malformed or
unreadable existing input fails closed without replacing it. Unrelated runtime
sections, comments, and ADS-server clients not supplied by an update remain
intact. No failure response, audit entry, temporary-file name, or persisted
document includes a secret value.

###### Offline communication operation and fleet-topology truth

Offline schema, apply, and topology require no live runtime and must not invent
live evidence. Offline responses remain schema version 4. ADS authoring writes
coordinated `runtime.toml` and `ads.toml`; OPC UA client authoring writes
coordinated runtime and sidecar state, retains disabled configuration on
disable, and removes the selected section and sidecar on remove. Configured
OpenOT appears as configured evidence without a live payload. Offline results
exclude passwords, tokens, source allowlists, and other secret-bearing fields.

Offline topology requires a readable valid `runtime.toml`. A missing optional
`io.toml` is an empty configured-driver set; a present malformed or unreadable
file is an error rather than an empty topology. An enabled ADS or OPC UA client
pointer requires its readable valid project-confined sidecar. Disabled pointers
do not read or require their sidecars. Relative sidecars resolve from the
selected project; absolute paths and normalized or symlink escapes reject
before a read.

The offline host and runtime are configuration evidence: source is `config`,
runtime mode is `stopped`, health is `configured_policy`, and host/runtime live
timestamps, runtime load, endpoint live payloads, and host temperature, uptime,
and load are absent. Configured service and enabled-driver endpoints remain in
configuration order with their offline detail and policy/disabled health.
Offline links are sorted and deduplicated by stable link ID. A parser or
sidecar failure reports the owned file and reason but never includes a secret
value or publishes a partial topology.

Fleet topology identity and roles come from the selected project and current
runtime evidence. Configuration determines runtime identity, endpoint roles,
counterparts, stable link IDs, and external-node relationships. Merely enabling
a service projects `configured_policy`, not a live connection. Live values and
`connected` status require bound runtime evidence; degraded and faulted evidence
retains its detail. Every link reports stable ID, source, destination, protocol,
role, direction, and evidence-derived status. Fleet output never serializes
MQTT, mesh, ADS, OPC UA, or other credentials.

Fleet topology schema version 4 contains `hosts`, `links`, `shared`, and
`external` arrays and omits an empty `discovered` array. IDs are deterministic
ASCII projection keys. The common sanitizer lowercases ASCII alphanumeric
characters, replaces each other character with a hyphen, trims boundary
hyphens, and returns `unknown` if no alphanumeric character remains. Local host
identity prefers the first non-empty normalized value from `HOSTNAME`,
`COMPUTERNAME`, and the operating-system hostname; normalization trims
whitespace and trailing DNS dots. A container identity is the first
reverse-scanned cgroup component containing at least 12 hexadecimal characters,
or the hexadecimal component of a `docker-<id>.scope` record, truncated to 12
characters. Short, non-hexadecimal, and empty components do not become
container IDs.

Configured host IP projection parses valid listen values as hosts, removes
unspecified wildcard addresses, sorts and deduplicates the result, and falls
back to `127.0.0.1` only when no concrete host remains. Bracketed IPv6 is
unbracketed in the host inventory. Whenever an IP address is combined with a
port for an endpoint, IPv6 is bracketed so the result is an unambiguous socket
authority; IPv4 and hostnames use `host:port`.

Discovery timestamps floor nanoseconds to milliseconds. An entry is stale only
when its age is strictly greater than 120,000 ms; future timestamps remain
fresh by saturating age at zero. The newest observed timestamp is the maximum
over the complete entry set and is absent for an empty set. An entry is self
when either its runtime name or discovery ID equals the current runtime ID and
is excluded from discovered-host and discovered-node projections.
Discovery-derived protocol order is `discovery`, then `web`, `mesh`, and
`control` when those advertised surfaces exist. Discovery IDs use the
sanitized runtime name for runtime nodes, the host-group when supplied for
host nodes, and otherwise the discovery ID. Shared host identity requires a
matching non-empty host group on a self entry; name similarity or address
overlap alone does not claim same-host placement.

Configured I/O endpoints retain configuration order and use the configuration
index in their stable endpoint ID. The first protocol instance omits an index
suffix; later instances include their zero-based configuration index. Driver
protocol normalization trims and folds ASCII case, maps hyphens to
underscores, and canonicalizes both `modbus-tcp` and `modbus_tcp` as
`modbus_tcp`. Roles are `client` for Modbus TCP and MQTT, `master` for
EtherCAT, and `owned_driver` otherwise. Display names use the trimmed `address`
or `broker` when present; otherwise the first configured instance uses the
protocol display name and later instances append their one-based ordinal.

An enabled configured I/O endpoint binds to the corresponding enabled runtime
health row while disabled configured rows consume no live-health ordinal.
Healthy, degraded, and faulted driver evidence map to `connected`, `degraded`,
and `error` with the underlying detail retained. Missing health is
`configured_policy`; a disabled endpoint is `disabled`, has no live sample,
and does not advertise Test. Test support is true exactly for an enabled
canonical Modbus TCP or MQTT endpoint, regardless of accepted hyphen,
underscore, or ASCII-case spelling. Health-only endpoints use the same
canonical protocol, role, health, ID, and Test rules.

I/O snapshot live evidence exists only when both a snapshot and a nonzero
observation timestamp exist. It reports exact input, output, and memory counts
and at most eight deterministic samples in input-then-output-then-memory order.
Each sample retains direction, optional name, canonical IEC address, and either
the debug-form scalar value, an error object, or `unresolved`. Bit, byte, word,
double-word, long-word, fixed-byte, and wildcard addresses preserve area and
size without inventing a bit coordinate for non-bit values.

Fleet endpoint parameters recursively preserve the TOML JSON shape while
redacting secret or access-control values at any depth, including tables inside
arrays. Credential matching is ASCII-case-insensitive and covers `password`,
`auth_token`, `token`, `credential`, `credentials`, `secret`,
`client_secret`, and `private_key`. ADS source pins and allowlists
(`source_ip`, `source_cidr`, `allowed_clients`, and `clients`) are likewise
redacted rather than exposed as editable topology data. A redacted value is the
literal non-secret marker `<redacted>`; no sibling value or container shape may
reveal the original credential.

ADS and OPC UA client endpoint projections retain the configured connection
denominator and order. ADS parameters expose route name, target identity,
host, AMS port, whether a local identity is set, transport class, automatic
route policy, and point mapping, but never a route credential. Symbol points
and index-group points remain distinct. OPC UA parameters expose endpoint,
security policy and mode, authentication class, a Boolean username-presence
fact, trust policy, timing, and point mapping, but never username credentials
or a password. Read, write, and read-write access serialize exactly and the
writable flag follows access rather than live status.

Live connection evidence matches ADS configuration by exact connection name
or target AMS Net ID plus AMS port, and OPC UA configuration by exact
connection name or endpoint URL. Unmatched live rows cannot upgrade a
configured connection. Exact name identity takes precedence over a fallback
target or endpoint match so one stale row cannot cross-bind two configured
connections. A missing report or a report with no matching row is
`configured_policy`; a matching fault dominates as `error`; otherwise any
non-connected state or degraded point yields `degraded`; only a non-empty set
where every configured connection has a matching connected row with zero
degraded points is `connected`. A partial match is therefore degraded rather
than connected or configured-only.

ADS live state labels come from the ADS diagnostic contract. OPC UA state
labels are `disabled`, `configured_policy`, `connecting`, `connected`,
`reconnecting`, `stale`, and `error`. Per-connection topology health maps
connected with zero degraded points to `connected`, faulted to `error`,
connected with degraded points and all transitional live states to
`degraded`, and disabled/configured to `configured_policy`.

Live summary counts, connection arrays, and freshness timestamps are computed
only over the configured connections and their matched status rows. Extra or
stale status rows from an earlier configuration do not affect `connected`,
`total`, or `last_seen_ms`. Each configured connection remains present even
when unmatched, with null live fields. Freshness is the maximum matched
per-connection or per-point observation; when a report exists but supplies no
timestamp, the projection may use the projection time but must not claim an
unmatched row's timestamp.

Fleet link IDs are
`link:<sanitized-protocol>:<sanitized-role>:<sanitized-from>:<sanitized-to>`.
The link retains its unsanitized source, destination, protocol, role,
direction, detail, same-host fact, status, and security fact separately.
Configured links remain present while disabled or while live evidence is
missing; their status is respectively `disabled` or `configured_policy`.
Healthy, degraded, and faulted driver evidence maps to the same link status as
its endpoint.

Configured Modbus targets project an outbound client link to an external device
identified by the trimmed address. Configured EtherCAT adapters project an
outbound master link to an external fieldbus segment identified by the trimmed
adapter. Empty or non-string targets do not create external nodes or links.
Modbus link security follows its Boolean TLS configuration; EtherCAT does not
claim transport security. MQTT brokers project a publish-subscribe link to one
shared broker node per exact trimmed broker address, sorted deterministically,
with the runtime listed once in `used_by`; broker security follows its Boolean
TLS configuration.

ADS and OPC UA client links retain configured connection order and remain
`configured_policy` without a matching status row. ADS security follows the
route transport, and OPC UA security is false only for policy `none`. ADS
external identity uses the target AMS Net ID; OPC UA external identity uses the
sanitized endpoint URL. Their link status follows the matching per-connection
health contract above and never derives from an unrelated status row.

Configured mesh targets are `configured_policy` without mesh evidence,
`connected` only when the exact target or its embedded peer identity is in the
live peer registry, `degraded` when the mesh session is ready but that peer is
absent, and otherwise `configured_policy`. Merely having a configured target
does not prove a live mesh link.

Shared MQTT nodes are grouped by exact trimmed broker address in lexical order.
Duplicate configured drivers contribute the local runtime ID once. Drivers
without a non-empty broker create neither a shared node nor a shared link.

External-node projection order is configured mesh targets, unresolved runtime
cloud targets, observed discovery peers, configured driver targets, ADS
targets, and OPC UA targets. IDs are unique in the result and the first
authoritative source for an ID wins. A runtime-cloud link with an explicit
managed target does not create a fabricated external placeholder; an empty
target creates the stable indexed policy placeholder. Self-discovery entries
are excluded from external, discovered, host, runtime, endpoint, and link
populations.

Each non-self discovery peer appears once in the `discovered` inventory with
its exact addresses, advertised optional control/web/mesh fields, stable
protocol order, `observed` direction, `adopted = false`, discovery source, and
floored last-seen timestamp. Duplicate observations with the same discovery
identity collapse without reordering the first observation.

Discovered entries sharing a non-empty host group coalesce into one host node.
That host retains first-observation order, a sorted deduplicated IP union, the
newest host timestamp, and one runtime child per unique runtime identity in
observation order. Entries without a host group use independent
discovery-identity hosts. A discovered runtime is `connected` while fresh and
`runtime_unreachable` while stale. Advertised web and mesh endpoints are
observations, not listener probes; web follows runtime freshness, while mesh is
`connected` only with ready matching mesh evidence and otherwise
`configured_policy`. Discovered endpoints are unowned and never advertise
Test.

##### 6.8.1b ADS CLI authoring and server-control contract

`trust-runtime ads` is the scriptable ADS authoring, validation, route, and
server-control surface. Offline import canonicalizes cached symbol snapshots,
generates deterministic ST, creates missing parent directories, reports an
unchanged byte-identical output without rewriting it, and refuses to replace
different generated content unless `--force` is explicit. Offline validation
requires a snapshot and fails visibly for missing symbols, type mismatches, or
generated-source drift. Live validation rejects cached snapshots and obtains
fresh symbols from every configured connection before applying the same
compatibility check.

The ADS parser preserves every selected live-client action and its explicit
target, target Net ID, connection, configuration, guarded-write, and JSON
options. When `--ams-port` is omitted, client discovery and Doctor select AMS
port 851. ADS server status and symbols preserve their endpoint,
authentication, and JSON choices without selecting another server action.

Discovery, browse, Doctor, guarded route addition, live symbol import, and live
validation require the `ads-wire` feature. A build without that feature fails
before synthesizing hardware results or writing generated files. Doctor write
probe options are all-or-none, accept only the documented scalar types, and do
not weaken the guarded write/read-back/restore policy. Route-add credentials
are accepted only through non-empty standard input and are never echoed in
reports or failures. Route-script and route-remove remain credential-free,
deterministic artifact generation.

ADS server status, symbols, and Doctor commands use the same versioned control
response rule as `trust-runtime ctl`: the response must contain Boolean
`ok = true` before either human or `--json` output is treated as success.
`ok = false` and a missing Boolean `ok` return non-zero and preserve the server
error text; choosing JSON output must never turn a rejected control request
into success. External Doctor proof metadata is accepted only when kind and
name are supplied together. Human and JSON modes render the same underlying
successful report or artifact.

###### ADS browser-to-control boundary

Every ADS HTTP route delegates to its exact registered control operation and
returns that operation's response; web routing alone does not manufacture ADS
success. Local web authentication may supply the configured internal control
token only to the in-process control dispatch that serves the authenticated
request. The generic control proxy must not forward that token in headers,
payload, logs, or error text to another target.

Credential-channel classification is derived from the observed request.
Loopback is `trusted_same_host`; a non-loopback request is
`trusted_https_admin` only when TLS is active and the authenticated role is
Admin; every other network request is `untrusted_plain_http_network`. The
server overwrites any caller-supplied channel before `ads.route_add` policy is
evaluated. Route-add responses and serialized failures never echo the supplied
username, password, or internal control token.

The runtime-host ADS setup page ships the `Beckhoff ADS Setup`,
`This runtime host`, `Open IDE Deploy`, `ADS Server`, and
`Expected AMS Net ID` onboarding labels, links to `/ide`, and exposes the
`/api/ads/status` and `/api/ads/server/status` routes from its local script.
It does not expose a runtime selector, a runtime target field, or a sample
`MAIN.Temperature` binding because the page configures the runtime host that
serves it.

Static setup-page asset checks establish only that this reviewed onboarding
surface and its route names are shipped. They do not prove rendered
interaction, authorization, control dispatch, ADS transport, or production
readiness.

###### ADS AMS/TCP frame codec

The ADS server's stream boundary uses the six-byte AMS/TCP prefix followed by
the AMS frame. The prefix contains two zero reserved bytes and a little-endian
`u32` length. That length and the configured `max_frame_bytes` limit cover the
32-byte AMS header plus payload and exclude the six-byte prefix.

A valid parsed frame preserves target and source AMS Net IDs and ports,
command ID, state flags, header data length, AMS error code, invoke ID, and
payload. Parsing rejects a prefix shorter than six bytes, nonzero reserved
bytes, a declared AMS length below 32, a declared length above the configured
cap, a frame shorter than its declared length, an unsupported command ID,
request state flags without the ADS command bit, or a header data length that
does not equal the payload length. The codec must reject those boundaries
before inventing missing bytes or accepting an inconsistent payload.

Serialization requires the header data length to equal the payload length and
emits the canonical prefix, header, and payload bytes. A response header swaps
the request source and target endpoints, preserves command and invoke
correlation, sets the response state, and records the supplied payload length.
This codec contract does not by itself prove socket lifecycle, authorization,
command dispatch, or external TwinCAT interoperability.

###### ADS server command dispatch and resource safety

The ADS server evaluates the client allowlist before decoding or executing a
command. A rejected client receives `AccessDenied`, emits one policy-rejection
audit event, and cannot allocate handles, register notifications, read runtime
values, or enqueue writes. Accepted commands require their exact ADS request
shape; truncated, extended, or internally inconsistent payloads return the
corresponding size/data error without guessing missing fields.

Direct, by-name, by-handle, system-symbol, upload, and sum-up reads preserve
the ADS group/offset distinction and the requested wire length. Symbol handles
and notification handles are non-zero and unique for the connection. Resource
limits fail with `NoMemory` or `InvalidSize` before inserting state; numeric
handle wrap searches for an unused value and must never replace a live handle.
Release or delete succeeds only for a live handle.

A direct or sum-up write reaches the runtime write port only for a resolved
symbol with Write capability and emits an accepted or rejected write audit.
Before a sum-up write or sum-up read/write performs its first item, the server
validates the complete aggregate header, every declared write-data range, the
exact aggregate length, the item limit, and the total write-byte limit. A
malformed later item therefore cannot leave an earlier runtime write queued.
After framing is valid, item-level address/access failures remain independent
and are returned in their documented per-item result slots.

Device notifications require a receiver endpoint, a supported cycle or
on-change mode, a resolvable symbol/system target, and a non-zero watch length
no larger than that target. Notification time values that are exact ADS
100-nanosecond multiples of one millisecond are normalized to milliseconds.
Deletion invalidates only the named live registration. Unsupported
`WriteControl` and client-sent `DeviceNotification` commands remain
side-effect-free and return `ServiceNotSupported`.

Notification sampling clamps the requested cycle to the configured minimum.
`SERVERCYCLE` emits every due sample even when its bytes are unchanged.
`SERVERONCHA` emits the first due sample, suppresses later equal bytes, and
emits again after a byte change. Symbol values, little-endian symbol-source
versions, and static system bytes are truncated to the registered watch
length. Static system-byte targets do not read runtime value storage.

A failed or undersized source read emits an invalidated sample that preserves
the notification handle and contains zero value bytes; it does not fabricate a
value. For representable inputs, sample timestamps use Windows FILETIME ticks:
`(unix_ms + 11_644_473_600_000) * 10_000`. Recovery after an invalidated
on-change sample, timestamp overflow, clock rollback, sampling at Unix time
zero, and notification-handle reuse remain explicit behavior gaps rather than
being inferred from the current focused tests.

Service-port routing is evaluated relative to the configured runtime, system,
router, and TCOM ports. An unknown target port returns AMS `AccessDenied` with
no payload. System service port 10000 accepts `ReadState`, responds from port
10000 with ADS state 5 and device state 0, and rejects symbol `Read` with
`ServiceNotSupported`. Router port 1 reports device-info version 3.1 named
`TCROUTER`; an unknown router metadata group/offset returns AMS transport
success carrying ADS `InvalidOffset` and zero data length.

A notification registration binds its receiver AMS Net ID and port when the
registration is accepted. Later requests multiplexed on the same TCP
connection cannot retarget that registration. The router TCP/IP metadata
entry-length rule remains unresolved: current behavior may return one complete
48-byte entry for a 40-byte request, while the general read contract preserves
requested wire length. No catalog claim is made for that conflict.

###### ADS server runtime publication and lifecycle

ADS server publication is a truST runtime product contract outside IEC
61131-3. A default server configuration is disabled and carries the public,
bounded defaults for ADS port, symbol/client/subscription/frame/sum-up/write
limits, string-byte limit, read/idle timeouts, and minimum notification cycle.
Those defaults are respectively ADS port 851, 256 symbols, 8 clients, 64
subscriptions per client, 256 total subscriptions, 65,536 frame bytes, 512
sum-up items, 8,192 write bytes, 4,096 string bytes, 5,000 ms read timeout,
60,000 ms idle timeout, and 50 ms minimum notification cycle. The default does
not acknowledge insecure transport, enable writes, permit unpinned clients or
public binds, or expose, write, or allow any configured entry.

Starting the enabled TCP service on an unavailable bind address returns the
server bind-error class and no usable server handle.

An empty `runtime.ads_server.expose` publishes no symbols. Otherwise exposure
and writable globs match either the canonical ADS name or the bare runtime
global name, and an invalid glob fails publication rather than broadening it.
Publication considers the complete eligible runtime-global set, orders it by
canonical ADS name, and only then applies `max_symbols`; storage insertion
order must not choose the bounded subset. The v1 supported scalar descriptors
are `BOOL`, signed and unsigned 8/16/32/64-bit integers, `REAL`, `LREAL`, and
`BYTE`/`WORD`/`DWORD`/`LWORD`. A symbol is always readable and becomes writable
only when both the global write gate and a writable glob authorize it.
Runtime values for which descriptor conversion returns no supported descriptor
are omitted without suppressing supported sibling symbols; this omission rule
does not select the unresolved `STRING` or array descriptor policy.
Namespace mapping is exact and reversible between `<namespace>.global.<name>`
(or `global.<name>` without a namespace) and its runtime global; a foreign
namespace is not stripped.

The server-owned symbol-table builder sorts accepted names, rejects duplicate
names, preserves declared capability flags, and assigns contiguous offsets
from the configured index group using each descriptor's byte size. The same
accepted symbol set therefore produces the same snapshot regardless of input
order. Deterministic snapshot serialization is stable for an unchanged
snapshot. The local symbol-version counter establishes version 1 on first
observation, retains it while the complete serialized snapshot is equal, and
advances when that serialized snapshot changes. This counter contract does not
prove atomic live publication.

Each value read obtains a runtime snapshot from the configured provider. No
snapshot returns ADS `NotReady`; a foreign ADS name or absent runtime global
returns `NotFound`; an unencodable value returns `InvalidData`; and a byte-size
mismatch returns `InvalidSize`. Those failures return no fabricated value
bytes. The clock authority and threshold for classifying an available snapshot
as `Good` or `Stale` remain unresolved under
`SPEC_GAP_P18_ADS_SERVER_SNAPSHOT_FRESHNESS_CLOCK_001`; existing freshness
observations are not specification authority.

A disabled lifecycle returns no server without consulting the snapshot
provider or binding sockets. An enabled lifecycle requires a concrete bind IP
and AMS identity, initializes its symbol source from the available runtime
snapshot or an empty not-ready snapshot, and owns both the ADS TCP listener and
UDP identify responder. Runtime status accessors report the live addresses,
policy, client count, and served symbol table. Refresh retains the bound socket
ownership, while explicit shutdown and drop stop both listeners. The atomic
publication and version transition for a refreshed symbol table remains
unresolved under `SPEC_GAP_P18_ADS_SERVER_SYMBOL_VERSION_ATOMICITY_001` and is
not test authority in this contract.

The capacity and descriptor policy for runtime `STRING` values remains
unresolved under `SPEC_GAP_P18_ADS_SERVER_STRING_DESCRIPTOR_POLICY_001`; the
public string-byte configuration bound alone does not select that behavior.

###### ADS server client policy, write-back, and audit

An ADS server client is authorized only when its AMS Net ID matches an
allowlist entry and its observed source matches that entry's parsed exact IP or
valid IPv4/IPv6 CIDR. Equivalent textual spellings of the same IP address are
equal; invalid IP or CIDR text and address-family mismatch fail closed. An
`Unpinned` entry remains AMS-Net-ID-only and is effective only while
`allow_unpinned_clients` is explicitly true, including when configuration is
constructed programmatically; when opted in, it does not require an observed
source. Missing source identity denies pinned `Ip` and `Cidr` entries, and
poisoned policy state denies all entries. A temporary permit authorizes only
its exact AMS identity and parsed source IP and is removed when its guard
drops.

Denied attempts retain bounded operator evidence. Repeated attempts with the
same AMS identity, source, and reason coalesce by increasing the count and last
seen time; distinct attempts retain at most 32 records and evict the oldest.
Reasons distinguish an unknown AMS identity, a missing source, and a source
outside the configured pin, and the suggested entry preserves the observed
identity without authorizing it.

Runtime write-back checks, in order, the live client policy, the global write
gate, combined exact-namespace and writable-glob authorization, runtime
readiness, and ADS value decoding before it queues a global write. A foreign
namespace or a symbol outside the writable allowlist returns `AccessDenied`
before runtime readiness is inspected; a stopped runtime cannot change that
security-first result to `NotReady`. `Ready`, `Running`, and `Paused` resources
may accept an otherwise valid write. `Boot`, `Faulted`, and `Stopped`
resources, or any resource carrying a last error, return `NotReady` only after
the earlier authorization checks pass. Malformed ADS value bytes return
`InvalidData`. A malformed writable glob rejects write-port construction with
`InvalidParameter`. Every rejection is free of a queued PLC mutation. Accepted
writes use the runtime debug queue's normal same-target coalescing behavior.

The audit adapter emits one runtime control-audit record per accepted write,
rejected write, or policy rejection when a sink is configured. It preserves
the event timestamp, client AMS identity and observed source, symbol and value
type when present, stable `ads.server.write` or `ads.server.policy` request
type, stable kind, success state, and ADS error code/message. Audit identifiers
are process-local unique records; an absent or disconnected audit receiver must
not change ADS command behavior.

###### ADS diagnostic report and production-readiness truth

ADS Doctor reports use diagnostics schema version 2 and retain the documented
snake-case or kebab-case wire names for roles, vantages, transports, steps,
statuses, skip reasons, actions, connection states, and readiness reasons.
Evidence and action maps serialize in deterministic key order. The report and
route-plan schemas contain no credential, username, password, token, or other
secret-bearing field; remediation may identify a credential channel but must
not echo its contents.

A Doctor report is `fail` when it has no steps or any production-blocking step
is not `pass`. Otherwise any warning, skip, or explicitly non-blocking failed
step makes the report `partial`; only a non-empty set of passing steps makes it
`pass`. A report is never production-ready without both an overall `pass` and
role-appropriate production evidence. Client evidence binds the runtime and
target identities. Server evidence binds the runtime identity and allowed
clients, and additionally requires independent external-client proof marked as
verified with kind `twincat` (ASCII-case-insensitive), a non-empty client name,
and a client timestamp. Loopback self-test or pyads evidence remains useful
diagnostic proof but cannot establish TwinCAT production readiness.

Production evidence hashes the canonical runtime identity, target identity or
allowed-client set, ADS configuration, symbol snapshot set, optional generated
ST, deployed ADS configuration, and the live ADS status used by the Doctor.
Symbol-snapshot hashing is independent of connection ordering. A later
readiness evaluation is `not_ready` when Doctor evidence is absent. With
evidence present it is `needs_recheck` when live runtime status is absent, the
Doctor did not bind a live-status hash, deployed configuration evidence is
missing or mismatched, the current status hash changed, runtime ADS is not
healthy, the runtime clock was reported unreliable, or the evidence is older
than its declared freshness bound. The effective expiry is the earlier of an
explicit expiry and `doctor_timestamp_ms + stale_after_ms`; arithmetic overflow
must not extend evidence lifetime. Only evidence with none of those conditions
is `ready`.

###### ADS onboarding identity authority

ADS onboarding identity is a truST product contract outside IEC 61131-3. The
selected runtime-host source address and the selected target address must each
be a syntactically valid IP address before they can become route, Doctor, or
symbol-import authority. An omitted local AMS Net ID is derived only from a
canonical IPv4 source as `<ipv4>.1.1`; IPv6 requires an explicit override. An
explicit override, including an explicitly blank value, must contain exactly
six decimal octets in the range 0 through 255 and is stored in canonical
decimal form. A malformed source address, target address, or override is
rejected before an onboarding identity is returned and before any ADS wire or
route side effect. Public and NAT-suspect identities remain ineligible for
automatic route creation.

###### ADS onboarding discovery endpoint identity

ADS onboarding discovery accepts a manual target, directed UDP identify, and
optional directed-broadcast results. Manual identity does not depend on UDP
when an AMS Net ID is supplied; without one, directed identify supplies the
target identity. A failed broadcast candidate does not erase successful manual
or other broadcast results.

Discovery uniqueness is the ADS endpoint identity `(AMS Net ID, AMS port)`,
not the host IP address alone. Multiple PLC runtimes may share one host IP and
must remain separate results when their AMS Net ID or AMS port differs.
Repeated observations of the same ADS endpoint are reported once even if its
IP or discovery provenance differs. Directed-broadcast targets are derived
only from usable non-loopback, non-link-local IPv4 interface candidates and
are themselves deduplicated.

###### ADS onboarding live-wire and guarded-write recovery

The `ads-wire` onboarding boundary performs real UDP identify, TCP 48898,
route-back, AMS target, state, symbol, handle, sum-up, notification, guarded
write, and route-add operations without exposing raw ADS types in the public
Doctor schema. UDP identify collection ignores malformed replies but retains
every distinct `(AMS Net ID, AMS port)` endpoint even when multiple runtimes
share one host IP. Repeated observations of that same endpoint are emitted
once, and a receive window with no valid reply fails explicitly.

The live ADS transport cache is scoped to target IP, target AMS Net ID, target
AMS port, and selected local AMS Net ID. Changing any component discards the
transport plus cached symbols and handles before reconnecting. Symbol upload
replaces the name cache; handle resolution may refresh that cache but must fail
for an absent symbol. Sum-up reads reject unknown local handles, non-good
quality, missing values, unknown returned point names, and value/type encoding
failure instead of manufacturing bytes. Notification setup resolves the named
symbol and subscribes in on-change mode.

A guarded write probe first requires a writable remote symbol whose complete
type descriptor matches the requested descriptor, resolves its handle, and
reads the original value. Once the probe write has been attempted, every write
error, read-back error, or read-back mismatch must attempt to write and verify
the original value before returning failure. A restore failure is retained in
the returned diagnostic alongside the initiating failure. A successful probe
is complete only after both probe read-back and restored-value read-back match.
Automatic route addition continues to use the already validated runtime-host
AMS identity and one-shot credentials; the transport never enables implicit
route creation.

###### ADS client configuration and activation

ADS client configuration and activation is a truST product contract outside
IEC 61131-3. Configuration parsing must fail closed before worker creation or
any ADS wire activity unless all connection names are unique; every connection
name and host is nonblank; every target AMS Net ID and explicit local AMS Net
ID contains exactly six canonical decimal octets; every target AMS port is
non-zero; and every connection declares at least one point. Plain transport
requires explicit insecure-transport acknowledgement, and that acknowledgement
is invalid for any non-plain transport.

Across the complete configuration, every declared global variable binding is
unique. Each point supplies exactly one complete address form: either one
nonblank symbol or the complete index-group, index-offset, and non-zero size
tuple. Notification options are valid only for notify-mode points, and every
point's IEC type, STRING capacity, array dimensions, address size, access, and
update-mode metadata must be internally valid. Production-readiness activation
additionally requires every configured local AMS Net ID to equal the selected
runtime-host identity; a missing or mismatched local identity is not
production-ready.

Secure transport remains an unresolved product decision: the configuration
schema currently recognizes a reserved `secure` value while the implemented
ADS transport rejects secure activation. This specification does not select a
new parser or transport behavior for that contradiction, and the contradiction
is not test authority until a separate product decision resolves it.

###### ADS core value codec and descriptor size

The shared ADS value codec is a truST product contract outside IEC 61131-3.
For the reviewed fixed-width scalar matrix, `BOOL` uses one byte with reviewed
true encoded as `1`; signed and unsigned integers and bit strings use their
declared one-, two-, four-, or eight-byte width; and `REAL` and `LREAL` use
four and eight bytes respectively. Multi-byte values use canonical
little-endian byte order. The reviewed matrix proves exact encode/decode
round trips for its named representative values; it does not select behavior
for noncanonical `BOOL` bytes, every elementary value, or non-finite egress.
Non-finite ingress remains governed by the ADS client configuration and
activation contract.

For the reviewed `STRING(8)` partition, the declared capacity is eight payload
bytes and the fixed layout is nine bytes including the terminator. Decoding the
reviewed zero-padded buffer returns bytes through the first NUL, and encoding
`Pump` produces the exact trailing-zero-filled nine-byte buffer. This partition
does not select Unicode capacity, invalid UTF-8, an absent terminator,
over-capacity values, or a missing descriptor length.

For the reviewed one-dimensional `INT[1..3]` partition, decoding and encoding
preserve the inclusive bounds, element order, scalar representation, and exact
total byte extent. Multidimensional order, invalid or overflowing bounds,
element-count overflow, and array element-type failures remain outside this
contract.

The reviewed mismatch boundary rejects an input byte slice whose length
differs from the descriptor as `ByteLengthMismatch`, a scalar value supplied
for an array descriptor as `ValueTypeMismatch`, and an array whose bounds
differ from the descriptor as `ArrayShapeMismatch`. These observations do not
establish a complete precedence order among all possible mapping failures.

ADS symbol metadata is admissible at the reviewed descriptor-size boundary
only when the endpoint `byte_size` equals the descriptor-computed extent. The
reviewed `WORD[1..4]` extent is eight bytes and the reviewed scalar `REAL`
extent is four bytes. A `REAL` endpoint size of eight rejects as
`ByteSizeMismatch` with expected `4` and actual `8`. Descriptor-size overflow
and every nested descriptor failure remain explicit gaps.

This shared codec authority does not select the unresolved runtime ADS server
publication policy for `STRING` or arrays. It also makes no wire,
external-endpoint, or TwinCAT interoperability claim.

###### ADS core point-quality lifecycle

The shared ADS point-quality record is a truST product contract outside IEC
61131-3. A reviewed cold-start status preserves its supplied point name and
starts in `Stale`; the cold-start partition does not establish the detail text
or a timestamp. An explicitly constructed reviewed stale record may preserve
the supplied last-good timestamp and detail.

For the reviewed in-memory transition sequence, an initial stale record has no
last-update timestamp. Marking it good sets `Good`, records the supplied update
timestamp, and clears stale detail. Marking that record failed sets `Error`,
replaces the timestamp with the failure timestamp, and records the failure
detail. Marking that error record stale sets `Stale`, preserves the last known
timestamp, and replaces the detail with the current stale reason.

These constructor and mutation rules do not select the clock source, timestamp
units beyond the record's declared millisecond field, monotonicity, concurrent
mutation, serde compatibility, worker publication timing, transport behavior,
or external endpoint truth. The runtime ADS worker remains responsible for
deciding when a transport or lifecycle event invokes these transitions.

###### ADS client transport boundary

The ADS client transport is a truST product contract outside IEC 61131-3. It
must validate the required target AMS Net ID and an optional local AMS Net ID
as exactly six decimal octets in canonical form, and validate the non-zero
target AMS port, before opening TCP 48898 or performing any other wire
operation. The runtime transport supports plain ADS only and must reject
reserved secure transport or implicit route creation before the wire; route
authoring remains an explicit onboarding operation outside this transport.

After connection authority validates and before replacing a client, the
transport attempts best-effort remote release of active notifications and
symbol handles. Remote release failure does not make remote cleanup
deterministic; the deterministic guarantee is that replacement, disconnect,
and drop tear down the transport's local handle maps, notification receiver,
and client ownership. ADS device state maps `Run` to runtime run, `Stop`,
`Idle`, and `Config` to runtime stop, `Error` and `Exception` to runtime fault,
and every other state to runtime unknown.

Symbol upload projects only supported scalar, array, and STRING metadata into
the runtime descriptor model. Every projected symbol is readable; the ADS
read-only flag withholds write access, and the persistent flag also establishes
the runtime retain guardrail. Array dimensions, STRING capacity, and byte size
must agree with the projected type. Unsupported metadata may be omitted, but
inconsistent, overflowing, or otherwise unrepresentable metadata fails closed
instead of producing a bindable descriptor.

A symbol-upload error establishes a missing return route when its typed kind is
`RouteMissing`, or when a `NoSymbols` failure explicitly describes a timeout,
no reply/response, route setup, or failure while receiving the reply. An actual
empty symbol-table response and a refused TCP connection are not reclassified
as route missing. This diagnostic classifier selects recovery guidance only; it
does not claim that adding a route will make the subsequent upload succeed.

By-name handle resolution first verifies that the remote symbol byte size
exactly equals the requested type size and then acquires a remote symbol
handle. Index-group/index-offset resolution verifies that its declared byte
size exactly equals the requested type size and does not manufacture a remote
handle. Replacing or removing a cached by-name handle releases the old remote
handle.

Sum-up reads and writes preserve input order and cardinality. Each item retains
its own conversion or ADS error as non-good quality; failed reads have no value,
and failed writes are never reported as successful. The transport must not
shift results between points, discard failed positions, or manufacture values
to fill missing or malformed results. Subscriptions accept notify-mode points
only, preserve the requested on-change or cyclic notification mode, and reject
poll-mode points before subscription. Unknown notification handles and samples
whose bytes do not decode as the resolved type fail closed with no manufactured
value. The ADS symbol-version byte is an opaque equality token: a changed token
invalidates symbol-derived state, but ordering, monotonicity, and wraparound
must never be inferred from its numeric representation.

The connection worker constructs handle and subscription correlation as an
unpublished candidate. Handle resolution must return exactly one response for
every requested point, and every response must preserve that point's configured
address and complete type descriptor. Response order is not authority, and the
numeric handle value is opaque: the same numeric value used by different points
is not rejected merely because of its representation, while this contract does
not select a zero-handle policy for by-name resolution. Missing, extra,
duplicate-point, wrong-address, or wrong-descriptor responses reject the
candidate. Notify subscriptions likewise return the exact requested point, and
every active subscription ID is unique within the connection; this contract
does not select a zero-ID policy. The worker publishes the complete
handle/subscription maps only after all candidate operations succeed.

A poll batch must have the same cardinality as its requested handle batch and
must preserve point identity at every position before any per-result cache
mutation. A structural mismatch rejects the complete poll response; a later
uniform connection fault projection may update all qualities, but no prefix of
the malformed response becomes accepted data. Each notification sample is
validated independently against both the configured notify point and that
point's currently active subscription ID before that sample mutates the cache.
Unknown points, inactive IDs, and cross-point correlations fail closed. Repeated
valid samples for one active point remain valid and are applied in delivery
order; a later invalid sample does not roll back the raw values stored by
earlier correctly correlated samples, but its connection-level validation
failure may revoke their `Good` authority. A structurally valid, correctly
correlated point-level non-good or non-finite result remains a per-point quality
result and does not invalidate other points in the batch.

Symbol-derived candidates are valid only when the opaque symbol-version token
is equal before and after upload, handle resolution, and subscription setup.
When the active token changes, the worker immediately makes its old handles,
subscriptions, token, and candidate state unavailable and revokes `Good`
authority from cached inputs before attempting refresh. Raw cached values may
remain available for diagnostics, but `apply_inputs` cannot apply them without
current `Good` quality. Revoking readable authority changes only currently
`Good` quality and cannot overwrite a read/write point's newer `ADS write
pending` or explicit non-finite output error. Numeric ordering and monotonicity
are never inferred, including across wraparound. A reconnect request or a
validation failure after a successful transport connect performs best-effort
disconnect, clears local
handles, subscriptions, symbol token, and symbol-check scheduling, and marks
affected input authority stale for transport recovery or error for validation
failure. Replacement state becomes visible atomically only after the complete
candidate passes correlation and token-stability validation. This contract does
not select whether pending outputs are replayed, preserved, or dropped across
reconnect.

Local output intents are generation-tagged. Capturing a finite changed output
atomically replaces the point's queued value with the latest value and reports
`ADS write pending`. A write completion may acknowledge or update quality only
when it still names the generation that was transmitted; completion for an
older value cannot remove a newer queued value or overwrite its pending/error
quality. The worker validates write-response cardinality before any per-result
acknowledgement or quality commit. A malformed response therefore performs no
partial per-result commit and leaves every transmitted generation pending. A
correlated success atomically acknowledges that generation and reports good
quality; a correlated failure terminally removes that exact generation and
reports explicit `ADS write failed: ...` quality without falsely acknowledging
success. A later changed finite output creates a new generation normally.
Transport-error and malformed-cardinality connection
projections compare the current complete write-generation state with the
complete snapshot captured atomically with the pending-write batch before
request construction. They must not overwrite a finite pending quality or
non-finite rejection created while the transport call is in flight, including a
change to an output point that was not in the transmitted request; other
input/read authority still becomes stale or error with the connection.

Worker shutdown and drop must wake an idle interval wait promptly, request
best-effort disconnect, publish `Disconnected`, revoke any remaining `Good`
input authority, and join the worker thread. Raw input values may remain for
diagnostics, and read/write points retain newer non-good pending or explicit
output-error quality. This local wakeup guarantee does not claim cancellation
of a transport call already in flight. Binding identity, declared runtime type,
remote descriptor, access, retain opt-in, and index byte-size validation remain
separate fail-closed partitions; success in one partition cannot substitute for
another.

The lifecycle/security invariant is that every connection attempt validates
all wire authority first and performs no implicit route mutation; after
successful validation, replacement, disconnect, and drop attempt best-effort
remote release and deterministically tear down transport-owned local resources.
The data/value invariant is that supported metadata, handles, sum-up positions,
qualities, notifications, and symbol-version equality preserve the remote
meaning exactly, while inconsistent or undecodable data fails closed without a
fabricated descriptor, success, or value.

###### ADS server Doctor orchestration and loopback proof

ADS server Doctor orchestration and loopback proof are truST runtime product
contracts outside IEC 61131-3. The Doctor reports only live,
endpoint-correlated proof. A supplied status snapshot cannot make symbol
service healthy when the running ADS server's live symbol table is empty; the
reported point count and readiness state must reflect the live table used to
serve clients.

Independent external-client evidence is verified only when its client kind and
name both contain non-whitespace text and its timestamp is present. Missing or
blank proof content must not produce a passing external-client step or set the
production-evidence `external_client_verified` flag. Existing production
readiness rules remain unchanged: loopback and pyads evidence are diagnostic,
while production readiness still requires verified TwinCAT evidence.

The loopback client rejects zero symbol and notification handles. A matching
notification handle proves delivery only when the sample contains the complete
payload requested for that symbol; a truncated matching prefix cannot pass.
Every solicited loopback reply must be an ADS response frame whose source and
target AMS Net IDs and ports are the exact reverse of the request and whose
command and invoke ID match the request before its payload is trusted.

Every passing server Doctor step and every verified evidence field is derived
from the running server's live
state or from complete, nonblank, endpoint-correlated external/loopback proof;
missing, zero, truncated, request-direction, or endpoint-mismatched input fails
closed without manufacturing health, verification, or success.

###### ADS Doctor orchestration and active-device safety

The client ADS Doctor emits exactly the ordered required-step set owned by the
runtime engine. A production-blocking failure prevents every later direct
probe; cancellation marks the first not-yet-started step as cancelled and the
remaining steps as blocked. Guarded writes are skipped without blocking when
writes were not explicitly enabled, and enabling writes without a complete
probe is a blocking configuration failure. Production evidence is attached
only after every required step passes, from a runtime-host vantage, with live
deployed status available.

Manual Doctor targets must contain a syntactically valid IP address, a non-zero
AMS port, and, when supplied, a canonical six-octet AMS Net ID. Those fields
are rejected before any ADS wire operation. An explicitly selected symbol must
contain non-whitespace text and must occur in the uploaded symbol table; it is
never accepted merely because a later transport happens to resolve it.

An active-device shortcut is valid only when its target IP and AMS port, plus
the expected AMS Net ID when supplied, identify the requested Doctor endpoint.
A mismatched snapshot fails closed without opening a second ADS connection.
For a configured route whose host is an IP literal, runtime active-device
lookup therefore requires that host IP and AMS port to equal the requested
target and, when the request supplies an expected target AMS Net ID, requires
that identity to equal the route target AMS Net ID. A route name, target
display name, or one matching endpoint component cannot substitute for the
complete required match. When both the configured route and request carry a
selected local AMS identity, those identities must also be equal. Hostname
resolution, omitted-local-identity semantics, and selection among multiple
exact matches remain unresolved product decisions and establish no test
authority here.
Read-only active-device reports derive state only from the supplied live
snapshot: a batch-read step passes only when at least one point has good quality
with a recorded update timestamp; no points, no timestamped good point, or any
degraded point produces a warning. The full Doctor for an overlapping active
device remains blocked until the caller explicitly pauses it.

###### ADS onboarding route artifact and credential safety

Automatic ADS route addition is available only over a credential-permitted
channel and never for a public or NAT-suspect runtime-host identity. Both the
one-shot username and password must contain non-whitespace text before the wire
operation is called. Credentials are neither serializable nor present in route
plans, generated artifacts, reports, logs, or error detail.

Route-plan and route-removal artifacts are deterministic and credential-free.
XML content escapes every user-controlled field. Generated PowerShell treats
that XML only as data: it transports the UTF-8 XML payload through a non-
executable encoding and decodes it before assigning `DocumentFragment.InnerXml`;
user-controlled text must never be able to terminate a PowerShell here-string
or introduce a command. PowerShell single-quoted values escape embedded quote
characters, and generated filenames use a filesystem-safe slug. Apply and
remove scripts back up each selected `StaticRoutes.xml`, match a route by its
exact name, preserve unrelated routes, and report an encoding/BOM change.
Client and server roles retain their distinct operator instructions.

###### ADS symbol-import artifact safety

The server-control symbol-import apply operation writes only inside its selected
project root. Explicit `ads.toml`, snapshot, and generated-ST destinations must
be non-empty relative paths without parent-directory, root, or platform-prefix
components. The default snapshot destination is available only when the
connection name is a safe single filename component; a connection name must
never become path syntax. The three resolved artifact destinations must be
distinct. A snapshot destination must be a direct `*.symbols.json` child of
`ads/snapshots`, because that canonical directory is the authority reloaded by
subsequent imports; accepting an undiscoverable snapshot location is forbidden.

Before writing any import artifact, server control checks an existing generated
ST destination. Byte-identical next content is safe to retain. Different next
content may replace the file only when the existing file still validates
byte-for-byte against the current `ads.toml` and every canonical cached
snapshot; this permits a legitimate follow-up import while protecting a
drifted or operator-edited file. Missing, malformed, or inconsistent current
authority fails closed. A rejected path or overwrite preflight leaves all three
artifacts unchanged.
These confinement and overwrite rules apply to the server-control authoring
surface; the explicit filesystem paths of the local CLI remain operator-owned.

###### ADS symbol-import selection and generated identifiers

ADS symbol selection is deterministic. A non-empty exact-symbol list has
exclusive precedence over include patterns. Otherwise include patterns are
ASCII-case-insensitive; `*` matches any sequence, anchored prefixes and suffixes
must be honored across the complete symbol, and a pattern without `*` performs
the documented substring search. An explicitly empty pattern matches nothing
rather than broadening the import to every uploaded symbol. With neither exact
symbols nor include patterns, all candidates remain selected by default.

Generated local variable names normalize symbol text to lowercase letters,
digits, and single embedded underscores, receive a stable suffix on collision,
and must lex as one Structured Text identifier rather than a keyword or other
token. A normalized keyword receives the `ads_` prefix. This follows IEC
61131-3:2013 6.1.2 and Table 2 for identifier form and 6.1.3 for the
case-insensitive prohibition on using keywords as variable names.

Apply remains read-only unless write acknowledgement is explicit. With that
acknowledgement, a remote symbol carrying both Read and Write capabilities maps
to `read_write`, while a Write-only symbol maps to `write`; acknowledgement
must not manufacture a remote Read capability that the symbol does not expose.

A cached snapshot is authoritative only for its exact selected connection; a
different connection name is rejected before candidate generation or mutation.
Cached candidate and group ordering is deterministic. Every candidate reports
its proposed local name, access and polling disposition, and selected state
without changing the remote symbol's capabilities. A live import is never
replaced by cached or synthetic results when wire support is unavailable.

###### ADS symbol-import apply and merge

Core apply requires at least one selected candidate and parses an existing
`ads.toml` strictly before producing replacement artifacts. It replaces only
the imported connection with the same exact name, preserves and sorts other
connections, pins the selected runtime-host AMS identity, canonicalizes the
merged snapshot set, and generates one deterministic ST interface. Local point
names and their generated quality names are unique across every connection.

Imported ADS scalar descriptors project to the corresponding supported IEC
BOOL, signed and unsigned integer, REAL/LREAL, bit-string, and STRING type
names defined by [Data Types](02-data-types.md). STRING capacity and every
inclusive array lower/upper bound are retained. Poll and notification modes and
read, write, or read-write access are serialized without silently changing the
selected capability. Malformed current configuration, missing selection,
unacknowledged writes, invalid descriptor metadata, or generation mismatch
fails before an artifact is returned.

The generated ADS interface is one deterministic Structured Text source for
all configured connections. It declares the shared `ADS_QUALITY` enum exactly
once, then each configured point and its `<name>_quality` companion in stable
connection/point order. Generated identifiers must be valid non-keyword
Structured Text identifiers and unique across point names and generated
quality names. Snapshot and configuration descriptor byte sizes must agree
before generation. STRING capacity and inclusive array bounds are rendered
without widening or flattening their declared type.

Offline validation regenerates the complete expected source from the current
configuration and canonical snapshots. It accepts only byte-identical source
and reports the first differing line for stale content. A generated source that
successfully compiles with its consuming project proves language/toolchain
compatibility for that exact artifact; generation or source equality alone does
not prove compilation, runtime binding, or live PLC communication.

Once that generated source is compiled into a runtime, its point and quality
declarations are ordinary first-class globals. Their declared type and enum
variant remain available through HMI schema/values, the HMI binding catalog,
control evaluation, debugger globals, historian capture, and the supported
OPC UA scalar/enum projection. This cross-surface rule requires runtime metadata
and snapshot evidence; source-text presence alone is not proof.

##### 6.8.1b Runtime benchmark measurement and output contract

`trust-runtime bench` validates each workload before measurement: sample,
payload, and dispatch-fanout counts are non-zero; project paths exist; and
synthetic loss and reorder rates are within the closed interval from zero to
one. Requested warmup cycles are executed before measured project cycles and
are excluded from latency samples.

For T0, mesh, and dispatch workloads, `--payload-bytes` is the exact payload
length exercised by every applicable measured path. Implementations must not
silently cap large requested payloads. Mesh pub/sub and query/reply both use
the requested length; dispatch embeds the requested length in its mapped
control payload. Their reports expose the effective payload length so evidence
cannot claim one workload size while measuring another.

Latency summaries use sorted nanosecond samples and nearest-rank p50, p95, and
p99 selection. Histogram classification rounds each sample up to a whole
microsecond and retains an overflow bucket. Jitter contains the absolute delta
between consecutive observations, so fewer than two observations produce no
invented jitter sample. Synthetic loss and reorder decisions use a fixed seed
for reproducible workload selection.

JSON output serializes the benchmark kind and its typed report; table output
renders the same contract fields. Project reports separate measured cycles,
configured cycle budget and overruns, throughput, requested watched globals,
and optional VM/Tier-1 profile data. A missing watched global is explicit JSON
`null`, not an omitted or fabricated value.

Every benchmark subcommand defaults to table output when `--output` is omitted;
an explicit `--output json` selects JSON without changing the selected workload
parameters.

##### PLCopen CLI surface contract

`trust-runtime plcopen export` accepts an optional project, optional output
path, optional export target, and JSON-output selection. An omitted export
target selects `generic`. The canonical targets are `generic`, `ab`, `siemens`,
and `schneider`; the documented Allen-Bradley/Rockwell, Siemens TIA, and
Schneider EcoStruxure aliases map to their corresponding canonical targets.

`trust-runtime plcopen import` requires an input XML path and accepts an
optional project and JSON-output selection. Parsing preserves supplied paths
and choices without performing import or export work.

##### 6.8.1c Runtime project-check contract

`trust-runtime check` validates a project without writing `program.stbc` or
another compiled bytecode artifact. An explicit `--project` selects the
project root; otherwise the command uses bundle auto-detection and falls back
to the current directory. A relative `--sources` override is resolved from the
project root, while the default source root is `<project>/src`.

The command validates required `runtime.toml` and `io.toml` files plus optional
`ads.toml` and `opcua_client.toml` files, inspects the complete source and local
dependency layout, and compiles valid layouts in memory. Independent
configuration and compile findings are accumulated in one response. A
configuration failure does not suppress an otherwise possible in-memory
compile, and a compile failure does not discard the source and dependency
identity already obtained from successful layout inspection.

`--json` and `--ci` emit response version 1. The response identifies the
command and project; exposes `ok`/`status`, error and warning counts, stable
issue codes and optional file locations; and reports the exact source count,
source paths, dependency roots, resolved dependency names, and optional
in-memory bytecode size. `source_count` equals the number of reported source
paths. `ok` is true exactly when the error count is zero. Human output is
derived from the same response and reports either the successful source list
or every accumulated issue.

Success exits 0. Any configuration or source-layout error exits with the
invalid-configuration class 10, including a response that also contains a
compile error. A compile-only failure exits with the build-failure class 11.
Both successful and failed checks leave an existing `program.stbc` untouched
and never create one.

##### 6.8.1d Runtime build and CI failure contract

`trust-runtime build` compiles the selected project and writes
`<project>/program.stbc`. An explicit `--project` selects the project root;
otherwise ordinary bundle discovery is used and the current directory is the
final fallback. The default source root is `<project>/src`. A relative
`--sources` override is resolved from the project root, including when the
command is launched from another working directory. Local package dependencies
are resolved and compiled with the selected project sources.

Compilation completes before `program.stbc` is opened for writing. A source,
dependency, or compile failure therefore neither creates the artifact nor
replaces bytes from an earlier successful build. A successful build writes the
new bytecode and reports the exact collected source paths, including dependency
sources.

`--ci` emits response version 1 with command `build`, status `ok`, the selected
project and output paths, `source_count`, the complete source list, dependency
roots, and resolved dependency names. `source_count` equals the number of
reported source paths. Human mode reports the written path and source count,
shows at most the first five collected paths, and reports the number omitted.

CI failures use stable process classes: invalid project or configuration input
is 10, build or compile failure is 11, test assertion or runtime-test failure is
12, timeout is 13, and an unclassified internal failure is 20. A recognized
failure class is preserved regardless of command context. Timeout takes
precedence when a message contains multiple recognized classes; otherwise test,
build, and configuration classes are considered in that order. Only an
unclassified message uses the parsed command as a fallback: `build` and `check`
map to 11, `test` to 12, `validate` to 10, and other or absent commands to 20.
This CLI and process-exit policy is a truST product contract rather than IEC
61131-3 language behavior.

##### 6.8.1e Standalone Web IDE contract

The built-in standalone Web IDE is a local authoring surface over the selected
project root. Its shell is served from bundled, content-hashed local assets and
does not require a CDN. Project selection is explicit: opening another approved
project updates the active root, while a request outside the approved base is
rejected before filesystem access.

An IDE session has a stable Viewer or Editor role. Viewer sessions are
read-only; Editor sessions may mutate project files. Successful activity renews
the sliding idle deadline, an idle session expires, and reaching the fixed
session limit evicts the oldest inactive session rather than an active,
recently renewed one. Pairing and session tokens are required by the endpoint
contract and are never accepted as workspace paths or file contents.

All source paths are relative to the active project root. Absolute paths,
parent traversal, escaping symbolic resolution, malformed include/exclude
globs, and oversized JSON or file content reject before mutation. File writes
use optimistic version authority: the caller supplies the expected version, a
successful write increments it, and a stale expected version returns a conflict
with the current version. Create, rename, and delete preserve project
containment, report collisions, and append the successful mutation to the
filesystem audit log. Tree, file-list, and workspace-search results use stable
relative paths and honor validated include/exclude globs.

Diagnostics, hover, completion, and formatting use the same Structured Text
analysis and formatting contracts as the language service. The format endpoint
returns the affected path, changed flag, and complete formatted content.
Build, test, and validate endpoints start an asynchronous job, return its job
ID, and retain terminal output plus parsed source locations. Health reports
active sessions, tracked documents, and accumulated frontend telemetry.
Frontend telemetry updates the same health snapshot; collaborative-presence
functionality that is not implemented is reported explicitly as out of scope,
not as an active collaboration session.

Workspace path normalization trims outer whitespace, removes `.` components,
uses `/` in returned paths, and rejects an empty non-root path, absolute or
rooted input, every `..` component, and every component beginning with `.`.
Source paths additionally end in `.st` case-insensitively. File and tree
enumeration omit hidden entries, return project-relative `/`-separated paths,
and sort tree siblings by name. Source enumeration includes only `.st` files
and does not follow a path outside the selected root.

Reads decode UTF-8 and admit a file whose exact byte length equals the
configured limit; one byte over returns `too_large`. Fingerprints contain exact
byte length and the filesystem modification time in Unix-epoch milliseconds.
Glob inputs are trimmed; absent or blank input disables that filter, malformed
syntax returns `invalid_input`, and a valid pattern is preserved as the
matching authority.

IDE positions count Unicode scalar values within zero-based lines. A position
beyond its line clamps to that line's byte end and a line beyond the document
clamps to document end. Compiler text offsets converted back to IDE positions
must be valid UTF-8 boundaries. Rename edits are applied from greatest start
offset to least so earlier offsets remain stable. Every range is ordered,
in-bounds, on UTF-8 boundaries, and non-overlapping; invalid edit sets reject
without returning partially edited text.

Completion prefix extraction considers ASCII letters, digits, and `_` in the
current line before the cursor. In-scope declaration fallback recognizes valid
ASCII identifiers from POU/type/class declarations and variable declaration
left-hand sides, ignores comment-only lines, ranks prefix-matching symbols
before keywords and non-prefix matches, deduplicates labels
ASCII-case-insensitively, then applies the requested result limit.

The formatter is deterministic, uses two spaces per nesting level, removes
trailing horizontal whitespace, preserves blank lines and comment text, and
returns one terminal newline for every nonempty input. POU, declaration,
conditional, loop, case, repeat, action, transition, method, property, class,
interface, resource, and configuration openers indent their bodies. Their
corresponding `END_*`, `ELSE`, `ELSIF`, and `UNTIL` lines dedent before
rendering; branch continuations then indent their following body.

Session removal is atomic across session authority, frontend telemetry,
analysis cache, and every document's open-handle set. Expiry removes entries
whose deadline is less than or equal to the current second and leaves later
deadlines intact. Generated session tokens are URL-safe unpadded encodings of
32 random bytes.

The standalone IDE resource contract is:

- one source apply must remain below 250 ms, and the repeated local in-process
  average below 40 ms;
- the HTTP apply path must remain below 250 ms maximum and 50 ms average in the
  focused reference fixture;
- reference p95 bounds are 2.5 s for boot-to-ready, 150 ms for completion,
  150 ms for hover, 300 ms for diagnostics, and 400 ms for workspace search;
- the maximum completion request over the 2,000-line typing fixture is 800 ms;
  and
- content above the advertised maximum file size returns HTTP 413 or the
  equivalent typed `too_large` error before writing.

These are focused reference-environment budgets. They are regression gates for
the shipped authoring path, not universal real-time guarantees for arbitrary
hardware or projects.

###### Unified Web IDE shell contract

The standalone Web IDE uses one unified product shell. `/`, `/ui`, `/ide`, and
supported tab deep links resolve to that shell, whose complete bundled module
set includes the workspace, editor, online, hardware, settings, observability,
logs, debug, and command surfaces. The shell preserves explicit tab panels,
status-bar state, compact toolbar and overflow actions, hidden-panel CSS
semantics, and ARIA-selected tab state.

Online control defaults to the same runtime web origin and normalizes both
wrapped and direct API payloads. I/O configuration reads and writes follow the
active workspace. The hardware surface preserves the MQTT connectivity probe,
Runtime Cloud topology, and realtime-link transport projection; settings
preserve the documented realtime-link fields. Removed legacy fleet mutation
routes remain not found rather than forwarding to a current Runtime Cloud
operation.

###### Unified Web IDE hardware and settings interaction contract

The `Hardware` and `Settings` tabs are rendered operator surfaces, not
source-presence claims. Their tab buttons use the tab/tabpanel ARIA
relationship, expose exactly one selected tab, update the `/ide/<tab>` route,
and preserve the selected tab across a same-origin reload. `Ctrl+2` and
`Ctrl+3` select Hardware and Settings when focus is outside a code editor.
Changing tabs does not itself write `runtime.toml`, `io.toml`, or
`simulation.toml`.

Hardware loads the active project's current runtime and I/O snapshots before
presenting them as configured state. A planned or offline runtime remains
visibly offline; the canvas, cards, or presence of configuration must not
invent a live connection. The component palette exposes its categories as
expandable buttons with accurate `aria-expanded` state. Hydrated I/O and
runtime communication drivers appear as labeled cards. Expanding and
collapsing the driver region updates both the visible region and the
controlling button's `aria-expanded` value.

Every driver-card configuration action carries one exact settings key and
category. Activating it selects Settings, updates the route, renders the owning
category, and focuses or exposes the corresponding field. Loopback and
simulated drivers share the documented simulated-I/O settings rather than
inventing a separate loopback schema. Runtime and endpoint actions preserve
the selected runtime scope when routing to Settings. Merely opening a
configuration action does not save a value.

Settings renders all categories plus a text filter. Filtering is
case-insensitive over labels, keys, category names, and group text; it reports
the visible and total field counts, hides nonmatching editable fields, and
offers a clear action that restores the category. A no-match query displays
an explicit empty result rather than stale fields. Category selection clears
neither entered values nor the selected runtime. Credential-bearing password
and token fields render with password input semantics. The Advanced category
labels runtime state as read-only and keeps import, export, reset, and
direct-TOML actions distinct from ordinary field edits.

Hardware and Settings use the shared `ide-runtime-selection-changed` event.
An accepted runtime-scope change on either surface is reflected by the other
without a write or feedback loop. Runtime-, I/O-, and simulation-backed fields
retain their distinct persistence targets. Structured JSON fields are parsed
and normalized before any write. Saves are serialized in user-action order,
use the current revision/version precondition, and dispatch the matching
runtime- or I/O-config-updated event only after a successful write. A parse,
authorization, conflict, transport, or backend failure is shown as a failure
and must not be reported as saved.

Automated acceptance must exercise these behaviors in the shipped browser
surface. Module-source string checks may lock packaging details but do not
prove rendering, interaction, routing, focus, accessibility state, or
cross-tab synchronization. Final acceptance also retains a real rendered
screenshot or recording for the Hardware and Settings journeys.

###### Standalone configuration UI workspace contract

The standalone configuration UI projects the current product mode, planned
workspace runtimes, topology, Runtime Cloud configuration and rollout state,
and the active runtime's I/O configuration from one selected workspace.
Planned but disconnected runtimes remain visibly offline/degraded and their
links failed; they are not reported as live because they exist on disk.

Runtime configuration writes use an expected source revision or digest. A stale
precondition returns conflict without changing the file; a current write
updates that runtime's `runtime.toml`. Structured Text writes remain inside the
selected runtime and the validation endpoint reports valid or invalid source
without treating invalid source as a successful validation.

Runtime create and delete operate on one named workspace runtime and are
reflected in subsequent project-state and lifecycle queries. Lifecycle status
identifies the workspace runtime and whether it is managed. Live-target
profiles may be listed, upserted, selected, connected, queried, and removed; an
unreachable target remains disconnected with its last error and active target
visible.

Workspace runtime IDs are trimmed, nonempty, lowercased ASCII identifiers
containing only letters, digits, `-`, and `_`. Resolution is exact after
normalization; duplicate configured resource names reject the workspace.
Workspace discovery admits `runtime.toml` at the selected root and in direct
child directories only, sorts runtimes by normalized ID, and rejects an empty
workspace. A generated runtime contains validated `runtime.toml`, `io.toml`,
and `src/main.st` files beneath a new child named by that ID. Creation never
reuses an existing ID or filesystem path. Deletion cannot remove the last
runtime, the workspace root, or any path outside the workspace root.

Host-group input is trimmed and lowercased. ASCII letters, digits, `-`, and `_`
are preserved; every other character becomes `-`; an empty result is absent.
The generated runtime template uses `default-host` when no nonempty group is
supplied and binds its resource, service, Unix control socket, local web
listener, local discovery, peer mesh, and Dev Runtime Cloud identity to the
normalized runtime.

Configuration revisions are lowercase SHA-256 over the exact UTF-8 file text.
An expected revision is checked before validation or replacement. A mismatch
returns the current revision as a conflict and leaves the file unchanged.
Accepted writes validate the complete candidate and replace the destination
through a sibling temporary file. Structured Text paths are relative to
`src/`, remain contained there, and end in `.st` case-insensitively; absolute
paths, parent traversal, empty paths, and other extensions reject.

Live targets are trimmed HTTP(S) origins. A missing scheme receives `http://`;
an explicit scheme is lowercase `http://` or `https://`. The target contains a
nonempty authority and no user-info, path, query, or fragment; one optional
terminal slash is removed. Profiles are keyed by normalized target and listed
in key order. A blank label defaults to the normalized target. Snapshots expose
the target and label, connection state, active target, last error, and update
time, but never the stored credential token. Removing the active profile also
clears its token, connection state, last error, and cached Runtime Cloud state.

Lifecycle probes accept configured `tcp://<socket-address>` and, on Unix,
nonempty `unix://<path>` endpoints. Empty, malformed, unsupported, or
unreachable endpoints are offline. Status distinguishes an externally running
runtime from a child managed by this configuration UI; stop and restart do not
claim ownership of or terminate an externally managed process.

Every configuration-UI response is JSON with the requested HTTP status.
Structured errors retain the stable `error_code`, message, ordered field-error
list, and optional conflict version without converting an error into `ok:
true`.

##### 6.8.2 Versioned deployment and rollback

`trust-runtime deploy` validates the source bundle before installation and the
copied bundle before changing deployment pointers. A custom deployment label
must be one non-empty filesystem component: absolute paths, `.`/`..`, path
separators, and platform prefixes are rejected. Automatically generated labels
must be unique across consecutive deployments from one process, including
deployments started within the same second.

When deployment signature policy is enabled, validation occurs before any
bundle file or deployment pointer is changed. A payload signed by a currently
trusted, unexpired key is accepted. A tampered payload, unknown key, expired
key, malformed signature, or policy mismatch rejects the deployment without a
partial install. Failure output may identify the key ID and failure class but
must never echo key material or another configured secret. Source paths with an
optional leading `src/` normalize to the same contained bundle destination;
normalization never permits an absolute path, parent traversal, or platform
prefix.

The deployment root contains `bundles/`, `current`, and optionally `previous`.
The pointer targets are resolved relative to the link location when an older
relative link is read; newly written links target the canonical absolute bundle
directory so a relative `--root` cannot create a broken pointer. Replacing a
pointer must also replace a dangling symlink instead of treating it as absent.
An ordinary file or directory at a pointer path is an error and is not deleted
or overwritten.
An existing pointer is eligible for retention or rollback only when it resolves
to a direct child directory of that deployment root's `bundles/`; a pointer to
another filesystem location is rejected rather than adopted.
Both deployment pointer slots are inspected before a new bundle is installed.
If either slot is occupied by an ordinary file or directory, deployment fails
without installing the candidate, changing either pointer, or publishing its
change summary.

Enabled ADS and OPC UA client configuration is self-contained in the installed
bundle. Each configured sidecar path must be a normalized relative path
contained beneath the source bundle, and the file is copied to the same
relative path in the installed bundle. Absolute paths, parent traversal,
platform prefixes, and symbolic links in any copied bundle path are rejected;
deployment never follows a symbolic link while constructing the installed
bundle.

After a successful deployment, `current` identifies the newly validated bundle
and `previous` identifies the bundle that was current immediately before the
deployment. Pruning retains exactly those existing bundle directories and
removes older directories only from the deployment root's own `bundles/`
directory. It must not delete the new rollback target. `rollback` swaps the two
valid pointers without copying or rebuilding either bundle.

Before deployment pointers change, the command constructs a deterministic
change summary that identifies both the bundle being deployed and the previous
bundle, when one exists. It publishes that summary only for a deployment whose
pointer transition succeeds. The `runtime.toml` and `io.toml` status is based
on the complete file content, so a change outside the selected
operator-friendly detail bullets must still be reported as an update rather
than as `unchanged`. Safe-state comparison uses the structured I/O address and
value, not a potentially lossy display form. Source comparison recursively
covers `.st` and `.pou` files case-insensitively beneath `src/`, with stable
relative-path ordering. A source directory traversal or file-read failure
aborts summary construction and the deployment; it is not converted into an
empty source set.

`deployments/<label>.txt` and `deployments/last.txt` contain the last successful
deploy command's change summary. They are not deployment pointers and rollback
does not rewrite them. The `current` symlink is the authoritative active-bundle
identifier before and after rollback.

##### 6.8.3 Managed local fleet scaffolding

`trust-runtime fleet runtime add` creates one managed local runtime project
beneath the selected fleet root and registers it in `fleet.toml`. Runtime names
use the CLI's portable name alphabet and must be unique in a manifest; hand-edited
manifests are subject to the same validation before list or lifecycle actions.
Generated entries use the portable runtime name as a relative project path
beneath the fleet root. A hand-edited nonempty project path is resolved against
the fleet root when relative and retained as a local path when absolute; this
path flexibility does not relax name, endpoint, or port validation.
Every control endpoint must parse under the runtime control-endpoint contract,
every web port must be nonzero, and TCP control ports and web ports must be
unique across roles and runtime entries. Ambiguous or malformed manifests fail
before filesystem or lifecycle mutation.

Generated control and web listeners use distinct available loopback ports. The
generated control credential is 32 bytes from the operating-system secure
random source and is encoded without padding. If secure randomness is
unavailable, scaffolding fails; it must not substitute timestamps, process IDs,
zero-filled bytes, or another predictable credential. Resource names are
derived deterministically from the portable runtime name, with a nonnumeric
fallback/prefix. Generated runtime and I/O configuration is parsed and
validated before the manifest is updated, and the source scaffold is emitted
deterministically.

Fleet lifecycle status is control-endpoint evidence, not PID-file inference.
Failure to connect means `stopped`; once an endpoint accepts a connection, an
authentication rejection, malformed envelope, oversized response, mismatched
response ID, or missing Boolean `ok` is a command error and must not be
flattened to `stopped`. Failure to read or parse the managed project's control
credential is likewise an error. The PID file is advisory process metadata.

A successful `shutdown` request prepares and writes its complete correlated
control response before it signals the resource stop. The stop signal remains
part of the accepted request and is applied immediately after the response
write attempt; process teardown must not race the operator-visible
acknowledgement off the control connection.

`status` and `logs` are read-only and do not create `.trust-runtime` state when
it is absent. Log tailing retains only the requested final lines in memory
rather than loading the complete log file. A start operation may create its
lifecycle directory and open the child log as spawn setup, but it publishes the
advisory PID only after spawning the process. If PID persistence fails, it
terminates and reaps that child, and if the child exits before the control
endpoint becomes ready, start fails and removes the stale PID file.

Managed-fleet scaffolding always emits a loopback TCP control endpoint because
the manifest and lifecycle commands need one portable explicit endpoint. This
is distinct from the generic runtime default, which uses `unix://` on Unix-like
platforms; a generated Unix socket is created with restrictive permissions
(0600) to prevent accidental exposure.

Managed-fleet projections are operator-facing and secret-free. ADS allowed
clients are rendered as human-readable identity/source summaries rather than
editable raw pin objects. Host names are trimmed, lose a trailing DNS dot, and
prefer the operating-system host name; a blank source is ignored and the stable
literal `local-host` is used only when no host authority exists.

#### 6.9 Debug Attach (Production)

Attach debugging is **optional** in production deployments but must be supported by the runtime
when enabled:

- Attach must not restart or reload the runtime.
- Attach must observe the current state (running/paused/faulted).
- Detach must not alter runtime execution.
- Debug hooks must be side-effect-free when disabled.
- Attach is gated by `runtime.control.debug_enabled`. When disabled, debug control requests are
  rejected. The default is **disabled** in production mode (see `runtime.control.mode`).
- `runtime.control.mode` defaults to `production` and can be set to `debug` for development
  workflows; `runtime.control.debug_enabled` overrides the mode when explicitly set.

##### 6.9.1 Managed debug floating-point ingress

In a managed local debug session, `stIoWrite` and `stIoForce` interpret values
for declared `REAL` and `LREAL` addresses as semantic decimal values. They
reject NaN, positive infinity, negative infinity, overflow to a non-finite
declared value, and raw integer encodings of non-finite IEEE bit patterns
before queueing, forcing, or changing the process image. A rejected request
returns a failed DAP response and preserves both the previous process-image
value and the previous force state. Attach-mode requests remain governed by
the separate runtime control endpoint contract.

##### 6.9.2 Debug mutation lifecycle

Debug writes are one-shot requests. An accepted write is applied at the next
scan boundary and is then consumed. A force is applied at scan boundaries
until the matching release request or a clearing lifecycle boundary. Release
removes the force without writing a replacement value; normal program and I/O
evaluation determine the value at the next scan.

Pause and resume preserve queued writes and active forces because neither
operation creates a new runtime lifecycle. A non-terminating debugger detach or
transport disconnect also preserves them: detach must not alter runtime
execution, and a later authorized client must observe the existing force state.
An authorization change governs subsequent write, force, and release requests;
it does not silently mutate an already accepted force.

Deliberate resource stop, fault handling, and every warm or cold restart are
clearing boundaries. Before safe-state handling or restarted execution can
proceed, the runtime clears queued debug writes and active variable and I/O
forces. Safe-state output therefore has precedence over debug forcing, and a
request accepted before a clearing boundary cannot be replayed into the next
runtime lifecycle.

##### 6.9.3 Simulation coupling thresholds

The file-backed `simulation.toml` loader rejects NaN and positive or negative
infinity in an optional `[[couplings]].threshold` before returning or
activating the configuration. It does not normalize, clamp, or substitute a
threshold. This contract does not govern programmatic coupling-rule
construction or source-value admission.

##### 6.9.4 Simulation model execution

An explicit `[simulation].enabled` value controls activation. When that field
is omitted, the loader enables simulation if the file contains at least one
coupling or disturbance, or enabled physics with at least one joint. An
explicit `enabled = false` remains authoritative even when model entries are
present. The loader preserves the configured seed and positive time scale,
sorts file-backed disturbances by due time, and preserves the declared
coupling and physics model. The seed is part of replay identity; changing it is
not required to alter a model that contains no stochastic element.

Post-cycle simulation observes the completed output image. A changed coupling
value is queued for its configured delay, with `source >= threshold` selecting
`on_true` and a lower value selecting `on_false`. Physics also queues encoder
feedback after stepping. Neither path writes the input image during
post-cycle processing. Pre-cycle processing applies due effects in due-time
then insertion order, so a zero-delay post-cycle result first becomes visible
at the following pre-cycle boundary.

A scripted fault disturbance in an accepted file-backed or programmatically
constructed model does nothing before its due time. At or after that time it
faults the runtime and returns a visible simulation error.
Accelerating the scheduler clock does not reinterpret scaled model time as
active watchdog execution time. An explicitly stopped, non-overrunning
accelerated resource reaches `Stopped` without a terminal error; genuine active
execution overruns remain governed by the separate watchdog contract.

The in-tree physics backend accepts revolute joints with output-bit enable
sources and input-word feedback targets. A physics feedback target that
duplicates a coupling target or an earlier physics target is rejected before
activation. Each enabled joint advances by its fixed step, clamps to its
configured limits, and publishes the rounded product of clamped angle and
counts-per-radian, bounded to the unsigned 16-bit range, through the queued
scan-boundary rule. A newly accepted joint starts at angle zero before its first
fixed step. Replaying the same accepted model and inputs produces the same
coupling and physics traces. Helper-selected trace length and a serialized
trace hash are not part of product compatibility. This contract does not yet
define different-seed behavior, programmatic disturbance reordering, or
overflow policy beyond the currently specified finite configuration limits.

##### 6.9.4.1 Simulation configuration and conversion boundaries

An absent optional simulation file yields no configuration; a present
unreadable or malformed file returns a path-qualified configuration error.
With no sections, file-backed defaults are disabled, seed zero, and time scale
one. File-backed time scale must be at least one. `SimulationController`
defensively treats a programmatically supplied scale of zero as one without
silently enabling a disabled model.

Coupling sources are concrete `%Q` addresses and targets are concrete `%I`
addresses. Thresholds are finite. `on_true` and `on_false` require a threshold,
are parsed according to the target process-image width, and must fit that width
without truncation or wrapping. Coupling delays must fit the signed runtime
duration range. Without a threshold, numeric coupling conversion to
`BYTE`/`WORD`/`DWORD` is checked and overflow is an error; `LWORD` preserves the
accepted unsigned value. Bit conversion uses zero/nonzero truth. Fixed byte
strings must fit their configured byte count.

File-backed set disturbances default to kind `set`, require a concrete `%I`
target and a width-valid value, and require `at_ms` to fit the signed runtime
duration range. Fault kind is ASCII-case-insensitive and defaults its message
when omitted. Unknown kinds and missing set fields are rejected. Disturbances
are stably sorted by due time: declarations with equal due times retain file
order.

Physics configuration defaults to the in-tree Rapier backend, a 10 ms fixed
step, and 1000 encoder counts per radian. The step is within
`1..=floor(i64::MAX / 1_000_000)` milliseconds, the largest millisecond value
representable by the signed nanosecond runtime duration. Global and per-joint
counts-per-radian are finite and positive. Revolute joint velocity and bounds
are finite, lower does not exceed upper, IDs are unique, enable sources are
`%QX` bits, and feedback targets are `%IW` words. Disabled physics or enabled
physics with no joints does not infer simulation activation and creates no
physics controller.

Disabled controllers perform no disturbance, coupling, or physics I/O.
Enabled pre-cycle processing applies equal-time disturbances and effects in
stable insertion order. A source value that has not changed since its last
post-cycle observation does not enqueue another effect. Backend address errors
while reading a coupling remain I/O errors; a failed injected or delayed input
write and a physics enable-read failure become visible simulation faults.
These checks prove deterministic in-process model semantics, not physical
plant fidelity.

##### 6.9.5 Runtime control authorization

Runtime control uses the ordered role hierarchy `viewer < operator < engineer <
admin`; a higher role includes lower-role permissions. Authorization is checked
before dispatch, so a denied request must not change runtime, debug, I/O, HMI,
configuration, pairing, or connector state.

A role denial returns the stable wire error code `insufficient_role` together
with the required role in the human-readable error. Missing and invalid
credentials retain their separate authentication error codes. Clients must use
the stable code, not parse the prose, when distinguishing authentication from
authorization failure.

The control endpoint grammar is exact and whitespace-sensitive. `tcp://` is
followed by a numeric IPv4 or bracketed IPv6 socket address with a required
port, and only a loopback IP is admitted. Hostnames, non-loopback addresses,
missing ports, trailing text, case variants, and leading/trailing whitespace
reject before binding. On Unix, `unix://` is followed by a nonempty socket
path; no implicit default path is created. Endpoint accessors preserve the
parsed address/path, and a started server owns the same shared control state
supplied by its caller.

Control source identity preserves registration order, IDs, paths, and text.
Exact stored paths have first priority. For a relative request with a project
root, lookup next considers `<root>/<request>` and, for a single path
component, `<root>/src/<request>`. Existing candidates may match an existing
registered path through filesystem canonicalization. Only after those checks
may a relative suffix match be used, and it succeeds only when exactly one
registered path has that suffix. Ambiguous suffixes and unmatched absolute
paths return no file. Source-text lookup is exact by file ID and returns the
first registered row for a duplicate ID.

Web-to-control dispatch injects a request token only when the JSON payload has
no `auth` member; an explicit string, null, or malformed `auth` member is never
overwritten. Communication operations `comm.apply`, `comm.test`,
`comm.browse_symbols`, and `ads.import_symbols.apply` receive one
server-owned `credential_channel` parameter. Missing client identity, `unix`,
`loopback`, and numeric loopback socket addresses classify as
`trusted_same_host`; every other client identity classifies as
`untrusted_remote_plain_tcp`. The server-owned value replaces a caller-supplied
value, but a non-object parameter shape remains unchanged for the handler to
reject. Other operations receive no injected field.

Successful status responses whose result is an object include access
capabilities for the authenticated role. Viewer and Operator cannot write,
force, release, or write HMI values; Engineer and Admin can. A denied role
includes a stable human-facing reason beginning with the capitalized role,
while Engineer and Admin use JSON `null`. Non-object or absent results are not
rewritten.

Authentication and transport defaults are:

- A configured control token identifies the Admin role. A valid pairing token
  identifies its stored role. Missing and invalid tokens fail before role
  authorization whenever control authentication is required.
- An untrusted network client without configured authentication receives only
  Viewer authority. A local Unix control socket with no configured token may
  use Admin authority because the socket is local and created with mode 0600.
- An unsupported request type performs no handler action. A request type that
  is not explicitly classified by the role policy uses an Admin fail-safe until
  it is reviewed; it must never inherit Viewer authority by default.

The explicit operation matrix is:

| Minimum role | Operations |
|---|---|
| Viewer | `status`, `health`, `tasks.stats`, `events.tail`, `events`, `faults`, `config.get`, `connectors.status`, ADS/discovery/status/route-plan reads, `comm.capabilities`, `comm.schema`, `comm.discover`, `fleet.topology`, `io.list`, `io.read`, HMI schema/value/trend/alarm/descriptor reads, historian reads, debug state/stack/scope/variable/location reads, `breakpoints.list`, `var.forced` |
| Operator | `pause`, `resume`, `restart`, `hmi.alarm.ack`, `pair.claim` |
| Engineer | step operations, breakpoint mutation, `eval`, `set`, variable and I/O write/force/release, `debug.evaluate`, HMI write/descriptor/scaffold mutation, communication apply/test, ADS server doctor actions |
| Admin | `shutdown`, `bytecode.reload`, ADS route add/remove, pairing start/list/revoke, and security/exposure configuration listed below |

Parameter-sensitive operations retain these boundaries:

- ADS doctor and symbol import/browse are Viewer-only for cached/offline reads
  and Engineer for live-device or write-enabled work. Presence of a non-null
  live `target` requires Engineer even if a snapshot is also present; later
  request validation may still reject that ambiguous combination.
  `ads.import_symbols.apply` is always write-enabled and therefore always
  requires Engineer, including when its source symbols came from a cached
  snapshot.
- `config.set` requires Admin if any request key is `control.auth_token`,
  `control.mode`, `control.debug_enabled`, `web.auth`, `mesh.auth_token`,
  `mesh.role`, `mesh.zenohd_version`, `mesh.plugin_versions`,
  `runtime_cloud.profile`, `runtime_cloud.wan.allow_write`, or
  `runtime_cloud.links.transports`. Other currently supported configuration
  keys require Engineer.
- Debug variable and I/O write, force, and release operations require Engineer
  for every address class. Viewer and Operator requests are denied before any
  queued write or force state changes. Enabling the debug surface itself
  requires Admin; an Engineer may use an already enabled surface but cannot
  enable it.

The registry is the single classification authority. Its operation names are
exact, lowercase wire tokens; case variants, whitespace variants, and unknown
names are unclassified and receive the Admin fail-safe. Every dispatchable
operation appears exactly once, no non-dispatchable alias receives authority,
and the debug-surface flag is true only for debug state, stack, scope,
variables, locations, pause/resume/step, breakpoint, evaluation, variable
write, variable force, and variable release operations. A classification
change is a security-contract change and requires the complete role matrix to
be reviewed, not merely one example request.

##### 6.9.6 Control credentials, pairing, and TLS materials

Role text is trimmed and matched ASCII-case-insensitively against exactly
`viewer`, `operator`, `engineer`, and `admin`; serialization and display use
those lowercase spellings. Permission inclusion follows that exact total
order. Secret comparison returns true only for equal bytes and equal length,
including the empty string, and examines the complete expected byte length
even when the provided value is shorter.

Control authentication applies this precedence:

1. A configured primary control token, when exactly matched, grants Admin.
2. Otherwise a supplied valid pairing token grants its stored role.
3. If credentials are required, an absent token returns
   `missing_auth_token` and a supplied but unmatched token returns
   `invalid_auth_token`.
4. Without required credentials, a client explicitly identified as `unix` or
   an in-process client with no transport identifier receives Admin. A
   network-style client identifier containing `:` receives Viewer. An opaque
   non-network identifier does not by itself reduce local authority.

Credential-state lock failure is authentication failure. It cannot be
interpreted as “no token configured” and cannot grant local or network
authority.

A pairing session owns at most one pending code. Starting again replaces the
previous pending code. A code is exactly six ASCII decimal digits, expires
300 seconds after creation, and remains valid at its exact `expires_at`
second. Claim input trims surrounding whitespace. A wrong code does not
consume the pending session; an expired or successful claim does. A successful
claim returns one URL-safe, unpadded 32-byte random token, stores it enabled,
and assigns a unique stable ID even when multiple claims occur in one clock
second.

The default pairing role is Operator. Viewer, Operator, and Engineer requests
retain their requested role; an Admin request is reduced to Engineer so the
pairing flow cannot mint an Admin credential. At most 256 enabled pairing
tokens may exist. Reaching that bound rejects the claim without creating a
token, and a later claim is possible after a token is revoked. Tokens expire
30 days after creation and remain valid through their exact `expires_at`
second. Validation accepts only an enabled, unexpired, byte-exact token and
returns its stored role.

Listing preserves storage order, never exposes the token, and reports only a
Unicode ellipsis plus the last at most four characters. Revoking an exact ID
disables only that token and is idempotently false when the ID is absent.
Revoke-all returns the number that changed from enabled to disabled. Disabled
tokens remain visible until expiry but never validate and are not counted
toward the enabled-token limit.

The pairing file is a JSON object with a `tokens` array. An absent file loads
an empty store. Unreadable or malformed content grants no token authority.
Legacy rows with `expires_at = 0` derive
`created_at + 30 days` with saturating arithmetic; already expired legacy rows
are pruned rather than temporarily revived. Expired rows are removed on load
and before every pairing, claim, validation, list, or revoke operation.
Successful token creation and revocation become externally successful only
after the resulting file is durably written. On Unix the credential file mode
is `0600`; replacement must not expose a partial JSON credential set.

TLS material loading is inert when TLS mode is disabled: paths may be absent
and no file is read. Every enabled mode requires certificate and private-key
paths. An absolute path is used unchanged; a relative path requires a project
root and resolves against it. An optional CA path resolves by the same rule.
Read failures identify certificate, key, or CA ownership and the resolved
path. When CA is omitted, the server certificate bytes are the client trust
root. The tiny-http projection contains owned copies of certificate and
private-key PEM only.

A rustls server configuration requires at least one valid certificate and one
valid private key, and rejects malformed, empty, or certificate/key-mismatched
material. A rustls client configuration requires at least one valid CA
certificate and rejects malformed or empty trust roots. Provider installation
is process-wide and repeatable; failure remains a visible control error rather
than silently selecting an insecure configuration.

##### 6.9.7 Runtime web authentication and POST admission

The embedded web surface applies one shared authentication contract before a
route may read or mutate runtime state. In `local` mode, authority is derived
only from the request socket: an IPv4 or IPv6 loopback peer receives Admin and
every non-loopback or unavailable peer identity is unauthenticated. In `token`
mode, the first `X-Trust-Token` header name is matched
ASCII-case-insensitively, while its value is byte-exact and is not trimmed. An
exact primary web token grants Admin. Otherwise, an exact enabled pairing token
grants its stored role. Missing, unknown, expired, or disabled tokens are
unauthenticated. Failure to read primary credential state fails closed and
must not fall through to pairing authority.

Role authorization follows the hierarchy in section 6.9.5. An authenticated
role below the route requirement returns `forbidden`; absent authority returns
`unauthorized`. Both denials are JSON objects with `ok: false` and the stable
error text, use `application/json`, and return HTTP 403 and 401 respectively.
The accepted token value is forwarded to control dispatch. A server-owned
internal control token is used only when the web request supplied no token; it
must not replace an explicit token. The `X-Trust-Ide-Session` header name is
also ASCII-case-insensitive and its value is preserved exactly.

Every state-changing API route applies the following POST admission rules
before parsing or dispatch:

- Where JSON is required, `Content-Type` is the media type
  `application/json`, matched ASCII-case-insensitively and optionally followed
  by media-type parameters. Prefix and suffix lookalikes such as
  `application/jsonx` and `application/json-patch+json` are rejected with HTTP
  415 and `contract_violation`.
- A request without `Origin` is accepted only when its socket peer is
  loopback. A non-loopback or unavailable peer without `Origin` is rejected
  with HTTP 403 and `permission_denied`.
- A supplied Origin requires Host. The literal opaque origin `null` is denied.
  Otherwise, after ASCII case normalization and removal of at most one terminal
  slash, Origin must equal `<scheme>://<host>`, where the scheme is `https`
  exactly when web TLS is enabled and Host is the trimmed request Host value.
  Host, scheme, port, path, user-info, and prefix/suffix mismatches are denied;
  origin comparison never uses prefix matching.
- JSON body admission is inclusive at the configured byte limit. Reading at
  most one byte beyond that limit distinguishes an oversized body from a
  malformed JSON body without unbounded allocation. A body of exactly the
  limit is parsed; a larger body returns HTTP 413. Empty or malformed JSON
  returns HTTP 400. Limit arithmetic is overflow-safe for every `usize` value.

POST-policy denials are JSON with `Content-Type: application/json`, `ok:
false`, a stable `denial_code`, and the documented error text. Admission proves
only request framing, origin, and body shape; route authentication,
authorization, and semantic validation remain separate mandatory checks.

Shared web helper grammar is deterministic:

- Header names are ASCII-case-insensitive; the first matching value is trimmed
  at its outer ASCII whitespace and otherwise preserved.
- A web listen socket projects to `http` or `https` according to TLS. Wildcard
  IPv4 projects to `localhost`; hostnames and IPv4 retain their host and port;
  IPv6 is emitted in bracketed URL authority form.
- Query parsing uses the first exact key. `limit` is an unsigned decimal
  integer. General values decode `+` as space and valid `%HH` octets before
  lossy UTF-8 conversion. A malformed or incomplete percent escape is
  preserved literally rather than fabricating a NUL byte or silently deleting
  source characters.
- A Runtime Cloud rollout action path is exactly
  `/api/runtime-cloud/rollouts/<nonempty-id>/<nonempty-action>` with no extra
  path component. Outer whitespace inside either captured component is removed;
  path-prefix, suffix, and case variants do not match.
- Probe JSON succeeds only for `ok: true`. Success projects `result.plc_name`,
  falling back to `result.resource` and then `PLC`, plus `result.state` falling
  back to `online`. Invalid JSON, missing/false `ok`, or a non-string error
  projects `{ok:false,error:"unreachable"}`; a string error is preserved.
- QR output is a complete SVG for the exact input bytes or a visible control
  error when the payload exceeds encoder capacity. Wall-clock helpers expose
  Unix-epoch milliseconds and nanoseconds, with nanoseconds never earlier than
  the contemporaneous millisecond value multiplied by one million.

#### 6.10 Configuration and Resources

IEC configurations model one or more resources (IEC 61131-3 Ed.3, §6.8.1;
Table 62). The runtime scheduler supports several independently constructed
resources, each scheduled in its own OS thread. The current
`CompileSession::build_runtime` source profile constructs exactly one resource:
it accepts zero or one explicit `RESOURCE` and rejects a multi-resource source
configuration instead of flattening it. Multi-resource orchestration therefore
uses separately constructed runtimes; one source build does not yet project an
IEC multi-resource configuration into several runtime objects.

Cross-resource data exchange is limited to explicitly declared globals (e.g., `VAR_GLOBAL` in configuration scope). (IEC 61131-3 Ed.3, §6.8.1; Table 62) Shared globals are synchronized under a single configuration lock: each resource cycle copies shared values in, executes ready tasks, then writes back updates before releasing the lock. This preserves deterministic ordering while serializing shared-global access.

##### Runtime configuration loading and validation contract

`runtime.toml`, `io.toml`, and enabled connector sidecars are truST product
contracts, not IEC decisions or deviations. Their user-facing field reference
is maintained under `docs/public/reference/config/`; the following rules are
normative for parsing and activation:

- Runtime and I/O TOML use closed schemas. Unknown fields, wrong shapes,
  unsupported enum values, incomplete conditional groups, and invalid security
  combinations fail as `InvalidConfig` before a configuration is returned.
  Validation errors retain the owning file name. Defaults are applied only to
  omitted documented optional fields and never replace a malformed explicit
  endpoint, path, interface, producer path, symbol pattern, or version-map
  entry. When any of those values is present, it must be nonempty after
  trimming; list and map parsing must not silently discard blank entries.
- Resource, task, retain, watchdog, ADS worker, OPC UA client polling, and
  other millisecond values converted into the signed runtime `Duration` must
  be representable without narrowing. The runtime duration stores signed
  nanoseconds, so unless a field has a stronger documented minimum, its
  accepted millisecond range is `1..=9223372036854`
  (`i64::MAX / 1_000_000`); larger values reject instead of overflowing or
  panicking during conversion. OPC UA client polling retains its stronger
  minimum of 10 milliseconds.
- `io.toml` accepts exactly one nonempty driver form: `io.driver` with table
  parameters, or a nonempty `io.drivers` list whose names are nonempty and
  whose parameters are tables. Safe-state values are parsed against their
  addressed output width. BYTE, WORD, DWORD, and LWORD accept only their exact
  unsigned ranges; narrowing, wrapping, clamping, or substitution is forbidden.
- TCP control requires a nonempty authentication token. Enabled remote web or
  mesh surfaces obey the configured TLS requirement. Enabled TLS requires the
  documented certificate/key group, provisioned TLS additionally requires a
  CA, signed deployment requires a keyring, and enabled connector/server
  sections must satisfy their documented credential, endpoint, allowlist,
  capacity, and explicit insecure-transport acknowledgement rules.
- Bundle loading requires a project directory, `runtime.toml`, validated
  project or system I/O configuration, and `program.stbc`. ADS and OPC UA
  client sidecars are loaded only when enabled; relative sidecar paths resolve
  against the bundle root, enabled missing sidecars fail visibly, and the
  retained evidence hash covers the exact bytes that were parsed.
- Loading is fail closed. No partially parsed runtime, I/O, connector sidecar,
  or default substitute is returned after any required file read or validation
  failure.

The configuration contract enums use trimmed ASCII-case-insensitive input and
closed vocabularies. Web authentication accepts `local` or `token`. TLS accepts
`disabled`, `self-managed` (with the compatibility spelling `self_managed`),
or `provisioned`. Mesh role accepts `peer`, `client`, or `router`. Runtime-cloud
profile accepts `dev`, `plant`, or `wan`; Plant and WAN require secure
transport while Dev does not. Preferred runtime-cloud transport accepts
`realtime`, `zenoh`, `mesh`, `mqtt`, `modbus-tcp` (with compatibility spelling
`modbus_tcp`), `opcua`, `discovery`, or `web`. OpenOT fence mode accepts
`fenced` or `unfenced`, and its source accepts `heartbeat` or exact `st-fb`.
Canonical output always uses the hyphenated lowercase spelling. Empty text,
near matches, extra words, unsupported punctuation, and NUL-bearing values
reject with the owning configuration key in the error.

Default optional runtime connectors are disabled. The ADS client sidecar is
`ads.toml` with a 20-millisecond worker tick; the OPC UA client sidecar is
`opcua_client.toml` with a 250-millisecond polling interval. Default OpenOT
telemetry is disabled, has an empty path, capacity 4096, fenced mode,
heartbeat source, no producer alias or producer list, and does not allow
unfenced proof. These defaults apply only when the corresponding documented
configuration is omitted; they cannot replace malformed explicit values.

##### Runtime web setup and I/O projection contract

The web setup surface uses the explicitly selected bundle root, otherwise the
current directory. A derived default resource name is the final path component
with every non-ASCII-alphanumeric character replaced by `_`; a missing
component uses `trust-plc`. Existing valid `runtime.toml` and project
`io.toml` values take precedence over platform defaults. Setup need remains
true while runtime configuration is absent or neither project nor system I/O
configuration exists.

I/O mutation accepts either a nonempty ordered `drivers` list or the legacy
single `driver`, with the list taking precedence when present. Driver names are
trimmed and nonempty. Parameters are JSON objects converted recursively to
TOML tables; null becomes an empty string, and Boolean, representable integer,
float, string, array, and object shapes retain their value. A top-level
non-object parameter rejects rather than being wrapped or discarded. Every
driver is enabled in the resulting configuration.

The I/O response preserves driver order, parameters, safe-state order, source
identity, and system-I/O selection. Its legacy primary `driver` and `params`
are the first configured driver. Address projection uses `%I`, `%Q`, or `%M`
plus `X`, `B`, `W`, `D`, or `L`, retains bit indices, and emits `%<area>*` for a
wildcard. Boolean safe values use `TRUE`/`FALSE`; integer and bit-string widths
use unsigned or signed decimal without narrowing.

Saving project I/O validates the complete rendered TOML before replacement.
Selecting system I/O removes only the project `io.toml`; selecting project I/O
does not change system configuration. Setup validates and writes
`runtime.toml`, writes validated project `io.toml` unless system I/O is
selected, and invokes system setup only when explicitly requested.

Source listing inspects the direct `src/` directory, includes regular `.st`
files case-insensitively, and sorts names. Source reads canonicalize both the
`src/` root and requested path and reject traversal or symbolic escape. HMI
asset reads apply the same containment rule under `hmi/` and admit only a
regular file with lowercase `.svg`; another extension, directory, traversal,
or symbolic escape rejects.

#### 6.11 Bytecode Format (Overview)

The executor consumes a stable bytecode container format emitted by the compiler. See the "ST Bytecode Format Specification" section in this document for details.
- Instruction encoding and versioning
- Program/function/function-block layouts
- Constant pools and type descriptors
- Resource, task, and POU metadata required by the runtime (process image sizing, task associations)

The runtime rejects unsupported major bytecode versions before configuring resources.

#### 6.12 Browser UI, Discovery, and Mesh (Operational UX)

Operational UX is **browser‑first** (no app). A built‑in web service exposes an
operational UI and discovery metadata. This is **implementer‑specific** and
outside IEC 61131‑3 scope.

Configuration (in `runtime.toml`):

```
[runtime.web]
enabled = true
listen = "0.0.0.0:8080"
auth = "local"              # local|token

[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = ["eth0", "wlan0"]

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
auth_token = "change-me"
publish = ["Status.RunState", "Metrics.CycleMs", "TempA"]

[runtime.mesh.subscribe]
"Plant-1:TempA" = "RemoteTemp"
```

Rules:
- **Local‑only by default**. Remote access must be explicitly enabled.
- **Request isolation is bounded.** The runtime admits read-only requests on a
  four-worker lane with at most 32 queued requests and body-bearing or mutating
  requests on a separate four-worker lane with at most four admitted requests.
  An incomplete request body must not prevent an unrelated admitted request
  from being served.
- **Saturation fails visibly.** A request that cannot be admitted without
  exceeding its lane's fixed bound receives HTTP `503 Service Unavailable`, a
  `Retry-After: 1` header, and the stable denial code `server_busy`; the server
  does not wait behind an incomplete body or create an unbounded worker thread.
  These limits are internal runtime safety constants in this contract version,
  not `runtime.toml` settings.
- **Discovery uses mDNS/Bonjour** on the local LAN only.
- The advertised and browsed service type is
  `_trust._plc._tcp.local.`. Service construction enables automatic interface
  addresses. The TXT projection preserves stable runtime `id` and `name`,
  web port and TLS flag, optional mesh port, control endpoint, and a trimmed
  nonempty host group; a missing or malformed optional value is absent rather
  than fabricated.
- A resolved service replaces the entry with the same runtime ID and receives a
  fresh observation timestamp. A removal event matches the exact entry ID or
  runtime name, or an mDNS instance name whose final `-<runtime-name>` suffix
  identifies that runtime. It removes only the matching entry.
- **Remote access** supports manual add and invite/QR pairing only.
- **Data sharing** is explicit (publish/subscribe mapping only).

##### Realtime T0 transport and cycle contract

Realtime T0 is a truST host-runtime transport outside IEC 61131-3. It is
separate from the generic network mesh described below.

**Route and QoS admission**

- `QosTier::T0HardRt` admits only `RealtimeRoute::T0HardRt`.
- `QosTier::T1Fast`, `QosTier::T2Ops`, and `QosTier::T3Diag` admit only
  `RealtimeRoute::MeshIp`.
- A T0 publisher or subscriber bind requested through `MeshIp` returns
  `T0ErrorCode::ContractViolation`. No mesh/IP connection is attempted.
  `fallback_denied_total` and the selected channel's
  `fallback_denied_count` each increase by one for the denied bind. The
  diagnostic contains the stable fragment `non-HardRT`.
- The canonical T0-to-communications error mapping is:

  | T0 error | Communications error |
  |----------|----------------------|
  | `NotConfigured` | `NotConfigured` |
  | `ContractViolation` | `RtContractViolation` |
  | `SchemaMismatch` | `SchemaMismatch` |
  | `StaleData` | `StaleData` |
  | `TransportFailure` | `TransportFailure` |

  The stable remediation for `RtContractViolation` is:
  `Use pre-bound T0 handles and fixed-layout payloads; generic IP mesh is non-HardRT.`

**Registration, shared-memory handshake, and binding**

- Channel registration validates a nonempty channel identity, schema identity,
  and schema hash plus a positive fixed slot size and positive bounded-spin
  limits before publishing readiness.
- Failure to create or open the configured shared-memory root returns
  `T0ErrorCode::TransportFailure`; its diagnostic identifies T0 shared-memory
  directory provisioning with the stable fragment
  `failed to create T0 SHM directory`.
- When page pinning is configured as required, an unavailable pinning provider
  or failed pinning operation returns `T0ErrorCode::TransportFailure`; the
  diagnostic contains the stable fragment `required page pinning failed`.
- Opening an existing channel mapping requires its channel identity, schema
  identity, schema version, schema hash, slot size, stale threshold,
  bounded-spin limits, and ownership to match the requested contract. A
  mismatch returns `T0ErrorCode::TransportFailure` with the stable fragment
  `schema_hash contract mismatch` before the runtime can use that mapping.
- A same-length mutation of the reviewed shared-memory header is rejected on
  the next registration as `T0ErrorCode::TransportFailure`. The bounded
  deterministic corruption campaign does not authorize parsing arbitrary
  damaged metadata or changing the mapping length.
- Publisher and subscriber binding requires a previously registered channel,
  `T0HardRt`, `fixed_layout = true`, a positive payload size no greater than
  the registered slot, an exact schema-hash match, and initialized pinned
  channel state. An absent channel returns `NotConfigured`; a schema mismatch
  returns `SchemaMismatch`; a variable or invalid layout returns
  `ContractViolation`; and unready or unpinned state returns
  `TransportFailure`.
- The resulting publisher and subscriber handles retain the bound channel,
  route, payload size, and schema identity. Publish and read operations do not
  perform route discovery, key parsing, generic mesh calls, or substitute an
  unbound transport. The source-bound T0 hot-path gate keeps mesh/discovery and
  key-parsing APIs out of the reviewed shared-memory modules while retaining
  the explicit diagnostic that generic IP mesh is non-HardRT.

**Latest payload, overrun, stale, and bounded-spin results**

- A successful publish commits one payload whose length equals the bound
  fixed-layout size. A later publish before the preceding value is read
  replaces the readable payload with the latest value and increments the
  channel's cumulative `overrun_count`.
- A successful fresh read copies that latest committed payload, returns its
  monotonically increasing sequence and byte count, reports
  `dropped_updates = newest_write_sequence - previous_read_sequence - 1`, and
  exposes the cumulative `overrun_count`. The read advances the channel's read
  sequence and resets its consecutive stale-miss count.
- A read with no new committed payload returns `NoUpdate` while the consecutive
  miss count remains below `max(stale_after_reads, 1)`. Reaching that threshold
  returns `T0ErrorCode::StaleData` and increments `stale_count`.
- A reader observing an unstable writer retries only within both configured
  bounds: `max_spin_retries` and `max_spin_time_us`. Exhausting either bound
  returns `StaleData`, increments `spin_exhausted_count`, and increments
  `stale_count`.
- Publish and read both require initialized pinned channel state. Either
  operation returns `TransportFailure` instead of accessing an unready or
  unpinned mapping.

**Cycle exchange and cloud-work isolation**

- `begin_cycle(cycle)` records the cycle identity, clears the current cycle's
  pre/post observations, and restores
  `max_cloud_ops_per_cycle` as that cycle's cloud-work budget. It does not
  clear the cumulative denied-work count.
- Within a cycle, `PreTask` may be marked at most once. `PostTask` may be
  marked at most once and only after `PreTask`. `PostTask` before `PreTask`, or
  a duplicate exchange point in the same cycle, returns
  `T0ErrorCode::ContractViolation`.
- A cloud-work request receives
  `min(requested_ops, cloud_budget_remaining)`. The remainder is denied,
  subtracted from no T0 budget, and added to `denied_cloud_ops_total`; granted
  work reduces only the current cycle's cloud budget.
- Consequently, a budget of three grants two operations and then one from
  requests of two and three, recording two denied operations. A budget of one
  across ten separately begun cycles grants ten of ten requests for fifty and
  records 490 denied operations.
- Functional T0 publish/read ordering, payload sequences, and stale/overrun
  counters remain independent of excess cloud-work admission. This is an
  accounting and ordering contract, not a measured execution-time guarantee.

**Cross-process visibility and proof ceiling**

- Two processes using the same shared-memory root and an exact matching channel
  contract may bind the permitted publisher/subscriber roles. After a
  successful committed publish, the subscriber can observe the same payload
  and a fresh read result through the shared mapping.
- Focused software checks of route rejection, shared-memory exchange,
  functional sequences, and counters do not prove worst-case latency, cycle
  deadlines, scheduler jitter, PREEMPT_RT configuration, production memory
  locking, physical topology, or hard-real-time certification. Those claims
  require separately named platform and timing evidence.

##### Linux realtime host profile contract

The Linux realtime profile parser accepts `fifo`, `rr`, and `other` as the
documented scheduler identities. An enabled profile rejects scheduler `other`
and duplicate CPU-affinity entries before host activation. Kernel readiness
classification distinguishes an explicit PREEMPT_RT signal from an explicit
non-RT signal and retains an unknown result when neither signal is present.
The reviewed `/proc` parsers preserve the user-space realtime scheduler and
priority and read the requested memory-lock field without inventing a value.

When observed FIFO policy and priority match the request, the reviewed
observation contains no errors. A strict profile hook returns an error and
retains the collected error detail when affinity or another reviewed readiness
check fails; it cannot report a failed profile as active. These parser and
failure-path checks do not prove that the validation host is PREEMPT_RT, that
page locking succeeded in production, or that any deadline or jitter bound was
met.

##### Mesh envelope, mapping, and numeric codec contract

- A published mesh envelope carries the runtime source identity, an unsigned
  sequence, a publication timestamp, and the encoded PLC value. A successful
  encode/decode round trip preserves the source, sequence, and supported value.
- A subscription source uses `peer:key`. Parsing splits at the first colon,
  trims both parts, and accepts the mapping only when both the peer and remote
  key remain nonempty.
- Inbound mesh values are decoded against the current local target type. Integer
  values outside that IEC type's range are rejected rather than wrapped, and a
  JSON number targeting `REAL`/`LREAL` is accepted only when the resulting
  runtime value is finite.
- A rejected mesh value queues no local update and leaves the current PLC value
  unchanged. The runtime does not clamp, wrap, normalize, or substitute a
  default value.
- The supported scalar codec is bounded and total over arbitrary input bytes:
  malformed bytes may be rejected, but decoding must not panic. This crash-only
  malformed-input guarantee does not claim a particular diagnostic or that
  every malformed byte sequence is rejected.

##### Mesh control-plane readiness, liveliness, QoS, and snapshot contract

- Mesh QoS classification uses slash-delimited key-space segments. `active`
  takes precedence over `cfg`, which takes precedence over `diag`; a key with
  none of those segments uses the fast data profile.
- The liveliness registry keeps a unique current peer set and a bounded
  insertion-ordered join/leave history. A leave removes only that peer and
  records the transition.
- Cloud readiness requires an established session, liveliness token/subscriber,
  and successfully declared identity and catalog queryables. A bounded wait
  whose deadline expires before all four conditions hold returns an error
  rather than readiness.
- Startup readiness flags for identity and catalog mean that the corresponding
  queryables were declared successfully. Those flags alone do not prove a
  query/reply payload exchange.
- A resource snapshot response that times out or disconnects is an error. It
  must not be returned as a successful empty snapshot.

- TOML remains the source of truth; offline edits are supported.

The local terminal dashboard is a projection of the same control contract. It
parses resource status, tasks, I/O drivers, events, simulation state, and
settings without inventing missing health. Settings include cycle interval and
simulation fields. Command routing preserves the beginner guard and pause
semantics; prompt navigation cannot mutate a read-only view. The rendered
dashboard keeps its reviewed status, task, I/O, and event regions in a stable
layout so an operator can compare snapshots without hidden field movement.

##### Runtime Cloud state, dispatch, and rollout contract

Runtime Cloud state projects the connected runtime context, discovery
transport metadata, topology nodes and links, and their current lifecycle and
health without inventing connectivity. A fresh mesh disconnect first becomes
stale and only later partitioned according to the documented staleness
threshold. Plant and WAN state access requires the secure profile transport
preconditions. Removed device-list mutation routes return not found rather than
changing topology through a legacy alias. Corrupt persisted configuration
state becomes an explicit error with retained parse evidence and is never
replaced by an in-sync default.

Dispatch evaluates the selected target before any operation. An unreachable
target or exhausted bounded query budget returns a visible denial and must not
fall back to applying the request on the local runtime. A local target remains
operational when an unrelated peer is partitioned. A successful remote status,
configuration, or I/O proxy request returns the remote payload. Configuration
dispatch preserves one request correlation through the target control audit,
and both successful and failed target operations retain their matching audit
identity and semantic result.

A rollout advances through its ordered target states and completes only after
the applied configuration is verified. An apply or reconciliation failure
makes both the rollout and affected target fail with retained error evidence.
Abort transitions an active rollout and its unfinished targets to aborted.
Pause, resume, or abort against a terminal rollout returns conflict and cannot
rewrite the terminal result.

##### Runtime Cloud action preflight and profile contract

Runtime Cloud federation is a truST product contract, not an IEC decision or
deviation. The `/api/runtime-cloud/actions/preflight` and
`/api/runtime-cloud/actions/dispatch` surfaces use the current `1.x` contract;
minor-version changes within that major are additive, while another major or a
malformed version is a contract violation.

Every action request has a nonempty `request_id`, `connected_via`, `actor`, and
`action_type`, plus at least one nonempty target runtime. A target runtime may
occur only once in a request. `connected_via` must identify the local runtime.
Malformed global request state is a denied preflight even when no per-target
decision can be emitted; an empty decision list must never turn a contract
violation into `allowed: true`. Rejecting an empty, blank, or duplicate target
list prevents both a false successful no-op and repeated side effects against
one runtime.

Preflight is side-effect free and evaluates the complete request before
dispatch. `status_read`, `cfg_apply`, and `cmd_invoke` are the supported action
families; unknown actions are contract violations. Viewer may read status,
Operator may invoke ordinary commands, Engineer may apply ordinary
configuration, and the security/exposure keys listed in the runtime control
authorization matrix require Admin. Stale and unreachable nonlocal targets are
denied with stable per-target reasons. A dispatch whose `dry_run` flag is true
returns the preflight result without performing the mapped control operation.

Profile policy is applied after the common contract, role, freshness, and
reachability checks:

- `dev` does not use `runtime.cloud.wan.allow_write`; cross-site writes remain
  governed by the ordinary role and target checks.
- `plant` requires token-authenticated TLS and secure transport metadata for a
  nonlocal target, but it does not use the WAN write allowlist.
- `wan` has the same secure transport preconditions and additionally denies
  every nonlocal `cfg_apply` or `cmd_invoke` unless an explicit
  action-and-target `runtime.cloud.wan.allow_write` rule matches. `*`, a prefix
  such as `site-b/*`, a suffix such as `*/runtime-b`, and exact target IDs have
  their documented meanings; other embedded-wildcard shapes do not match.

Preflight and dispatch responses retain the caller's `request_id` and
`connected_via`. Each dispatched target produces one result for that target;
successful target results carry the target audit correlation supplied by the
control operation. A denied or failed target must remain visibly denied or
failed and must not be summarized as a successful aggregate action.

A live update to `runtime.cloud.wan.allow_write` uses the ordinary validated
configuration-write path. The accepted exact action and target pattern is
visible in subsequent configuration reads, and the control audit retains the
configuration request correlation. An invalid policy update is rejected before
replacing the active allowlist.

The Runtime Cloud control and I/O proxy planners are action-request builders,
so they must either produce an action that already satisfies this common
contract or reject the proxy request before preflight and before any local or
remote side effect. Both planners validate API compatibility and reject a blank
actor, target runtime, or `connected_via`. A control proxy request may omit its
request ID, in which case the planner generates one; an explicitly supplied
blank request ID is a contract violation. The chosen control proxy request ID
is identical in the action request and forwarded control payload so audit and
response correlation cannot diverge.

##### Runtime Cloud desired-configuration reconciliation contract

Runtime Cloud desired configuration is also a truST product contract outside
IEC 61131-3. A desired write has a nonempty actor and an object-valued merge
patch. Optional expected revision and ETag values are optimistic-concurrency
preconditions; a mismatch rejects the write as a revision conflict. An accepted
write advances the desired revision, records a new ETag and writer, and becomes
pending until the runtime control operation confirms it.

Reconciliation applies an immutable snapshot containing the desired value,
revision, and ETag. The control operation runs without holding the configuration
state lock. If another desired write is accepted while that operation is in
flight, the older completion may acknowledge only the snapshot it actually
applied: it must not copy the newer desired value into `reported`, attach the
older revision or ETag to newer content, or mark the agent in sync. The newer
desired revision remains pending and the next reconciliation attempt applies
it. A failed apply preserves the last confirmed reported value and revision and
surfaces a blocked or error state with the stable reason available from the
control failure. If that failure belongs to a superseded snapshot, its
diagnostic remains visible but the newer desired revision stays pending and
eligible for the next reconciliation attempt.

##### Runtime Cloud contract schema and canonical keyspace

Runtime Cloud API versions use the decimal `<major>.<minor>` form. Equal
versions are exact, versions with the same major are additive-compatible, and
different majors are breaking. Mesh schema evolution is forward-additive only:
field names are nonempty and unique, every field has a nonzero representable
`offset + size` range, and field ranges do not overlap. Every previous field
must remain present with exactly the same offset, size, and type. New fields may
start only at or after the end of the complete previous layout; inserting one
into an existing field or padding range is not an additive append.

Canonical Runtime Cloud keys are assembled from already validated nonempty
site, runtime, group, and identity segments. The root is
`truST/<site>/<runtime>`. Runtime-local zones are `_meta`, `io`, `cmd`, `cfg`,
`diag`, and `svc`; `_meta`, `diag`, and `svc` are reserved. The site-level
`active` slot may be published only by the current Active authority. Desired,
reported, status, and metadata configuration documents use the authoritative
site-level and runtime-alias paths defined by the keyspace helpers. Default UI
and operations staleness is `max(2 * expected_period, 2 seconds)`. A retained
last value may support UI continuity but never a control path.

##### Runtime Cloud topology measurement and same-host contract

Runtime Cloud topology is a truST product projection, not an IEC decision or
deviation. Discovery heartbeat and reachability observations may establish a
node's lifecycle and a communication edge's healthy, stale, degraded, or failed
state. They are not latency or packet-loss measurements. Unless the projection
receives an actual measurement for a metric, `latency_ms_p95` and `loss_pct`
remain absent; it must never manufacture a nominal constant for a healthy edge.

The `T0_HardRt` topology overlay is same-host only. Equal nonempty discovery
`host_group` values are positive same-host evidence; different explicit groups
are negative evidence. Without complete host-group evidence, an exact shared
advertised non-loopback IP address is positive evidence. A loopback address is
host-local and therefore proves only that the advertising runtime is colocated
with the dashboard's known local runtime. It must not classify two arbitrary
peer runtimes as colocated, and the result must not depend on source/target or
runtime-ID ordering.

A realtime preference creates a `T0_HardRt` overlay only while that same-host
evidence holds. The ordinary mesh operations edge may remain visible alongside
the overlay, but it is not a deterministic fallback for T0 and must not be
reported as one.

##### Runtime Cloud HA lease, split-brain, and replay contract

Runtime Cloud dual-host HA is a truST product safety contract outside IEC
61131-3. An action that addresses the active namespace, and every
`cmd_invoke`, requires a nonempty HA group, an external consistent lease
authority, an available lease, one ACTIVE target whose runtime ID is the lease
owner, a valid nonempty fencing token, and no ambiguous leadership. Loss of any
authority or fence condition denies the protected action before side effects
and requires the affected runtime to remain or become `demoted_safe`.

The supplied `payload.ha.targets` map is the asserted complete authority view
for the HA group, not merely a description of the action's target subset. If
more than one entry simultaneously satisfies every ACTIVE lease-owner and
fencing condition, the group is split-brain. Every protected action is denied
with `lease_unavailable`, including an action whose `target_runtimes` names
only one of those candidates. Selecting one candidate in the action must never
hide another valid ACTIVE owner from the split-brain check.

Dual-host commands use a positive `command_seq` per group and runtime. A
completed request ID with the same sequence returns its stored result without
performing the control operation again; reusing that request ID with a
different sequence or submitting a stale sequence is a conflict. While the
first dispatch for a request ID is still owned by the current live coordinator,
another begin attempt is also a conflict and cannot start a second control
operation. A serialized pending request carries no live-process ownership; on
recovery, the same request ID and sequence may be resumed once for
reconciliation and then becomes subject to the ordinary completed-result
deduplication rule.

HMI customization (implementer-specific):
- `hmi.schema.get` returns `theme`, `pages`, and widget-level layout metadata (`page`, `group`, `order`, `unit`, bounds) in addition to stable widget IDs.
- Project-level `hmi.toml` supports:
  - `[theme]` (`style`, optional `accent`)
  - `[write]` (`enabled`, `allow`) for explicit writable-target allowlists.
  - `[[pages]]` (`id`, `title`, `order`)
  - `[widgets.\"<path>\"]` overrides for label/unit/bounds/widget/page/group/order.
- ST-level `@hmi(...)` annotations on variable declarations support `label`, `unit`, `min`, `max`, `widget`, `page`, `group`, and `order`.
- Merge precedence is deterministic: defaults < ST annotations < `hmi.toml` overrides.
- Theme fallback is deterministic: unknown/missing theme values fall back to built-in `classic`.
- `hmi.write` remains disabled unless `[write].enabled = true`, and writes are accepted only for explicit allowlist matches (`id` or `path`) with control authz enforcement.

Operational UX and pairing flow are documented internally.

#### 6.13 Debugging and Diagnostics

The runtime emits structured events for debugging and testing:
- Cycle start/end (with timestamp)
- Task start/end (with task name, priority)
- Breakpoint hit / step events (statement boundaries)
- Fault and overrun notifications

These events are consumed by the debugger (`trust-debug`) and test harnesses to validate behavior deterministically.

### 7. Build Configuration

#### 7.1 Feature Flags

```toml
[features]
default = ["debug"]
debug = []  # enable debug instrumentation and runtime events
```

#### 7.2 Conditional Compilation

Desktop builds use the standard library unconditionally. Embedded support will introduce additional `cfg` gates for alternative clock implementations.

### 8. Why Not Alternatives

| Alternative | Reason for rejection |
|-------------|---------------------|
| **Containers** | Cannot run on microcontrollers. Adds complexity without benefit for this use case. |
| **FreeRTOS POSIX simulator** | Adds unnecessary layer on desktop. Not production-grade. |
| **Embassy (async Rust)** | Cooperative scheduling unsuitable for deterministic PLC timing. |
| **WASM** | Adds complexity. Real-time I/O interaction awkward. Could be future target. |
| **Transpile to C** | Loses runtime flexibility. Debugging harder. |

### 9. Future Considerations

**WebAssembly target:** The runtime core (executor, timers) could compile to WASM for browser-based simulation. Would require a WASM-friendly `Clock` implementation.

**Remote I/O:** Process image exchange via Modbus TCP is architecturally separate from the clock layer. Networking abstraction would be added alongside the `Clock` trait, not replacing it.

**Retain variables:** Persistent storage across power cycles requires platform-specific implementation (filesystem on desktop, flash on embedded). This is orthogonal to the `Clock` trait and would be added as a separate storage interface if needed.

### 10. Summary

The thin clock abstraction approach provides:

1. **IEC-aligned task scheduling** (periodic and event tasks with defined priority rules)
2. **Minimal clock surface** - easy to maintain and verify
3. **Clear separation** - runtime logic vs clock primitives
4. **Deterministic behavior** - explicit scheduling and I/O latching rules
5. **Testability** - full runtime runs natively on development machine

The runtime is implemented in Rust, using the standard library for desktop targets initially. Embedded backends are planned with identical runtime logic and alternate clock implementations.
