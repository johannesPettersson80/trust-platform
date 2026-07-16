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
- Inputs are latched at the start of each scheduler cycle; outputs are committed after all ready tasks complete.
- The maximum number of tasks per resource and minimum interval resolution are implementer-specific and are reported by the runtime configuration.
- The resource loop maintains a `RUNNING/FAULT/STOPPED` state and halts on faults.

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
(DEV-044)

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
state has a stronger success boundary (DEV-046):

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

##### Floating-point boundary admission policy

IEC 61131-3 Ed.3, Section 6.4.2.1, Table 10 defines `REAL` and `LREAL`
using the IEC 60559 basic single- and double-width formats and leaves results
involving infinity or not-a-number implementer-specific. Section 6.6.2.5.15,
Table 39 defines `IS_VALID` so program logic can distinguish finite values from
NaN and infinity. Those provisions do not define how an external host or
protocol admits a value into a PLC process image. truST therefore applies the
following fail-closed admission contract (DEV-045):

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
vocabularies (DEV-051):

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
faulted rather than healthy (DEV-052).

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
- Timeout thresholds and fault action are configured per resource (see §6.9) and are
  **implementer-specific** in IEC 61131-3 (recorded in `docs/IEC_DEVIATIONS.md`).
- Default action is **safe_halt**: outputs are set to configured safe values (if provided),
  then the resource halts. For **halt** and **safe_halt**, safe-state outputs are applied
  before halting. If no safe-state output values are configured, fault handling performs
  no physical output write; it must not re-send the pending process-image output as a
  substitute safe state.

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

These pause rules are a truST host/debugger extension outside the IEC language
execution model (DEV-049).

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

#### 6.7 Retain Storage (IEC 61131-3 §6.5.6)

Retentive variables must follow IEC 61131-3 retentive variable rules (§6.5.6, Figure 9). At
startup:

- **Warm restart**: RETAIN variables restore their retained values; NON_RETAIN are initialized.
- **Cold restart**: RETAIN and NON_RETAIN variables are initialized.
- Unqualified variables follow the runtime's retain policy (see `docs/IEC_DECISIONS.md`).
- `VAR_STAT` follows the documented vendor-extension storage rules from `docs/IEC_DEVIATIONS.md`:
  function statics persist across calls, method statics persist per instance and per method, and
  `PROGRAM`/`FUNCTION_BLOCK`/`CLASS` `VAR_STAT` uses ordinary instance storage.

Retain storage is provided via a pluggable backend:

```rust
pub trait RetainStore: Send {
    fn load(&self) -> Result<RetainImage, RuntimeError>;
    fn store(&self, image: &RetainImage) -> Result<(), RuntimeError>;
}
```

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
transport or transaction boundary (DEV-024). A reload request is applied only
at a completed scan boundary. The runtime must prepare every fallible input
needed by the change before replacing live execution state:

- decode and validate the complete bytecode container and materialize its VM
  module;
- validate the selected resource, tasks, and process-image sizes; and
- read, decode, canonicalize, and validate the retained snapshot required by
  the warm-reload policy.

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

This behavior is implementer-specific; IEC 61131-3 does not define
hardware driver selection or OS-level IO configuration (see `docs/IEC_DEVIATIONS.md`).

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
the separate runtime control endpoint contract. (DEV-040)

##### 6.9.2 Debug mutation lifecycle

Debug writes are one-shot requests. An accepted write is applied at the next
scan boundary and is then consumed. A force is applied at scan boundaries
until the matching release request or a clearing lifecycle boundary. Release
removes the force without writing a replacement value; normal program and I/O
evaluation determine the value at the next scan. (DEV-047)

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
construction or source-value admission. (DEV-041)

##### 6.9.4 Runtime control authorization

Runtime control uses the ordered role hierarchy `viewer < operator < engineer <
admin`; a higher role includes lower-role permissions. Authorization is checked
before dispatch, so a denied request must not change runtime, debug, I/O, HMI,
configuration, pairing, or connector state. (DEV-050)

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
