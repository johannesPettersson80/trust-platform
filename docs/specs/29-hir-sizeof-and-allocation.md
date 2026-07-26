# HIR SIZEOF and Allocation Contract

This specification defines truST-specific HIR semantics for `SIZEOF`, `NEW`,
and `__DELETE`. IEC 61131-3 reserves or defines related language vocabulary but
does not define this complete product contract. These rules are not IEC
deviations.

## 1. SIZEOF result and accepted operands

`SIZEOF(operand)` is a compile-time DINT-compatible byte count. It accepts:

- an explicit complete type;
- a resolved variable or field with a statically known storage size;
- `THIS.field` inside the owning method;
- a pointer or reference value, whose size is the platform pointer width.

The result may participate in another constant expression, including an array
bound. A bare name that resolves both as a local value and as a top-level type
selects the in-scope value; an explicit type operand remains available through
type syntax.

On a 32-bit target the pointer/reference handle size is 4 bytes; on a 64-bit
target it is 8 bytes.

## 2. SIZEOF rejected operands

`SIZEOF` rejects:

- a call result;
- an arithmetic or other non-lvalue expression;
- an unknown or ambiguous value;
- `ARRAY[*]`, whose concrete storage size is caller-dependent;
- a FUNCTION_BLOCK instance whose runtime storage layout is not part of the
  HIR size contract;
- bare `THIS`, whose receiver storage layout is not part of the contract.

A failed nested constant expression, unknown name, or ambiguous `USING`
candidate emits its primary diagnostic once. `SIZEOF` must not add an
invalid-operation cascade that assumes an arbitrary operand.

## 3. Allocation and deletion

`NEW(Type)` accepts one resolved allocatable type operand and returns a
reference compatible with that type. A value expression in type position is
rejected as an invalid argument type. An ambiguous type reports ambiguity
rather than selecting a candidate.

`__DELETE(reference)` accepts a resolved reference produced for dynamically
allocated storage. An unresolved operand reports its primary resolution error
without a secondary argument-type diagnostic.

This section specifies HIR acceptance and diagnostics only. Allocation
lifetime, allocator failure, ownership, and runtime reclamation remain outside
this HIR invariant until separately specified.
