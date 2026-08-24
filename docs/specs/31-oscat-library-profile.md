# OSCAT Library Product Profile

This document is the normative truST product contract for the OSCAT packages
shipped under `libraries/oscat`. OSCAT is a third-party library profile built on
Structured Text; it is not part of IEC 61131-3. The IEC standard governs the
language used to express the library, while this document governs the selected
OSCAT API, behavior, compatibility adaptations, and object facade.

The detailed public signatures and function summaries in
[`OSCAT_LIBRARY_GUIDE.md`](../guides/OSCAT_LIBRARY_GUIDE.md) and
[`OSCAT_OOP_LIBRARY_GUIDE.md`](../guides/OSCAT_OOP_LIBRARY_GUIDE.md) are
incorporated into this contract. If a guide and this profile disagree, this
profile owns the product rule and the guide must be corrected.

## 1. Scope and authority

The classic package SHALL publish the manual-aligned chapter set from
`03_data_types` through `26_list_processing`. The OOP package SHALL remain a
facade over selected classic functions and function blocks; it SHALL NOT create
a second implementation of their mathematical or scan-state semantics.

The conformance oracle is the observable public behavior described here and in
the incorporated guide tables:

- exact public types, globals, functions, function blocks, methods, and
  properties;
- exact results for the documented valid inputs;
- documented clamping, bounds, reset, edge, scan, and state-transition rules;
- explicit absence or rename of unsupported/conflicting public names.

Private test seed/probe helpers, fixture harness behavior, and tautological
assertions are not product oracles.

## 2. Classic package contract

### 2.1 Initialization, carriers, and public naming

`OSCAT_BASIC_Constants()` SHALL initialize the `MATH`, `PHYS`, and `LANGUAGE`
global carriers on its first successful call, return `TRUE`, and preserve the
initialized values on later calls. Consumers SHALL initialize these carriers
before reading them or invoking helpers that depend on them.

The Chapter 3 carrier types and Chapter 4 globals and helpers SHALL expose the
fields, values, and classifications documented in the classic guide.
`T_PLC_MS()` SHALL use the runtime millisecond time bridge and `T_PLC_US()`
SHALL remain the documented millisecond-derived compatibility projection rather
than claiming a hardware microsecond clock.

The following truST product adaptations are mandatory:

- `CALENDAR.LOCAL_DT`, `LOCAL_DATE`, and `LOCAL_TOD` replace upstream names
  that collide with reserved date/time keywords;
- `SEQUENCE_4.STATE` and `SEQUENCE_8.STATE` replace the upstream `STEP` field;
- the upstream Chapter 19 function `OVERRIDE` is published as `OVERRIDE_3`;
- buffer helpers use typed `ARRAY[*] OF BYTE` ports and the documented typed
  pointer surface.

The replaced upstream names SHALL remain absent. These are OSCAT/truST package
choices, not IEC deviations.

### 2.2 Numeric, array, complex, double, geometry, and vector behavior

The Chapters 5 through 11 public functions SHALL implement the input/output,
clamping, mutation, ordering, approximation, and conversion behavior stated in
the classic guide tables.

Array procedures that are documented as in-place operations SHALL mutate only
the addressed elements. Numeric helpers SHALL preserve their declared result
type and documented domain handling. Complex, double-precision, geometry, and
vector helpers SHALL return the documented component-wise or scalar result.

Random helpers SHALL remain callable with the documented types, but a test
proves their behavior only when it asserts a non-tautological deterministic
property such as a bound, state transition, or reproducible relation.

OSCAT bit-transfer wrappers follow the truST explicit-conversion policy:
finite IEEE `REAL` patterns round-trip, while `DWORD_TO_REAL` rejects patterns
for NaN or either infinity with overflow. Consequently, `CHK_REAL` can observe
the normal finite classification through supported truST program execution;
its upstream non-finite classification branches remain source-compatible but
are not executable conformance oracles.

### 2.3 Time, calendar, and string behavior

The Chapters 12 and 13 public functions and function blocks SHALL implement the
documented:

- `TIME`, date, time-of-day, and date-time conversions;
- calendar component, leap-year, work-week, holiday, sunrise, and sunset rules;
- string search, extraction, editing, comparison, conversion, checksum,
  formatting, and message behavior.

Calendar operations SHALL use the documented Gregorian and ISO calendar rules.
String operations SHALL honor the declared bounds and returned status/count
contract rather than silently extending or truncating outside that contract.

### 2.4 Memory, pulse, logic, generator, and signal-processing behavior

The Chapters 14 through 19 function blocks SHALL preserve state between scans
per instance. Reset, load, clock, enable, and edge inputs SHALL change only the
documented state. Separate instances SHALL NOT share retained scan state.

Pulse, ramp, generator, filter, controller, and signal-processing blocks SHALL
follow the documented timing and recurrence rules. Chapter 19 SHALL expose the
renamed `OVERRIDE_3` surface and SHALL NOT expose the conflicting upstream name.

### 2.5 Sensor, measuring, calculation, control, and device behavior

The Chapters 20 through 24 public functions and function blocks SHALL implement
the conversion, calibration, threshold, alarm, calculation, controller, and
driver behavior stated in the classic guide. Stateful blocks SHALL retain only
their own instance state and SHALL honor the documented initialization, reset,
limit, and alarm transitions.

### 2.6 Buffer and list behavior

The Chapters 25 and 26 public helpers SHALL operate on caller-owned typed
buffers and list strings through their declared `VAR_IN_OUT` or typed pointer
ports. Reads and writes SHALL remain within the declared logical bounds.
FIFO/stack/list operations SHALL preserve documented ordering, count, empty,
full, and error behavior without mutating unrelated caller storage.

## 3. OOP facade contract

### 3.1 Context, conversion, and calendar parity

The OOP context and service objects SHALL call or reproduce the corresponding
classic initialization, carrier, direction, conversion, calendar, and sun
helper behavior. Public read-only properties SHALL report the current wrapped
state and SHALL NOT become a second mutable source of truth.

### 3.2 Stateful scan lifecycle and isolation

Each stateful OOP component SHALL own one classic stateful instance. Its
configuration/initialization method SHALL establish the documented initial
state, its update method SHALL advance exactly one scan, and its reset surface
SHALL restore the documented reset state.

Two separately declared OOP objects SHALL NOT share scan history. Invalid
configuration SHALL be rejected or projected through the documented status
without partially changing unrelated component state.

### 3.3 Component parity

The shipped OOP memory, logic, pulse, generator, filter, controller, measuring,
calendar, building, and device components SHALL match the corresponding classic
result and state transition for the same inputs and scan sequence.

Interface dispatch SHALL preserve the same result as a direct concrete call.
Unsupported behavior SHALL remain absent or fail closed as documented; the OOP
facade SHALL NOT manufacture success for a classic operation that is not
available.

### 3.4 Shipped OOP example projects

The paired `examples/OSCAT/<example>/{non-oop,oop}` corpus SHALL retain the
reviewed grouped layout and the documented design-pattern structures claimed
by the OOP guide. The airport-baggage namespace aggregate fixture SHALL compile
and execute its reviewed aggregate trigger successfully through the real
runtime runner.

The grouped-layout and pattern assertions prove only the tracked product
artifact structure. The executable aggregate assertion proves only the named
fixture result. Neither form establishes unreviewed OSCAT behavior, arbitrary
project architecture, or semantic parity for examples that are not executed.

## 4. Verification partitions

The authoritative conformance partitions are:

1. classic initialization, carriers, Chapter 3-4 types/helpers, and public
   naming adaptations;
2. classic Chapters 5-11 numeric and geometric helpers;
3. classic Chapters 12-13 time, calendar, and string helpers;
4. classic Chapters 14-19 stateful memory, logic, generator, and signal
   processing;
5. classic Chapters 20-24 sensor, measuring, calculation, control, and device
   behavior;
6. classic Chapters 25-26 buffer and list behavior;
7. OOP context/conversion/calendar parity;
8. OOP stateful scan lifecycle, reset, and instance isolation;
9. OOP component and interface parity; and
10. shipped OOP example layout, pattern structure, and named executable
    aggregate result.

An executable Structured Text test can prove a partition only when its
assertions observe the specified public result or transition. Compilation,
fixture plumbing, private seed/probe access, and an always-true comparison do
not establish product conformance.
