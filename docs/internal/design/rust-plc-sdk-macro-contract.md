# Rust PLC SDK And Macro Contract

**Status:** implementation contract, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md`.
**Applies to:** `trust-plc`, `trust-module-abi`, proc macros, sequence
validation, rust-analyzer-facing SDK surface.

If this file disagrees with the master SDK shape, the master wins. The v1.1
shape is reconciled with master section 6.1: retain fields are declared on
the user struct with `#[retain]` and reached through `Exchange<Self>` during
cycle calls, not through a separate public `Retain` associated type.

## 1. Purpose

The SDK must make scan-safe PLC authoring feel like normal Rust while keeping
the runtime contract explicit. User modules are safe Rust. Any required
unsafety lives inside the SDK, ABI crate, or module host and is registered as
TCB code.

## 2. Crate Boundaries

| Crate | Owns | Must not depend on |
|---|---|---|
| `trust-module-abi` | `repr(C)` descriptors, vtables, status enums, fixed ABI structs | runtime host, HIR, LSP, VS Code, web |
| `trust-plc` | user-facing traits, attributes, exchange types, sequence API, TestBench facade | runtime host internals, VS Code, web |
| `trust-plc-macros` if split | proc macros and validators | runtime host internals |
| `trust-sim` | test harness facade for machine tests | VS Code, web |

The ABI crate is the only shared dependency between host and modules. The SDK
may re-export ABI types only when that does not leak host implementation.

## 3. Safe User Code Rule

- User PLC code must compile with no `unsafe` requirement.
- SDK-generated code may contain unsafe only in generated glue modules with
  reviewed safety comments.
- The SDK must make retained pointers, process-image references across waits,
  raw host pointers, and host-owned storage escape structurally impossible.
- Runtime docs must distinguish Rust memory safety from PLC safety. Safe Rust
  is not a certification claim.

## 4. Core Traits

Minimum public traits:

```rust
pub trait PlcProgram {
    fn init(&mut self, ctx: &mut InitCtx) -> PlcResult<()>;

    fn cycle(
        &mut self,
        io: &mut Exchange<Self>,
        ctx: &mut CycleCtx,
    ) -> PlcResult<()>;
}
```

`PlcFb` follows the same ownership rule: runtime-owned exchange data is
passed for the call and cannot be retained by user code. `#[trust_function]`
exports are instance-less and stateless by structure; no separate public
`PlcFunction` trait is locked for v1 unless the master adds one.

## 5. Attributes

Required attributes:

- `#[trust_program(name = "...")]`
- `#[trust_fb(name = "...")]`
- `#[trust_function(name = "...")]`
- `#[trust_sequence]`
- `#[input]`
- `#[output]`
- `#[retain]`
- `#[safe_default(...)]`
- `#[budget("...")]`
- `#[unit("...")]`
- `#[range(min = ..., max = ...)]`
- `#[trust_device(connection = "...")]`

Every attribute must produce diagnostics with the user's source span and a
stable diagnostic code when the construct is invalid.

## 6. `#[trust_program]`

The macro validates and emits:

- exported POU metadata;
- exchange schema;
- retain schema;
- budget metadata;
- safe-default metadata;
- source-location map for generated ST and diagnostics;
- ABI descriptor glue;
- compile-time errors for unsupported IEC/Rust type mappings.

The macro must not require the runtime crate. It may emit descriptors consumed
by codegen or the module packager.

## 7. `#[trust_fb]` And `#[trust_function]`

`#[trust_fb]` supports brownfield module declaration and greenfield generated
ST declarations. `#[trust_function]` is stateless by structure and must reject
retained state, interior host references, implicit storage, or private fields
that would require instance memory.

## 8. `#[trust_sequence]`

A sequence is a restricted, macro-validated source language using Rust await
syntax. It is not general Rust async.

Allowed awaits:

- `s.next_cycle().await`
- `s.wait(duration).await`
- `s.wait_until(predicate).await`
- `s.wait_until(predicate).deadline(duration, fault).await`
- `s.all(...).await`
- `s.race(...).await`
- child trust sequence await

Admitted macros inside a sequence body:

- `iec!(...)` for IEC duration/time literals used as wait/deadline arguments.

Every other macro is rejected unless a later contract explicitly adds it to
the allowlist with positive and negative validator fixtures.

Rejected with F23:

- foreign `.await`;
- async blocks;
- async closures;
- executor APIs;
- hand-built futures;
- boxed futures;
- macros outside the allowlist;
- loops without an admitted wait point.

Unknown macros are rejected because they can hide `.await`.

## 9. Closure-scoped I/O And Retain Access

The sequence API exposes only closure-scoped access:

```rust
s.read(|io| io.start);
s.write(|io| io.motor = true);
s.retain_read(|r| r.count);
s.retain_write(|r| r.count += 1);
```

The SDK must not expose `io_mut()` or retain guards that can be stored across
wait points. Predicates passed to wait points must be synchronous and cannot
return futures.

## 10. Diagnostic Quality

Macro diagnostics must:

- name the invalid construct;
- point at the user source span;
- include the accepted alternative where useful;
- avoid claiming proof stronger than the evidence grade;
- be stable enough for UI tests.

Examples:

- `trust(F23): foreign await is not admitted in a cycle sequence`
- `trust(RS-14): field type Vec<u8> cannot be mapped to IEC exchange storage`
- `trust(RS-71): output without safe_default uses typed initial value`

## 10.1 Fault-code namespace

SDK-generated `_FAULT_CODE` values follow the master namespace:

- `0`: no fault;
- `1..=999`: truST runtime/SDK reserved faults;
- `1000..=9999`: truST toolchain/generated-artifact faults;
- `10000..=65535`: standard library and built-in FB migrations;
- `65536..=0x7fff_ffff`: user/module fault enums;
- `0x8000_0000..=0xffff_ffff`: external vendor/integration faults with
  source namespace in diagnostics.

Macros that lower user `FaultCode` enums default into the user/module range
and emit a compile-time error on collision with reserved ranges unless the
user explicitly marks an external namespace.

## 11. rust-analyzer Expectations

- Macro expansion must be as rust-analyzer-friendly as practical.
- Generated symbols must not overwhelm completion with glue internals.
- SDK docs are hover-first documentation.
- Proc macro errors must preserve spans.
- When rust-analyzer is absent, VS Code must show honest degraded editing
  state without blocking CLI/CI flows.

## 12. Tests

- Positive macro expansion fixtures for each attribute.
- Negative fixtures for each rejected type/attribute combination.
- F23 rejection matrix.
- Positive validator fixture proving `iec!(...)` is admitted.
- Trybuild or equivalent compile-fail tests for macro diagnostics.
- Rustdoc/hover review for public SDK docs.
- API snapshot tests for public traits and ABI descriptors.
- Safety review for each unsafe block emitted or used by the SDK/ABI glue.
