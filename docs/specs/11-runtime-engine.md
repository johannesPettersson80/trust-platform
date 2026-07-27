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

Insertion stores the complete runtime `Value` without changing its concrete
variant or nested contents. This is a deterministic snapshot-construction
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
variant. A missing file alone represents an empty snapshot; a path that exists
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
- a structure field added by the current declaration receives its declared
  initializer or type default, while a field removed from the declaration is
  dropped; and
- a retained global absent from the current program is dropped as an orphan.

Each successful value-shape migration emits `RetainMigrationApplied`; each
dropped orphan emits `RetainOrphanDropped`. These events identify the runtime
transition, not a stable debug-format string. Migration remains transactional:
all entries are validated and staged before any retained target changes, so one
incompatible entry rejects the whole snapshot without exposing earlier staged
values.

**Power-loss guidance:** retained values are only guaranteed to persist if the most recent
snapshot has been flushed to the retain store (i.e., at shutdown or after the save cadence).
Unflushed changes may be lost on sudden power loss (implementer-specific).

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

After successful preparation, the commit replaces the executable and resource
metadata as one cycle-boundary operation, restarts at the program entry point,
applies the prepared retained snapshot, rebinds I/O, and returns the new runtime
metadata. Retained variables follow section 6.7; non-retained variables and
function-block instances follow warm-restart initialization. Logical time is
preserved, the scan counter restarts, and queued writes and forces are cleared
under section 6.9.2. A request must never report `reloaded` unless this complete
commit succeeds.

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
file-read failures. An empty or invalid source set is a startup error.

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
policy unchanged.

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
clears. HMI control failures use a structured error payload with a stable code;
clients do not need to parse diagnostic prose.

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

The version-4 capability registry contains exactly the 15 registered
communication protocols. Build availability, platform availability,
configuration state, and operational evidence are distinct. Every protocol
shown by default on the current platform must have a built backend; an unknown
platform marker fails closed instead of silently removing that requirement.

A communication Test request may succeed at the control-envelope level while
returning `result.ok = false` for a completed negative protocol test.
Unsupported Test operations return `supported = false` and must not appear as
usable controls.

Communication capability `health` and fleet topology runtime, endpoint, and
link status are versioned product-projection vocabularies, not the
connector-report `health` vocabulary in
[Connector Status](23-connector-status.md). Capability health is one of
`not_in_build`, `not_configured`, `simulate`, `runtime_unreachable`,
`connected`, `degraded`, `error`, or `configured_policy`. Consumers must
interpret values within their response type and reject unknown values rather
than treating them as healthy.

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

###### Offline communication operation and fleet-topology truth

Offline schema, apply, and topology require no live runtime and must not invent
live evidence. Offline responses remain schema version 4. ADS authoring writes
coordinated `runtime.toml` and `ads.toml`; OPC UA client authoring writes
coordinated runtime and sidecar state, retains disabled configuration on
disable, and removes the selected section and sidecar on remove. Configured
OpenOT appears as configured evidence without a live payload. Offline results
exclude passwords, tokens, source allowlists, and other secret-bearing fields.

Fleet topology identity and roles come from the selected project and current
runtime evidence. Configuration determines runtime identity, endpoint roles,
counterparts, stable link IDs, and external-node relationships. Merely enabling
a service projects `configured_policy`, not a live connection. Live values and
`connected` status require bound runtime evidence; degraded and faulted evidence
retains its detail. Every link reports stable ID, source, destination, protocol,
role, direction, and evidence-derived status. Fleet output never serializes
MQTT, mesh, ADS, OPC UA, or other credentials.

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

Static setup-page asset checks establish only that the reviewed onboarding
labels and route names are shipped. They do not prove rendered interaction,
authorization, control dispatch, ADS transport, or production readiness.

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

Invariant `RT_ADS_SERVER_DOCTOR_TRUTH_001`: every passing server Doctor step
and every verified evidence field is derived from the running server's live
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
names defined by [Data Types](05-data-types.md). STRING capacity and every
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
  and Engineer for live-device or write-enabled work.
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

#### 6.10 Configuration and Resources

IEC configurations may declare multiple resources. Each resource is scheduled independently in its own OS thread. (IEC 61131-3 Ed.3, §6.8.1; Table 62)

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
