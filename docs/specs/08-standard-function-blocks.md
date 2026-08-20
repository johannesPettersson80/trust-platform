# Standard Function Blocks

IEC 61131-3 Edition 3.0 (2013) - Section 6.6.3.5

This specification defines standard function blocks for trust-hir.

## 1. Overview

Standard function blocks are predefined FBs with internal state. They require instantiation and maintain state between calls.

### FB Index

| Name / Group | Category | Signature Shape | IEC ref | trust-hir | trust-runtime | Deviations |
|--------------|----------|-----------------|---------|-----------|---------------|------------|
| `SR`, `RS` | Bistable | fixed BOOL inputs/outputs | Table 43 | signature only | full stateful behavior | none |
| `R_TRIG`, `F_TRIG` | Edge detection | fixed BOOL inputs/outputs | Table 44 | signature only | full stateful behavior | none |
| `CTU`, `CTD`, `CTUD` | Counters | fixed or overloaded counter types | Table 45 | signature only | full stateful behavior | CTUD LD profile in `docs/IEC_DEVIATIONS.md` |
| `TP`, `TON`, `TOF` and explicit `*_TIME`/`*_LTIME` variants | Timers | fixed TIME/LTIME signatures | Table 46, Figure 15 | signature only | scan-step state machines | scan/lifecycle choices in `docs/IEC_DECISIONS.md`; LD diagnostic `ET` key is a product implementation detail |

### Common Characteristics

- Must be instantiated to use
- Internal variables persist between calls
- Can be overloaded for different data types
- Have standard timing/edge detection behaviors

### Standards Cross-References

- The CTUD LD pin-model omission is recorded in `docs/IEC_DEVIATIONS.md`.
- Timer scan/lifecycle choices are recorded in `docs/IEC_DECISIONS.md`.
- `trust-hir` owns static signatures only; runtime execution owns the stateful behavior described below.

## 2. Bistable Function Blocks (Table 43)

### SR - Set Dominant Bistable

```
     +-----+
     |  SR |
BOOL---|S1 Q1|---BOOL
BOOL---|R   |
     +-----+
```

| Input | Description |
|-------|-------------|
| S1 | Set (dominant) |
| R | Reset |

| Output | Description |
|--------|-------------|
| Q1 | Output state |

**Behavior**:
```
Q1 := S1 OR (NOT R AND Q1)
```

- S1=TRUE always sets Q1=TRUE (set dominant)
- R=TRUE resets Q1=FALSE only if S1=FALSE

**Truth Table**:
| S1 | R | Q1 (next) |
|----|---|-----------|
| 0 | 0 | Q1 (unchanged) |
| 0 | 1 | 0 |
| 1 | 0 | 1 |
| 1 | 1 | 1 (set dominant) |

### RS - Reset Dominant Bistable

```
     +-----+
     |  RS |
BOOL---|S  Q1|---BOOL
BOOL---|R1  |
     +-----+
```

| Input | Description |
|-------|-------------|
| S | Set |
| R1 | Reset (dominant) |

| Output | Description |
|--------|-------------|
| Q1 | Output state |

**Behavior**:
```
Q1 := NOT R1 AND (S OR Q1)
```

- R1=TRUE always resets Q1=FALSE (reset dominant)
- S=TRUE sets Q1=TRUE only if R1=FALSE

**Truth Table**:
| S | R1 | Q1 (next) |
|---|----|-----------|
| 0 | 0 | Q1 (unchanged) |
| 0 | 1 | 0 |
| 1 | 0 | 1 |
| 1 | 1 | 0 (reset dominant) |

### Initial State

The initial state of Q1 is FALSE (default BOOL value).

The runtime stores each bistable's `Q1` independently per function-block
instance. A scan with neither input asserted preserves that stored value. The
dominant input is applied in the same scan as the other input, so `SR(TRUE,
TRUE)` produces `TRUE` and `RS(TRUE, TRUE)` produces `FALSE` without an
intermediate externally visible state.

## 3. Edge Detection (Table 44)

### R_TRIG - Rising Edge Detector

```
     +--------+
     | R_TRIG |
BOOL---|CLK   Q|---BOOL
     +--------+
```

| Input | Description |
|-------|-------------|
| CLK | Clock input |

| Output | Description |
|--------|-------------|
| Q | Edge detected |

**Behavior**:
```
FUNCTION_BLOCK R_TRIG
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR

Q := CLK AND NOT M;
M := CLK;
END_FUNCTION_BLOCK
```

- Q=TRUE for one execution cycle following a FALSE→TRUE transition of CLK
- Q=FALSE at all other times

**Timing**:
```
CLK:  ___/‾‾‾‾‾\____/‾‾‾‾\___
Q:    ___/‾\_______/‾\_______
      (pulse on rising edge)
```

### F_TRIG - Falling Edge Detector

```
     +--------+
     | F_TRIG |
BOOL---|CLK   Q|---BOOL
     +--------+
```

| Input | Description |
|-------|-------------|
| CLK | Clock input |

| Output | Description |
|--------|-------------|
| Q | Edge detected |

**Behavior**:
```
FUNCTION_BLOCK F_TRIG
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR

Q := NOT CLK AND NOT M;
M := NOT CLK;
END_FUNCTION_BLOCK
```

- Q=TRUE for one execution cycle following a TRUE→FALSE transition of CLK
- Q=FALSE at all other times

**Timing**:
```
CLK:  ‾‾‾\____/‾‾‾‾\____/‾‾‾
Q:    ___/‾\_______/‾\_______
      (pulse on falling edge)
```

### Cold Restart Behavior

- R_TRIG with CLK connected to TRUE: Q=TRUE on first execution after cold restart
- F_TRIG with CLK connected to FALSE: Q=TRUE on first execution after cold restart

Each trigger stores its previous sampled phase independently per instance.
Holding `CLK` at one level cannot produce repeated pulses. A pulse is exactly
one executed call wide; calls are the sampling boundary, and no transition is
inferred between calls. The aliases `DIFU` and `DIFD` execute the same state
machines as `R_TRIG` and `F_TRIG`, respectively.

### Edge-qualified input declaration shorthand

IEC 61131-3 Ed.3 Tables 40 and 47 and Annex A `Edge_Decl` permit a
function-block or program input declaration such as:

```iecst
VAR_INPUT
  Start, Reset : BOOL R_EDGE;
  Stop : BOOL F_EDGE;
END_VAR
```

Each name is equivalent to a separate private implicit `R_TRIG` or `F_TRIG`
instance whose `CLK` receives the raw input and whose `Q` is the value visible
to the owning body. The implicit instance uses the same Table 44 state
machine, including the first-execution behavior after cold restart, and cannot
be named or accessed by source. Multiple names in one declaration do not share
phase.

This shorthand is not a type conversion and is not legal on an explicit
`R_TRIG`/`F_TRIG` instance, a non-`BOOL` value, an initialized declaration, a
function/method input, or a section other than function-block/program
`VAR_INPUT`. Function-block methods cannot access the edge-qualified input.
Section-level restart policy is inherited by both the raw stored input and
hidden trigger phase; `CONSTANT` is rejected because the shorthand implies a
function-block instance.

## 4. Counter Function Blocks (Table 45)

### CTU - Up Counter

```
     +-------+
     |  CTU  |
BOOL--->CU  Q|---BOOL
BOOL---|R    |
INT---|PV  CV|---INT
     +-------+
```

| Input | Type | Description |
|-------|------|-------------|
| CU | BOOL (R_EDGE) | Count up (rising edge) |
| R | BOOL | Reset |
| PV | INT | Preset value |

| Output | Type | Description |
|--------|------|-------------|
| Q | BOOL | Counter >= PV |
| CV | INT | Current value |

**Behavior**:
```
IF R THEN
  CV := 0;
ELSIF CU AND (CV < PVmax) THEN
  CV := CV + 1;
END_IF;
Q := (CV >= PV);
```

**Variants**:
- `CTU_INT` - INT counter (default)
- `CTU_DINT` - DINT counter
- `CTU_LINT` - LINT counter
- `CTU_UDINT` - UDINT counter
- `CTU_ULINT` - ULINT counter

### CTD - Down Counter

```
     +-------+
     |  CTD  |
BOOL--->CD  Q|---BOOL
BOOL---|LD   |
INT---|PV  CV|---INT
     +-------+
```

| Input | Type | Description |
|-------|------|-------------|
| CD | BOOL (R_EDGE) | Count down (rising edge) |
| LD | BOOL | Load preset |
| PV | INT | Preset value |

| Output | Type | Description |
|--------|------|-------------|
| Q | BOOL | Counter <= 0 |
| CV | INT | Current value |

**Behavior**:
```
IF LD THEN
  CV := PV;
ELSIF CD AND (CV > PVmin) THEN
  CV := CV - 1;
END_IF;
Q := (CV <= 0);
```

**Variants**: Same as CTU (CTD_DINT, CTD_LINT, etc.)

### CTUD - Up/Down Counter

```
     +---------+
     |  CTUD   |
BOOL--->CU   QU|---BOOL
BOOL--->CD   QD|---BOOL
BOOL---|R      |
BOOL---|LD     |
INT---|PV   CV|---INT
     +---------+
```

| Input | Type | Description |
|-------|------|-------------|
| CU | BOOL (R_EDGE) | Count up (rising edge) |
| CD | BOOL (R_EDGE) | Count down (rising edge) |
| R | BOOL | Reset to 0 |
| LD | BOOL | Load PV |
| PV | INT | Preset value |

| Output | Type | Description |
|--------|------|-------------|
| QU | BOOL | Counter >= PV |
| QD | BOOL | Counter <= 0 |
| CV | INT | Current value |

**Behavior**:
```
IF R THEN
  CV := 0;
ELSIF LD THEN
  CV := PV;
ELSIF NOT (CU AND CD) THEN
  IF CU AND (CV < PVmax) THEN
    CV := CV + 1;
  ELSIF CD AND (CV > PVmin) THEN
    CV := CV - 1;
  END_IF;
END_IF;
QU := (CV >= PV);
QD := (CV <= 0);
```

**Note**: If both CU and CD have rising edges simultaneously, count is unchanged.

**Variants**: Same as CTU (CTUD_DINT, CTUD_LINT, etc.)

### Counter Runtime Conformance Contract

The following requirements bind the runtime implementation of IEC
61131-3 Table 45:

- Count inputs are rising-edge inputs. Holding `CU` or `CD` high across
  repeated calls changes `CV` only on the first call.
- `CTU.R` dominates a `CU` edge. `CTD.LD` dominates a `CD` edge. For `CTUD`,
  `R` dominates `LD`, which dominates both count edges.
- Simultaneous `CTUD` rising edges cancel and leave `CV` unchanged.
- Signed counters saturate at the minimum and maximum of their declared
  integer type; unsigned down counters saturate at zero. No counter wraps.
- `Q`, `QU`, and `QD` are recomputed from the post-transition `CV` on every
  executed call. `CTU.Q` and `CTUD.QU` mean `CV >= PV`; `CTD.Q` and
  `CTUD.QD` mean `CV <= 0` for signed types and `CV = 0` for unsigned types.
- The generic `CTU`, `CTD`, and `CTUD` forms take their concrete `PV`/`CV`
  type from the call. `PV` and existing `CV` must have the same supported
  integer type. A mismatch raises `RuntimeError::TypeMismatch` and does not
  publish new user-visible outputs.
- Edge memory, current value, and outputs are isolated by function-block
  instance. A new instance starts with `CV = 0` and both edge memories false.

## 5. Timer Function Blocks (Table 46, Figure 15)

### Common Timer Interface

IEC 61131-3 Ed.3 section 6.6.3.5.5 and Table 46 define `TP`, `TON`, and `TOF`
as overloads whose `PT` and `ET` duration family is selected consistently as
either TIME or LTIME. IEC also names explicit `*_TIME` and `*_LTIME` forms.

truST accepts the overloaded base names with either duration family, the
explicit `TP_TIME`, `TON_TIME`, and `TOF_TIME` names, and the explicit
`TP_LTIME`, `TON_LTIME`, and `TOF_LTIME` names. An explicit `*_TIME` instance
requires TIME for both `PT` and `ET`; an explicit `*_LTIME` instance requires
LTIME. The TIME and LTIME forms use the same state machine and differ only in
duration type and range.

| Input | Type | Description |
|-------|------|-------------|
| IN | BOOL | Timer input |
| PT | TIME or LTIME | Preset time |

| Output | Type | Description |
|--------|------|-------------|
| Q | BOOL | Timer output |
| ET | TIME or LTIME | Elapsed time |

### Timer Scan-Step Contract

IEC 61131-3 Ed.3 section 6.6.3.5.5 requires the timer behaviors shown in
Table 46 and Figure 15. truST observes those diagrams at executed function
block calls: inputs are sampled, elapsed time is applied, and `Q`/`ET` become
visible as one scan step. A timer has no background transition between calls,
and this contract makes no continuous-time claim.

The implementation-owned timer boundaries are reviewed in
`docs/IEC_DECISIONS.md`. In particular, the current `PT` is sampled on each
executed call while timing is active, non-positive `PT` is treated as zero,
and TIME/LTIME variants share the same state transitions. Restart, clock-step,
conditional-call, and TP retrigger decisions are specified there but are not
asserted by the first timer trace vertical.

### TP - Pulse Timer

```
     +-------+
     |  TP   |
BOOL---|IN  Q|---BOOL
TIME---|PT ET|---TIME
     +-------+
```

**Behavior**: Generates a fixed-duration pulse.

- When IN goes TRUE, Q goes TRUE for duration PT
- Q stays TRUE for full PT duration regardless of IN changes
- ET counts up while Q is TRUE, stops at PT
- After the pulse expires while an executed call still samples IN as TRUE, Q
  remains FALSE and ET remains at PT. The first later executed call sampling IN
  as FALSE resets ET to zero and rearms edge detection. A subsequent executed
  FALSE-to-TRUE call starts a new pulse with Q TRUE and ET advanced by that
  call's elapsed-time contribution.

**Timing Diagram**:
```
IN:  __/‾‾‾‾\____/‾\_______/‾‾‾‾‾‾‾‾‾\_____
Q:   __/‾‾‾‾‾‾‾‾\_/‾‾‾‾‾‾‾‾\_/‾‾‾‾‾‾‾‾\____
ET:  __/‾‾‾‾‾‾‾‾\_/‾‾‾‾‾‾‾‾\_/‾‾‾‾‾‾‾‾\____
        |<--PT-->| |<--PT-->| |<--PT-->|
```

### TON - On-Delay Timer

```
     +-------+
     |  TON  |
BOOL---|IN  Q|---BOOL
TIME---|PT ET|---TIME
     +-------+
```

**Behavior**: Delays turning on.

- Q goes TRUE after IN has been TRUE for duration PT
- If IN goes FALSE before PT, Q stays FALSE and ET resets
- ET counts while IN is TRUE and Q is FALSE

**Timing Diagram**:
```
IN:  __/‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾\_____/‾‾‾\_____
Q:   _______/‾‾‾‾‾‾‾‾‾‾\_____________
ET:  __/‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾\____/‾‾‾\_____
        |<PT>|              |<PT (not reached)
```

**Use Case**: Debounce, delayed start

### TOF - Off-Delay Timer

```
     +-------+
     |  TOF  |
BOOL---|IN  Q|---BOOL
TIME---|PT ET|---TIME
     +-------+
```

**Behavior**: Delays turning off.

- An executed call with IN TRUE sets Q TRUE, resets ET to zero, and rearms the
  off-delay.
- On a sampled TRUE-to-FALSE transition, Q remains TRUE and ET advances toward
  PT on executed calls.
- The first executed call whose accumulated elapsed time reaches PT sets Q
  FALSE and ET to PT.
- Later executed calls with IN still FALSE keep Q FALSE and ET at PT.
- The next executed call with IN TRUE sets Q TRUE, resets ET to zero, and
  rearms the off-delay.

#### TOF Scan-Step State Machine

The post-expiry `ET = PT` plateau is required by IEC Figure 15(c). It persists
through the remaining low-input interval and ends only when a later executed
call samples IN TRUE.

**Timing Diagram**:
```
IN:  __/------\______________/--\____________
Q:   __/-------------\_______/---------\______
ET:  0        /-----PT========0  /---PT========
                    hold             hold
```

**Use Case**: Keep motor running after button release, extend output

### Timer Variants

The overloaded standard names accept either a consistent TIME or LTIME
interface:

- `TP`
- `TON`
- `TOF`

Explicit TIME names are also supported:

- `TP_TIME`
- `TON_TIME`
- `TOF_TIME`

Explicit LTIME names are supported:

- `TP_LTIME`
- `TON_LTIME`
- `TOF_LTIME`

Each explicit variant follows the corresponding Figure 15 scan-step state
machine above and requires `PT` and `ET` to use its named TIME or LTIME family.

### Timer Runtime Boundary Contract

- Timer state is isolated per instance. A newly created timer has `Q = FALSE`,
  `ET = 0`, no active interval, and a first-call elapsed contribution of zero.
- `PT <= 0` is normalized to zero. A zero-preset `TON` completes on an
  executed call with `IN = TRUE`; a zero-preset `TP` has no positive-duration
  pulse; and a zero-preset `TOF` expires on the falling-edge call.
- Elapsed contributions are non-negative. A direct negative delta, a stationary
  runtime clock, or a backward runtime clock contributes zero and establishes
  the new baseline without reducing `ET`.
- Elapsed accumulation saturates at the current non-negative `PT` and never
  wraps. If the current `PT` decreases below accumulated `ET`, the same call
  completes at the new `PT`. If active `TON.PT` increases, `Q` is recomputed
  from the increased threshold.
- A `TP` falling edge does not cancel an active pulse. A later rising edge
  restarts the interval only after an executed call has sampled the intervening
  low level.
- Once `TOF` expires, `ET` holds the `PT` sampled at expiry while `IN` remains
  low, even if later low-state calls provide another `PT`. A high-state call
  rearms the instance and resets `ET`.
- `PT` determines the output family: `TIME` publishes `TIME ET`, and `LTIME`
  publishes `LTIME ET`. An incompatible `IN`, `PT`, or existing `ET` value
  raises `RuntimeError::TypeMismatch` before new user-visible outputs are
  published.

### Built-in Registry Contract

Runtime built-in lookup is ASCII case-insensitive. It recognizes the IEC names,
the counter and timer width suffixes documented above, and `DIFU`/`DIFD`.
`standard_function_blocks()` publishes one unique static definition for each
accepted public spelling. Parameter order, names, directions, and concrete
types in those definitions are the call-binding contract; timer `PT` and `ET`
always use the same duration family, and counter `PV` and `CV` always use the
same integer family.

## 6. Usage Examples

### Bistable Example

```
VAR
  StartButton: BOOL;
  StopButton: BOOL;
  MotorRunning: SR;
END_VAR

MotorRunning(S1 := StartButton, R := StopButton);
MotorOutput := MotorRunning.Q1;
```

### Edge Detection Example

```
VAR
  Sensor: BOOL;
  SensorEdge: R_TRIG;
  Count: INT := 0;
END_VAR

SensorEdge(CLK := Sensor);
IF SensorEdge.Q THEN
  Count := Count + 1;
END_IF;
```

### Counter Example

```
VAR
  PulseInput: BOOL;
  ResetButton: BOOL;
  Counter: CTU;
END_VAR

Counter(CU := PulseInput, R := ResetButton, PV := 100);
IF Counter.Q THEN
  // Counter reached 100
  Alarm := TRUE;
END_IF;
CurrentCount := Counter.CV;
```

### Timer Example

```
VAR
  StartCommand: BOOL;
  DelayTimer: TON;
  MotorOn: BOOL;
END_VAR

DelayTimer(IN := StartCommand, PT := T#5s);
MotorOn := DelayTimer.Q;  // Motor starts 5 seconds after command
```

### Combined Example

```
VAR
  Button: BOOL;
  ButtonEdge: R_TRIG;
  PulseTimer: TP;
  Output: BOOL;
END_VAR

// Generate 500ms pulse on each button press
ButtonEdge(CLK := Button);
PulseTimer(IN := ButtonEdge.Q, PT := T#500ms);
Output := PulseTimer.Q;
```

## 7. Timing Considerations

### Execution Rate

Timer accuracy depends on execution rate:
- Timer resolution = execution cycle time
- For T#10ms timer with 100ms cycle: actual time ≈ 100ms

### Edge Detection Accuracy

- Edge is detected between consecutive executions
- Multiple edges within one cycle appear as one edge

### Counter Overflow

- PVmax and PVmin are Implementer specific
- Typically max value of the counter type (e.g., 32767 for INT)
- Counter saturates at limits

## Implementation Notes for trust-hir

trust-hir validates standard FB calls by signature and static types only; it
does not model internal state or timing behavior. The stateful behavior is
owned by `trust-runtime`. (IEC 61131-3 Ed.3, Section 6.6.3.5, Tables 43-46,
Figure 15.)

The behavioral descriptions above are retained for reference; `trust-runtime`
executes the stateful timer/counter/trigger behavior for `SR`, `RS`, `R_TRIG`,
`F_TRIG`, `CTU`, `CTD`, `CTUD`, `TP`, `TON`, and `TOF`. The CTUD single-input
LD profile is documented in `docs/IEC_DEVIATIONS.md`; the TP/TOF diagnostic
`ET` key is an internal product representation rather than an IEC deviation.

### FB Definitions

trust-hir provides built-in signatures for:
1. Input variables with their declared types
2. Output variables with their declared types

Internal state variables and behavioral specifications are documented above but
are not modeled in trust-hir. Runtime execution covers that stateful behavior;
see the runtime timer/counter implementations and the CTUD LD profile deviation.

### Edge Detection Internal

R_EDGE and F_EDGE input qualifiers:
```
VAR_INPUT
  CU: BOOL R_EDGE;  // Rising edge detection
END_VAR
```

Internally equivalent to:
```
VAR_INPUT
  CU: BOOL;
END_VAR
VAR
  // Private compiler-generated state, not a source-visible declaration:
  CU_EDGE: R_TRIG;
END_VAR
// Before the body: CU_EDGE(CLK := raw CU)
// In the body: reading CU yields CU_EDGE.Q
```

The illustrative `CU_EDGE` name is not inserted into the source symbol table.
The hidden identity is collision-free, and every declared input name receives
its own state.

### Timer Implementation

Timers require:
1. Time tracking (ET accumulation)
2. State machine for IN/Q relationship
3. Comparison with PT

### Standard Library

trust-hir should provide built-in definitions for:
- SR, RS
- R_TRIG, F_TRIG
- CTU, CTD, CTUD (and typed variants)
- TP, TON, TOF (and LTIME variants)
