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

Process-image drivers must keep scan-cycle methods bounded. Drivers with
blocking wire protocols may own background workers, but the `IoDriver` boundary
remains the cycle handoff: `read_inputs` copies the latest worker snapshot or
returns the configured policy result, and `write_outputs` hands off the latest
desired output without waiting for protocol round trips. Worker health is
projected through the existing `IoDriverHealth` surface rather than a parallel
status model. Output handoff is level/latest-value semantics for `%Q`, not an
edge or pulse delivery guarantee.

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

The runtime's OPC UA server publishes PLC snapshot variables with CurrentRead
access only. The periodic publisher refreshes those values from runtime
snapshots; client writes are rejected because no transactional write-back path
to PLC storage is defined. A future writable server surface requires an
explicit allowlist, authorization, type validation, scan-boundary application,
and visible write result before it may advertise CurrentWrite.

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
- Runtime exchange is worker-backed: broker connect/poll/publish and
  reconnection happen on the MQTT worker, while scan-cycle reads/writes use
  bounded snapshot/handoff state.
- Under the default `fault` policy, disconnected or stale MQTT input reads and
  bounded reads with no available snapshot return `IoFreshness`. Completed
  connection failures retain their connection context at the worker boundary;
  they are not replaced by a generic unavailable-snapshot message.
- Reconnection is non-blocking; runtime cycle remains deterministic.
- Security baseline rejects insecure remote brokers unless explicitly overridden.
- Sparkplug B non-goals in this profile: command subscriptions, device-level
  DBIRTH/DDATA topics, metric aliases, templates, and store-and-forward.

3. **EtherCAT (backend v1)**
- Driver name: `ethercat`.
- Deterministic process-image mapping for module-chain profiles (including
  `EK1100` + digital I/O modules such as `EL1008` / `EL2008`).
- Startup discovery diagnostics emit discovered module summary and expected
  process-image sizes.
- Cycle-time health telemetry upgrades driver status to **degraded** when cycle
  read/write exceeds configured warning threshold.
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
- This rule is limited to OPC UA client scalar ingress. OPC UA egress, arrays,
  structures, subnormal values, signed zero, and other protocol/API/retained
  ingress boundaries require their own reviewed contracts.

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
- The runtime does not clamp, normalize, or substitute a default value. ADS
  egress, subnormal values, and signed zero are outside this rule.

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

Ordinary handler failures exit with status 1. When a parsed command enables
`--ci`, the entrypoint uses the documented CI error classification, including
command-aware fallback codes. Known configuration and permission failures may
append one actionable tip to the original error text; the tip does not replace
or reclassify the underlying error.

##### Runtime launcher input, validation, and startup contract

`trust-runtime play --project <path>` is the product convenience entry point:
it creates the default project when the selected folder is missing or lacks the
minimum generated runtime artifacts. All non-filesystem launch arguments,
including restart mode and simulation time scale, must be validated before that
creation or any other project mutation. A rejected option therefore leaves a
previously missing project path missing. `trust-runtime run` consumes an
existing project and does not provide this creation behavior.

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

Simulation startup rejects a CLI or project time scale of zero. `--simulation`
enables simulation without discarding the project's simulation model; a CLI
time scale greater than one enables simulation and overrides the project time
scale. Otherwise the loaded project simulation configuration remains
authoritative. The settings exposed to control and status surfaces must reflect
the effective runtime configuration and this effective simulation/backend
selection, rather than an independent set of launcher defaults.

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
failure. A response with `ok = false`, or without a Boolean `ok`, is a command
failure and must produce a non-zero process exit rather than being printed as
a successful scripted result. Human-oriented status, health, and task-summary
rendering happens only after `ok = true`.

##### 6.8.1a ADS CLI authoring and server-control contract

`trust-runtime ads` is the scriptable ADS authoring, validation, route, and
server-control surface. Offline import canonicalizes cached symbol snapshots,
generates deterministic ST, creates missing parent directories, reports an
unchanged byte-identical output without rewriting it, and refuses to replace
different generated content unless `--force` is explicit. Offline validation
requires a snapshot and fails visibly for missing symbols, type mismatches, or
generated-source drift. Live validation rejects cached snapshots and obtains
fresh symbols from every configured connection before applying the same
compatibility check.

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

##### 6.8.2 Versioned deployment and rollback

`trust-runtime deploy` validates the source bundle before installation and the
copied bundle before changing deployment pointers. A custom deployment label
must be one non-empty filesystem component: absolute paths, `.`/`..`, path
separators, and platform prefixes are rejected. Automatically generated labels
must be unique across consecutive deployments from one process, including
deployments started within the same second.

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

After a successful deployment, `current` identifies the newly validated bundle
and `previous` identifies the bundle that was current immediately before the
deployment. Pruning retains exactly those existing bundle directories and
removes older directories only from the deployment root's own `bundles/`
directory. It must not delete the new rollback target. `rollback` swaps the two
valid pointers without copying or rebuilding either bundle.

Before deployment pointers change, the command writes a deterministic change
summary that identifies both the bundle being deployed and the previous bundle,
when one exists. The `runtime.toml` and `io.toml` status is based on the complete
file content, so a change outside the selected operator-friendly detail bullets
must still be reported as an update rather than as `unchanged`. Safe-state
comparison uses the structured I/O address and value, not a potentially lossy
display form. Source comparison recursively covers `.st` and `.pou` files
case-insensitively beneath `src/`, with stable relative-path ordering. A source
directory traversal or file-read failure aborts summary construction and the
deployment; it is not converted into an empty source set.

`deployments/<label>.txt` and `deployments/last.txt` contain the last successful
deploy command's change summary. They are not deployment pointers and rollback
does not rewrite them. The `current` symlink is the authoritative active-bundle
identifier before and after rollback.

##### 6.8.3 Managed local fleet scaffolding

`trust-runtime fleet runtime add` creates one managed local runtime project
beneath the selected fleet root and registers it in `fleet.toml`. Runtime names
use the CLI's portable name alphabet and must be unique in a manifest; hand-edited
manifests are subject to the same validation before list or lifecycle actions.
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
rather than loading the complete log file. A start operation writes lifecycle
state only after spawning the process; if PID persistence fails, it terminates
and reaps that child, and if the child exits before the control endpoint becomes
ready, start fails and removes the stale PID file.

Control endpoints are local by default (`unix://` on Unix-like platforms) and the Unix socket is
created with restrictive permissions (0600) to prevent accidental exposure.

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

##### 6.9.4 Runtime control authorization

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
- **Remote access** supports manual add and invite/QR pairing only.
- **Data sharing** is explicit (publish/subscribe mapping only).
- Inbound mesh values are decoded against the current local target type. Integer
  values outside that IEC type's range are rejected rather than wrapped, and a
  JSON number targeting `REAL`/`LREAL` is accepted only when the resulting
  runtime value is finite.
- A rejected mesh value queues no local update and leaves the current PLC value
  unchanged. The runtime does not clamp, wrap, normalize, or substitute a
  default value.
- TOML remains the source of truth; offline edits are supported.

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
