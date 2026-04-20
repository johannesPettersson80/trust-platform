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
| `TP`, `TON`, `TOF` and `*_LTIME` variants | Timers | fixed TIME/LTIME signatures | Table 46 | signature only | full stateful behavior | TP/TOF `ET` exposure in `docs/IEC_DEVIATIONS.md` |

### Common Characteristics

- Must be instantiated to use
- Internal variables persist between calls
- Can be overloaded for different data types
- Have standard timing/edge detection behaviors

### Deviation Cross-References

- Counter/timer runtime deviations are recorded in `docs/IEC_DEVIATIONS.md`.
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

## 5. Timer Function Blocks (Table 46, Figure 15)

### Common Timer Interface

All timers share:

| Input | Type | Description |
|-------|------|-------------|
| IN | BOOL | Timer input |
| PT | TIME | Preset time |

| Output | Type | Description |
|--------|------|-------------|
| Q | BOOL | Timer output |
| ET | TIME | Elapsed time |

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

- Q goes TRUE immediately when IN goes TRUE
- When IN goes FALSE, Q stays TRUE for duration PT
- If IN goes TRUE again before PT, Q stays TRUE and ET resets

**Timing Diagram**:
```
IN:  __/‾‾‾‾‾‾\___________/‾\___________
Q:   __/‾‾‾‾‾‾‾‾‾‾‾‾‾\____/‾‾‾‾‾‾‾‾‾\___
ET:  _________/‾‾‾‾‾‾\________/‾‾‾‾‾‾\___
              |<-PT-->|       |<-PT-->|
```

**Use Case**: Keep motor running after button release, extend output

### Timer Variants

Standard timers use TIME. Variants for LTIME:
- `TP_LTIME`
- `TON_LTIME`
- `TOF_LTIME`

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
does not model internal state or timing behavior. (IEC 61131-3 Ed.3,
Section 6.6.3.5, Tables 43-46, Figure 15; DEV-010)

The behavioral descriptions above are retained for reference; `trust-runtime`
executes the stateful timer/counter/trigger behavior for `SR`, `RS`, `R_TRIG`,
`F_TRIG`, `CTU`, `CTD`, `CTUD`, `TP`, `TON`, and `TOF`. Current runtime
deviations are documented in `docs/IEC_DEVIATIONS.md`, including the CTUD
single-input LD profile and TP/TOF ET exposure.

### FB Definitions

trust-hir provides built-in signatures for:
1. Input variables with types and edge qualifiers
2. Output variables with types

Internal state variables and behavioral specifications are documented above but
are not modeled in trust-hir. Runtime execution covers that stateful behavior;
see `docs/IEC_DEVIATIONS.md` and the runtime timer/counter implementations.

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
  CU_EDGE: R_TRIG;
END_VAR
// In body: use CU_EDGE(CLK := CU).Q instead of CU
```

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
