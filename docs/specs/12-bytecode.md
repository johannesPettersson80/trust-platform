# Bytecode Format

**Status:** Implemented 1.x container/metadata handling and the specified 1.1 execution contract. A3 implements opt-in 2.0 authoring, codecs, validation and disassembly under §11; the current hosted executor still rejects 2.0. Source-free construction/execution and production activation of the P/Q compatibility window remain A4 and release work. Scope evidence is recorded in the runtime-portability checklist.

### 1. Purpose

This document defines the bytecode format consumed by the ST runtime executor. It is intended to be stable, versioned, and easy to inspect for debugging and testing. The IEC 61131-3 standard does not define a bytecode format; this container is implementer-specific.

### 2. Goals

- Deterministic execution across platforms
- Compact, mostly fixed-width instruction encoding
- Explicit typing information for runtime checks
- Backward-compatible evolution via versioning
- KISS: one container, one section table, clear validation rules

### 3. Conventions

- Endianness: little-endian for all multi-byte integers.
- Integer sizes:
  - u8/u16/u32/u64: unsigned
  - i32/i64: signed two's complement
- Strings: UTF-8, encoded as `u32 length` followed by raw bytes (no trailing NUL).
- Arrays: `u32 count` followed by count entries.
- Offsets: `u32` byte offsets from the start of the file.
- Alignment: section offsets and lengths are 4-byte aligned; padding bytes are `0x00`.
- Jump offsets are `i32` byte deltas relative to the next instruction.

### 4. Container Layout

The bytecode is a single container with a fixed-size header and a section table.

#### 4.1 Header (Version 1.x)

```
struct Header {
  u8  magic[4];          // "STBC"
  u16 version_major;     // currently 1
  u16 version_minor;     // currently 1
  u32 flags;             // header flags (see below)
  u16 header_size;       // bytes, header only (currently 24)
  u16 section_count;     // number of section table entries
  u32 section_table_off; // offset to section table (currently 24)
  u32 checksum;          // CRC32 if flags&0x0001 != 0, else 0
}
```

Validation rules:
- `magic` must be `STBC`.
- `version_major` must be supported by the runtime.
- `header_size` and `section_table_off` must be >= 24 and 4-byte aligned.
- `section_table_off` + `section_count * 12` must fit within the file.
- If `flags & 0x0001` is set, `checksum` must be the CRC32 of the section table and all section payloads (bytes from `section_table_off` to end of file).

The current byte decoder rejects foreign major versions; it is not a general
proof of execution support for every minor it can structurally decode. Existing
1.0 metadata/container compatibility tests remain distinct from the 1.1
execution contract.

**Planned dual-major window (§11.1):** the hosted compatibility reader accepts
major in `{1, 2}`, with explicit supported version/layout policy. Preserve
existing 1.0 structural/metadata cases; hosted 1.1 execution/import requires the
matching source-derived construction context. A source-free consumer accepts
2.0 and rejects all 1.x input for execution, even if shared tooling can inspect
it. Foreign majors and unsupported pairs fail before state publication. The
encoder selects an explicit output version; increasing one shared constant is
not sufficient. Header/CRC defaults and layout selection use the version pair,
so a 2.0 minor value of zero does not accidentally select old 1.0 layouts.

#### 4.2 Section Table Entry

```
struct SectionEntry {
  u16 id;        // section identifier
  u16 flags;     // 0 = none
  u32 offset;    // absolute offset in file
  u32 length;    // section length in bytes
}
```

Section table rules:
- Entries may appear in any order.
- Each standardized section ID in the range `0x0001` through `0x000C` may
  appear at most once. A duplicate standardized section ID invalidates the
  container before any section is selected for validation or execution.
- Offsets must be 4-byte aligned.
- Sections must not overlap.
- In STBC version 1.x, an unknown section ID with `flags = 0` is an optional
  extension. The decoder preserves its payload as uninterpreted bytes, semantic
  validation ignores it, and runtime apply does not execute or otherwise
  interpret it.
- STBC version 1.x defines no required-extension flag. A future producer that
  needs an unknown section to be mandatory must use a separately reviewed
  versioned contract; it cannot encode that requirement in a version 1.x file.

The common reader rejects byte ranges beyond the supplied input, including address-size
overflow, with `UnexpectedEof` and without advancing its cursor. Invalid section extents return
`SectionOutOfBounds`. The declared header size must be 4-byte aligned, the section
table must start at or after the declared header, and payloads must start at or after
the end of the section table. Payloads may not alias unprotected header/table bytes. These are architecture-independent requirements; MCU compilation alone
does not constitute execution evidence for the 32-bit overflow branch.

#### 4.3 Section Flags

| Bit | Name | Meaning |
|-----|------|---------|
| `0x0001` | `COMPRESSED_ZSTD` | section payload is zstd compressed |
| all others | reserved | ignore if unknown |

#### 4.4 Header Flags

| Bit | Name | Meaning |
|-----|------|---------|
| `0x0001` | `CRC32` | header `checksum` is CRC32 of the section table and section payloads |

#### 4.5 Collection Count Bounds

Before reserving capacity for any top-level or nested collection encoded by a
`u32 count`, the decoder must prove with checked arithmetic that
`count * minimum_entry_bytes` fits in the unread bytes of the containing
section or entry. `minimum_entry_bytes` includes only fields present in every
encoded entry of that collection. A count that cannot fit must be rejected
before a count-sized allocation is attempted.

This is a necessary structural bound, not a general VM resource budget. It
does not define maximum container size, instruction count, stack depth, local
count, reference count, call depth, or execution-time limits; those broader
determinism and resource-limit contracts remain separately specified.

#### 4.6 Fixed Resource Limits

The STBC decoder, validator, and executor enforce the following fixed limits.
They are part of the bytecode version 1.x product contract and are not
configuration knobs:

| Resource | Maximum |
| --- | ---: |
| Encoded STBC container | 67,108,864 bytes (64 MiB) |
| Decoded instructions in one module | 1,000,000 |
| References in one module | 65,536 |
| Local references in one POU | 65,536 |
| Declared POU parameters | 1,024 |
| Native-call arguments | 1,024 |
| Operand-stack values | 16,384 |
| Active VM call frames | 1,024 |
| Executed instructions in one top-level VM invocation | 1,000,000 |
| Nested constant-payload type references | 64 |

The encoded-container limit is checked before checksum calculation or section
decoding. Count and decoded-instruction limits are checked before count-sized
or instruction-state allocation. A module above any validation limit is
rejected as a complete candidate before it can replace the active module.

The execution budget counts each executed bytecode instruction, not only loop
back-edges. Nested POU and native calls share the caller's remaining budget.
The stack interpreter and optimized register/tier-1 paths charge the same
original bytecode instructions even when lowering fuses or expands internal
operations. Exhausting the instruction budget faults the invocation through
the existing execution-timeout category; it does not define a stable public
error identifier. Deadline/watchdog checks remain independent and may fault an
invocation before its instruction budget is exhausted.

##### 4.6.1 Validation Analysis Budgets

Validation is separate from execution admission. The portable API accepts explicit
`ValidationLimits`; the hosted default is 64 MiB of accounted scratch storage and
67,108,864 logical work units per complete candidate, shared by all POUs and metadata.
Embedded profiles must select smaller measured limits. Exhaustion rejects the entire
candidate with `bytecode_invalid_section`, using the typed `ValidationStorageLimit` or
`ValidationWorkLimit` reason. No partially validated token or runtime is returned.

Lookup indexes for POU identifiers, case-insensitive POU/variable names, local-reference
intervals and reference types are built once per candidate. Sorting and lookup work is
charged; repeated instruction/debug/variable checks must not scan entire metadata tables or
rebuild maps per POU. Duplicate/name matching retains original declaration order. Local-range
validation uses sorted intervals, checking overflow/bounds before overlap and ownership.
Limits are independent upper bounds, not a guarantee that every Cartesian combination fits
one profile. Native scale fixtures must reach the one-million-instruction and 65,536-reference
limits with many POUs/debug records and prove admission under the hosted analysis defaults.

The instruction stream is decoded once. Semantic passes share that representation and
a borrowed table context. Stack dataflow retains states only at basic-block entries,
merges toward unknown types, and terminates unchanged loops without replaying them.
Every retained instruction, block-state, analysis stack, lookup vector and temporary
name buffer is reserved fallibly within the scratch budget before allocation. Work
accounting covers decoded/visited instructions, metadata and nested entries, type-chain
steps, compared string bytes, and copied/moved analysis elements. An exhausted budget
stops the analysis before that charged operation. The accounting is deterministic for
a given target layout and input; it is not CPU time or WCET.

Scratch accounting covers requested collection capacity, not allocator headers,
transient realloc copies, error diagnostics, the decoded artifact itself or native
stack. Diagnostic construction preserves the actual rejection reason even when no analysis
budget remains; budget charges apply to retained analysis, including temporary qualified names. Constant/type traversal depth remains bounded separately. Tests must include
an isolated native process memory measurement as well as exact accounting boundaries;
neither replaces MCU preparation-peak measurements. This contract closes the prior
per-instruction full-stack allocation defect without claiming a heap-free loader.

Raw struct-built modules must obey the same known section-id/payload and type-kind/
payload agreements as decoded modules. `validated()` returns an immutable borrowed
`ValidatedBytecode` token only after validation; host VM materialization requires that
token. It is not an executable module or proof of a profile's memory/time admission.
Existing `validate()` and hosted container construction remain available. Stable error
codes and existing diagnostic text remain compatible; new code uses named rejection
reasons instead of duplicating static diagnostic strings.

Serialization rejects count conversion, extent and alignment overflow before writing
truncated data, and rejects output above the encoded-container limit. The portable
`align4` helper returns `None` on overflow; `compute_type_offsets_for_entries` returns a
fallible result. Owned host/core conversions move sections without cloning, and a
borrowed view's section reference lives as long as the borrowed section slice.

##### 4.6.2 Exact Boundaries and Counter Arithmetic

An empty `REF_TABLE` and an empty `POU_INDEX` are valid inputs to declared
resource-limit validation. For every fixed declared-count limit, the exact
maximum is accepted and the first value above it is rejected. In particular,
65,536 references are accepted and 65,537 references are rejected with an
`InvalidSection` diagnostic identifying `REF_TABLE entries` as exceeding the
fixed resource limit.

Decoded-instruction accounting uses checked addition before either the fixed
limit comparison or mutation of the accepted total. If incrementing the
machine-sized counter would overflow, validation returns `InvalidSection` with
a diagnostic identifying `decoded module instruction count overflow`, and the
previous total remains unchanged.

### 5. Section IDs (Version 1.x)

| ID | Name | Required | Purpose |
|----|------|----------|---------|
| 0x0001 | STRING_TABLE | Yes | Interned UTF-8 strings |
| 0x0002 | TYPE_TABLE | Yes | Type declarations |
| 0x0003 | CONST_POOL | Yes | Constant literals |
| 0x0004 | REF_TABLE | Yes | Value reference table |
| 0x0005 | POU_INDEX | Yes | POU directory and signatures |
| 0x0006 | POU_BODIES | Yes | Bytecode bodies |
| 0x0007 | RESOURCE_META | Yes | Resources/tasks/process image |
| 0x0008 | IO_MAP | Yes | Direct I/O bindings |
| 0x0009 | DEBUG_MAP | No | Source mapping, breakpoints |
| 0x000A | DEBUG_STRING_TABLE | No | Debug-only strings (file paths) |
| 0x000B | VAR_META | No | Variable type and retention metadata |
| 0x000C | RETAIN_INIT | No | Retain initialization values |
| 0x8000-0xFFFF | VENDOR | No | Vendor/experimental |

### 6. Section Definitions

#### 6.1 STRING_TABLE (0x0001)

```
struct StringTable {
  u32 count;
  StringEntry entries[count];
}

struct StringEntry {
  u32 length;
  u8  bytes[length];
}
```

String indices are zero-based. All identifiers in other sections refer to this table.
For version >= 1.1, each `StringEntry` is padded with `0x00` bytes to the next 4-byte boundary; the padding is not included in `length`.
The DEBUG_STRING_TABLE section uses the same encoding.

#### 6.2 TYPE_TABLE (0x0002)

```
struct TypeTable {
  u32 count;
  u32 offsets[count]; // byte offsets from TYPE_TABLE start (version >= 1.1)
  TypeEntry entries[count];
}

struct TypeEntry {
  u8  kind;       // see TypeKind
  u8  flags;      // reserved
  u16 reserved;
  u32 name_idx;   // 0xFFFFFFFF for anonymous
  // payload follows based on kind
}
```

For version 1.0, `offsets` is omitted and entries are stored back-to-back.

Type kinds (Version 1.x):
- 0 PRIMITIVE
- 1 ARRAY
- 2 STRUCT
- 3 ENUM
- 4 ALIAS
- 5 SUBRANGE
- 6 REFERENCE
- 7 UNION
- 8 FUNCTION_BLOCK
- 9 CLASS
- 10 INTERFACE

Primitive payload:
```
struct PrimitiveType {
  u16 prim_id;     // see PrimitiveId
  u16 max_length;  // for STRING/WSTRING; 0 means default/unspecified
}
```

Array payload:
```
struct ArrayType {
  u32 elem_type_id;
  u32 dim_count;
  Dim dims[dim_count];
}

struct Dim {
  i64 lower;
  i64 upper;
}
```

Struct payload:
```
struct StructType {
  u32 field_count;
  Field fields[field_count];
}

struct Field {
  u32 name_idx;
  u32 type_id;
}
```

Enum payload:
```
struct EnumType {
  u32 base_type_id; // integer type
  u32 variant_count;
  Variant variants[variant_count];
}

struct Variant {
  u32 name_idx;
  i64 value;
}
```

Alias payload:
```
struct AliasType {
  u32 target_type_id;
}
```

Ordinary enumerated data types use `ENUM` and retain their closed, ordered
variant set. Data types with named integer values use `ALIAS` to their declared
integer base because they retain that base type's full value range and
arithmetic behavior rather than forming a closed enumeration.

Subrange payload:
```
struct SubrangeType {
  u32 base_type_id; // signed/unsigned integer
  i64 lower;
  i64 upper;
}
```

Reference payload:
```
struct ReferenceType {
  u32 target_type_id;
}
```

Union payload:
```
struct UnionType {
  u32 field_count;
  Field fields[field_count];
}
```

POU type payload (FUNCTION_BLOCK / CLASS):
```
struct PouType {
  u32 pou_id; // POU_INDEX id
}
```

Interface payload:
```
struct InterfaceType {
  u32 method_count;
  InterfaceMethod methods[method_count];
}

struct InterfaceMethod {
  u32 name_idx;
  u32 slot; // interface method slot (0..method_count-1)
}
```

Primitive IDs (Version 1.x):
- 1 BOOL
- 2 BYTE
- 3 WORD
- 4 DWORD
- 5 LWORD
- 6 SINT
- 7 INT
- 8 DINT
- 9 LINT
- 10 USINT
- 11 UINT
- 12 UDINT
- 13 ULINT
- 14 REAL
- 15 LREAL
- 16 TIME
- 17 LTIME
- 18 DATE
- 19 LDATE
- 20 TOD
- 21 LTOD
- 22 DT
- 23 LDT
- 24 STRING
- 25 WSTRING
- 26 CHAR
- 27 WCHAR

#### 6.3 CONST_POOL (0x0003)

```
struct ConstPool {
  u32 count;
  ConstEntry entries[count];
}

struct ConstEntry {
  u32 type_id;
  u32 payload_len;
  u8  payload[payload_len];
}
```

Payload encoding follows the referenced type:
- Integer/boolean: little-endian, natural size of the primitive.
- REAL/LREAL: IEEE-754 binary32/binary64.
- STRING/WSTRING: the complete entry payload is UTF-8/UTF-16LE text. When a
  string value is nested in an aggregate constant, the aggregate child frame
  supplies its payload boundary.
- TIME/LTIME: `i64` nanoseconds.
- DATE/TOD/DT: `i64` ticks in the runtime `DateTimeProfile` resolution.
- LDATE/LTOD/LDT: `i64` nanoseconds.
- REFERENCE: `0xFFFFFFFF` for NULL. STBC 1.1 does not materialize a live
  `REF_TABLE` identity from `CONST_POOL`; every other `u32` value is invalid.
- ARRAY: `u32 elem_count` followed by `elem_count` child frames. Each child
  frame is `u32 payload_len` plus that element's constant payload.
  `ARRAY[*]` uses the sentinel bounds `(0, i64::MAX)` in `TYPE_TABLE` and has
  exactly zero elements in `CONST_POOL`; the concrete caller shape is supplied
  by call binding.
- STRUCT/UNION: `u32 field_count` followed by one child frame per field in
  declaration order. Each child frame is `u32 payload_len` plus that field's
  constant payload.
- ENUM: `i64` numeric value.

Aggregate child framing is the canonical STBC 1.1 representation. Aggregate
constants were not materializable by the earlier implementation, so this
defines their first accepted wire representation without making an older
unframed draft valid. Encoder, validator, and runtime materialization apply the
same maximum of 64 nested alias, subrange, array, structure, and union type
references. A deeper or cyclic type path rejects the complete candidate before
apply.

#### 6.4 REF_TABLE (0x0004)

Static value references used by LOAD/STORE instructions and task FB associations.

```
struct RefTable {
  u32 count;
  RefEntry entries[count];
}

struct RefEntry {
  u8  location;     // see RefLocation
  u8  flags;        // reserved
  u16 reserved;
  u32 owner_id;     // frame/instance id; 0 for global/retain/io
  u32 offset;       // variable index within the owner scope
  u32 segment_count;
  RefSegment segments[segment_count];
}
```

Reference locations:
- 0 GLOBAL
- 1 LOCAL
- 2 INSTANCE
- 3 IO
- 4 RETAIN

Reference segments:
```
struct RefSegment {
  u8  kind; // 0 = INDEX, 1 = FIELD
  u8  reserved[3];
  union {
    IndexSegment index;
    FieldSegment field;
  };
}

struct IndexSegment {
  u32 count;
  i64 indices[count];
}

struct FieldSegment {
  u32 name_idx;
}
```

#### 6.5 POU_INDEX (0x0005)

```
struct PouIndex {
  u32 count;
  PouEntry entries[count];
}

struct PouEntry {
  u32 id;
  u32 name_idx;
  u8  kind;        // 0 PROGRAM, 1 FUNCTION_BLOCK, 2 FUNCTION, 3 CLASS, 4 METHOD
  u8  flags;       // reserved
  u16 reserved;
  u32 code_offset; // offset within POU_BODIES section
  u32 code_length; // byte length (0 if no body)
  u32 local_ref_start;
  u32 local_ref_count;
  u32 return_type_id; // 0xFFFFFFFF if no return
  u32 owner_pou_id;   // METHOD only; 0xFFFFFFFF otherwise
  u32 param_count;
  ParamEntry params[param_count];
  // if kind == FUNCTION_BLOCK or CLASS:
  u32 parent_pou_id; // 0xFFFFFFFF if no EXTENDS
  u32 interface_count;
  InterfaceImpl interfaces[interface_count];
  u32 method_count;
  MethodEntry methods[method_count];
}

struct ParamEntry {
  u32 name_idx;
  u32 type_id;
  u8  direction;   // 0 IN, 1 OUT, 2 IN_OUT
  u8  flags;       // reserved
  u16 reserved;
  u32 default_const_idx; // CONST_POOL index (0xFFFFFFFF if none; version >= 1.1)
}

`default_const_idx` is present in bytecode format `1.1`, which is the only
supported minor version. It carries portable call-local defaults for function
and method `IN` and `OUT` parameters. An interface type default remains NULL
and therefore needs no constant entry. A class or function-block formal has no
portable `CONST_POOL` default; a supplied actual is bound directly and omission
cannot fabricate an instance constant. Function-block parameter defaults remain
in instance storage and are not duplicated as call-local constants; the
explicit `EN`/`ENO` execution-control defaults remain encoded.

struct MethodEntry {
  u32 name_idx;
  u32 pou_id;      // method POU id
  u32 vtable_slot; // virtual dispatch slot
  u8  access;      // 0 PUBLIC, 1 PROTECTED, 2 PRIVATE
  u8  flags;       // 0x01 OVERRIDE, 0x02 FINAL, 0x04 ABSTRACT
  u16 reserved;
}

struct InterfaceImpl {
  u32 interface_type_id; // TYPE_TABLE index
  u32 method_count;
  u32 vtable_slots[method_count]; // map interface slot -> class vtable slot
}
```

#### 6.6 POU_BODIES (0x0006)

A raw bytecode blob that contains all POU instruction streams. Offsets are relative to the start of this section.

#### 6.7 RESOURCE_META (0x0007)

```
struct ResourceMeta {
  u32 resource_count;
  ResourceEntry resources[resource_count];
}

struct ResourceEntry {
  u32 name_idx;
  u32 inputs_size;
  u32 outputs_size;
  u32 memory_size;
  u32 task_count;
  TaskEntry tasks[task_count];
}

struct TaskEntry {
  u32 name_idx;
  u32 priority;        // 0 = highest priority
  i64 interval_nanos;  // 0 disables periodic scheduling
  u32 single_name_idx; // 0xFFFFFFFF means none
  u32 program_count;
  u32 program_name_idx[program_count];
  u32 fb_ref_count;
  u32 fb_ref_idx[fb_ref_count];
}
```

`ResourceEntry.name_idx` resolves through `STRING_TABLE` to the exact IEC
`RESOURCE` identifier represented by that entry. Newly encoded bytecode must
not substitute a generic placeholder when the source declares a resource. A
source without a `RESOURCE` declaration uses the synthetic name `RESOURCE` for
the runtime's single implicit execution resource.

For newly encoded bytecode, `inputs_size` and `outputs_size` are derived from
the highest addressed input and output process-image byte required by the
encoded `IO_MAP` bindings. Bit and byte bindings occupy at least one byte;
WORD, DWORD, and LWORD bindings occupy two, four, and eight bytes respectively,
and byte-array bindings occupy their declared length. A resource with an
addressed input or output therefore declares a non-zero corresponding process
image size large enough to contain that binding.

#### 6.8 IO_MAP (0x0008)

Direct I/O bindings between the process image and program variables.

```
struct IoMap {
  u32 binding_count;
  IoBinding bindings[binding_count];
}

struct IoBinding {
  u32 address_str_idx;  // IEC address string (e.g., "%IX0.0")
  u32 ref_idx;          // REF_TABLE entry
  u32 type_id;          // 0xFFFFFFFF if unspecified
}
```

#### 6.9 DEBUG_STRING_TABLE (0x000A, optional)

Same encoding as STRING_TABLE. Used for debug-only strings such as source file paths.

#### 6.10 DEBUG_MAP (0x0009, optional)

```
struct DebugMap {
  u32 entry_count;
  DebugEntry entries[entry_count];
}

struct DebugEntry {
  u32 pou_id;
  u32 code_offset;  // offset within POU_BODIES
  u32 file_idx;     // debug string table index (v1.1+)
  u32 line;         // 1-based
  u32 column;       // 1-based
  u8  kind;         // 0 statement, 1 breakpoint, 2 scope
  u8  reserved[3];
}
```

For version >= 1.1, `file_idx` refers to DEBUG_STRING_TABLE. For version 1.0, it refers to STRING_TABLE.

#### 6.11 VAR_META (0x000B, optional)

```
struct VarMeta {
  u32 entry_count;
  VarMetaEntry entries[entry_count];
}

struct VarMetaEntry {
  u32 name_idx;        // STRING_TABLE index
  u32 type_id;         // TYPE_TABLE index
  u32 ref_idx;         // REF_TABLE index
  u8  retain;          // 0=UNSPECIFIED, 1=RETAIN, 2=NON_RETAIN, 3=PERSISTENT
  u8  reserved;
  u16 reserved2;
  u32 init_const_idx;  // CONST_POOL index (0xFFFFFFFF if none)
}
```

VarMeta entries describe typed storage references. Global and instance-storage
entries use their source variable names and may carry retain or initializer
metadata. Base local declarations use the reserved name
`@local/<pou_id>/<slot>/<name>`, where `slot` is the zero-based offset within
the POU's contiguous local-reference range. Local entries must use `retain = 0`,
must not carry an initializer constant, and must refer to an empty-path LOCAL
reference owned by exactly one POU. Return and parameter entry types must match
the corresponding POU_INDEX signature; the entry type is authoritative for
declared local variables, whose types are otherwise absent from POU_INDEX.

Within one `VAR_META` section, every `ref_idx` is unique and every resolved
textual name is unique even when duplicate text appears at different
`STRING_TABLE` indexes. These constraints prevent order-dependent metadata
selection. A local entry must not carry retain state or an initializer; those
states are rejected rather than silently ignored.

Version 1.1 containers produced before local metadata was introduced may omit
these local entries. A runtime may continue untyped numeric copy-back for such a
container, but it must reject STRING or WSTRING output copy-back when the
receiving declaration's type cannot be recovered; it must not perform an
unbounded raw string write. Internal `Null` remains the unassigned-output
sentinel; every non-null value copied to a declared STRING or WSTRING target
must match that target's string family before normalization.

The local metadata extension retains the version 1.1 wire layout. Runtimes
before truST 0.24.34 do not enforce its local string-copy semantics, so
bytecode generated by truST 0.24.34 or later that uses local string output
targets must be deployed with a runtime from the same or a later release.
Mixed deployment with an older runtime is unsupported.

These 1.1 rules remain unchanged. Executable local initializers are not encoded
as `VAR_META.init_const_idx`; the planned source-free representation is described
in §11 and requires an explicitly supported new format version.

#### 6.12 RETAIN_INIT (0x000C, optional)

```
struct RetainInit {
  u32 entry_count;
  RetainInitEntry entries[entry_count];
}

struct RetainInitEntry {
  u32 ref_idx;    // REF_TABLE index
  u32 const_idx;  // CONST_POOL index
}
```

RetainInit provides cold-start initialization values for retained variables; warm restarts restore retained state instead.

### 7. Instruction Encoding (Version 1.x)

#### 7.1 Encoding Rules

- Each instruction begins with a 1-byte opcode.
- Operands are encoded in little-endian, with sizes defined per opcode.
- Invalid opcodes or malformed operands cause a runtime fault.

#### 7.2 Operand Types

- `u32` indexes refer to STRING_TABLE, TYPE_TABLE, CONST_POOL, REF_TABLE, or POU_INDEX as documented.
- `i32` offsets are relative to the next instruction.
- Stack values are `Value` instances; references are pushed as `Value::Reference`.

#### 7.3 Accepted Instruction Set

This table is the executable STBC 1.1 contract. An opcode not listed here is
not accepted merely because an older design document assigned it a mnemonic.
The validator rejects unimplemented values before dispatch.

Control flow:
- `0x00 NOP`
- `0x01 HALT`
- `0x02 JMP i32`
- `0x03 JMP_TRUE i32` (pop bool)
- `0x04 JMP_FALSE i32` (pop bool)
- `0x06 RET`
- `0x09 CALL_NATIVE u32 u32 u32` (`kind`, `symbol_idx`, `arg_count`; pop
  encoded arguments and push the call result)

Stack and constants:
- `0x10 CONST u32` (const pool index)
- `0x11 DUP`
- `0x12 POP`
- `0x13 SWAP`

Static references:
- `0x20 LOAD_REF u32` (ref table index)
- `0x21 STORE_REF u32` (ref table index)
- `0x22 PUSH_REF u32` (push `Value::Reference`)
- `0x23 LOAD_SELF` (push the current instance)
- `0x24 LOAD_SUPER` (push the current instance's parent)
- `0x25 LOAD_NULL` (push the null value)

Dynamic references:
- `0x30 REF_FIELD u32` (field name index; pop ref, push ref)
- `0x31 REF_INDEX` (pop index, pop ref, push ref)
- `0x32 LOAD` (pop ref, push value)
- `0x33 STORE` (pop value, pop ref)

Arithmetic and logic:
- `0x40 ADD`
- `0x41 SUB`
- `0x42 MUL`
- `0x43 DIV` (fault on divide by zero)
- `0x44 MOD`
- `0x45 NEG`
- `0x46 AND`
- `0x47 OR`
- `0x48 XOR`
- `0x49 NOT`
- `0x4C EXPT`

Comparison:
- `0x50 EQ`
- `0x51 NE`
- `0x52 LT`
- `0x53 LE`
- `0x54 GT`
- `0x55 GE`

Type and partial access:
- `0x60 SIZEOF_TYPE u32` (type-table index; push byte size as DINT)
- `0x61 SIZEOF_VALUE` (pop value; push byte size as DINT)
- `0x62 PARTIAL_READ u32` (pop value; push selected bit/byte/word/dword field)
- `0x63 PARTIAL_WRITE u32` (pop replacement and value; push updated value)
- `0x64 REFERENCE_ATTEMPT u32` (target type-table index; pop source and push
  the compatible reference/interface identity or the target family's null)

Debug marker:
- `0x70 DEBUG_MARK u32` (consume a debug marker index without changing
  product state)

The following previously published or legacy values are explicitly
unimplemented in STBC 1.1 and are rejected before dispatch: `0x05`, `0x07`,
`0x08`, `0x14`, `0x15`, `0x16`, `0x4A`, `0x4B`, `0x4D`, and `0x4E`.
Standard-library, function, function-block, and method calls use
`CALL_NATIVE`; bit shifts and rotates use the registered runtime operations
rather than those unimplemented bytecode values.

`REFERENCE_ATTEMPT` is the executable form of source `?=`. Its operand must
name a `Reference` or `Interface` type-table entry. For `REF_TO`, the VM reads
the source reference's live storage type and accepts an exact target,
derived-to-base relation, or implemented-interface relation; incompatibility
pushes `Value::Reference(None)`. For an interface target, the VM checks the
live instance POU against the class/function-block parent and implemented-
interface metadata; incompatibility pushes `Value::Null`. A null source always
produces the target family's null. The opcode does not mutate the assignment
target itself; the following validated store performs the single write.

Reserved opcode ranges:
- `0x80-0xEF` reserved for future core extensions.
- `0xF0-0xFF` vendor/experimental.

#### 7.4 Validator Before Apply

An STBC module must pass complete structural and semantic validation before it
may replace the runtime's active module or mutate runtime metadata, configured
tasks, process-image sizing, retain state, or executable VM state. Validation
is fail-closed: the first observed violation rejects the complete candidate
module; validation order does not make an otherwise invalid module acceptable.

The validator enforces these module-wide contracts:

- every required section in section 5 is present with the decoded section
  kind assigned to that ID;
- string, type, constant, reference, POU, variable, resource, I/O, retain, and
  debug indexes resolve inside their owning table;
- array bounds are ordered, constant payloads are complete, bounded to 64
  nested type references, and compatible with their declared type, and
  optional metadata agrees with the referenced POU or storage declaration;
- POU IDs are unique, code ranges stay inside `POU_BODIES`, local-reference
  ranges are checked for arithmetic overflow, bounds, overlap, contiguous
  offsets, local location, and unique frame ownership;
- a POU may use a local reference only from its declared local range, including
  path references rooted in that same frame owner;
- a frame-local reference must not be stored into global, retain, I/O,
  instance, or otherwise longer-lived storage, directly or through a dynamic
  non-local reference;
- every instruction is recognized and carries its complete fixed-width
  operand payload; an opcode in a reserved range remains invalid until the
  accepted bytecode version explicitly implements it;
- direct, native, method, and interface calls resolve to compatible targets,
  metadata, parameter directions, and argument shapes supported by the active
  runtime;
- every relative jump stays inside its POU body and lands at a decoded
  instruction boundary or the exact end of that body;
- operand-stack dataflow has no underflow, has compatible depth at each
  control-flow merge, uses reference/numeric/boolean shapes where required,
  and leaves no values at a normal POU-body exit; and
- resource, task, program, I/O, retain, variable, and debug metadata resolves
  to compatible table entries and code locations.

`BytecodeModule::decode` may reject malformed container bytes before semantic
validation. `BytecodeModule::validate` performs the decoded semantic checks.
`Runtime::apply_bytecode_bytes` must perform both boundaries and must
materialize the candidate executable module before changing live runtime
metadata. Any rejection leaves the previously active runtime configuration and
executable module unchanged.

`BytecodeError` variants identify the in-process failure category. Every
variant also has the following stable machine identifier. Diagnostic text may
provide a narrower reason, but text is not part of the machine contract.

| `BytecodeError` variant | Stable identifier |
| --- | --- |
| `InvalidMagic` | `bytecode_invalid_magic` |
| `UnsupportedVersion` | `bytecode_unsupported_version` |
| `InvalidHeader` | `bytecode_invalid_header` |
| `InvalidChecksum` | `bytecode_invalid_checksum` |
| `InvalidSectionTable` | `bytecode_invalid_section_table` |
| `SectionOutOfBounds` | `bytecode_section_out_of_bounds` |
| `SectionOverlap` | `bytecode_section_overlap` |
| `SectionAlignment` | `bytecode_section_alignment` |
| `UnexpectedEof` | `bytecode_unexpected_eof` |
| `InvalidSection` | `bytecode_invalid_section` |
| `MissingSection` | `bytecode_missing_section` |
| `InvalidOpcode` | `bytecode_invalid_opcode` |
| `InvalidJumpTarget` | `bytecode_invalid_jump_target` |
| `InvalidPouId` | `bytecode_invalid_pou_id` |
| `InvalidIndex` | `bytecode_invalid_index` |

`BytecodeError::stable_code()` returns the table entry. Conversion into the
public runtime error preserves that identifier; it must not derive a code by
parsing `Display` text. Direct decode/validation and
`Runtime::apply_bytecode_bytes` therefore report the same identifier for the
same rejected candidate. Control responses place the identifier in
`error_code` while retaining the existing human-readable `error` field.

The fixed limits in section 4.6 are validated before apply and enforced again
at their allocation or execution boundary. Their diagnostic text remains
non-normative even though the enclosing error category has a stable machine
identifier.

POU body extents use checked arithmetic and reject overflow as
`InvalidSection("POU code out of bounds")`. Relative jumps use checked signed arithmetic;
an unrepresentable target returns `InvalidJumpTarget` carrying the encoded displacement.
Representable invalid targets continue to report the target. Debug and release builds must
reject these malformed inputs without panicking. Struct-built modules must also reject code
positions/lengths outside the validator's signed-offset representation.

#### 7.4.1 VM Trap Identifiers

VM traps that represent malformed executable state retain a stable identifier
when converted to `RuntimeError`. Invalid opcode, jump, POU, and table-index
traps reuse the corresponding `bytecode_*` identifier above. The remaining
VM-only structural identifiers are:

| Trap category | Stable identifier |
| --- | --- |
| Operand stack underflow | `vm_stack_underflow` |
| Operand stack overflow | `vm_stack_overflow` |
| Call stack underflow | `vm_call_stack_underflow` |
| Call stack overflow | `vm_call_stack_overflow` |
| Unsupported runtime opcode | `vm_unsupported_opcode` |
| Unsupported reference location | `vm_unsupported_reference_location` |
| Invalid native-call metadata or payload | `vm_invalid_native_call` |
| Bytecode decode failure without a narrower decoder variant | `vm_bytecode_decode` |

Condition, null-reference, loop-step, deadline, instruction-budget, and other
runtime-value traps use the stable `runtime_*` identifiers in
`docs/specs/10-runtime-semantics.md`. `VmTrap::Runtime` preserves the embedded
runtime error's identifier.

#### 7.4.2 Numeric and Partial-Access Domains

For bytecode stack-shape validation, primitive IDs 6 through 15 inclusive are
the complete numeric domain. IDs outside that interval, including BOOL,
bit-string, temporal, string, null, and unknown IDs, are not classified as
numeric merely because they have a primitive table entry.

The `u32` operand of `PARTIAL_READ` and `PARTIAL_WRITE` packs the partial kind
in bits 8 and 9 and the zero-based index in bits 0 through 7; all higher bits
must be zero. Kind 0 accepts bit indices 0 through 63, kind 1 accepts byte
indices 0 through 7, kind 2 accepts word indices 0 through 3, and kind 3
accepts dword indices 0 through 1. The validator rejects the first index above
each range and any operand with bits outside the ten-bit encoding domain.

#### 7.5 Fault Semantics

The executor must fault on:
- Type mismatches (e.g., BOOL in arithmetic)
- Invalid references or out-of-bounds indexes
- Divide by zero
- FOR loop step expressions that evaluate to 0 (encoder emits a step==0 guard that executes `HALT` before loop entry)
- Invalid jump targets
- Method/interface dispatch on NULL or incompatible references

##### 7.5.1 Source-to-Bytecode Projection

Successful source lowering emits one complete module that decodes and passes
section 7.4 validation. Identifier lookup performed before emission is
case-insensitive, so accepted call-heavy source remains encodable regardless of
identifier spelling case; emission preserves the resolved declaration
identity.

The encoder projects source declarations as follows:

- aliases, arrays, structures, unions, enumerations, subranges, references,
  classes, function blocks, and interfaces retain their declared type
  relationships in `TYPE_TABLE`;
- class inheritance, method ownership, override slots, interface slots,
  parameter directions, and call-local function/method parameter defaults
  retain their relationships in `POU_INDEX` and `CONST_POOL`;
- function and method return, parameter, and local slots occupy contiguous
  LOCAL `REF_TABLE` ranges, and their scoped names and declared types are
  emitted in `VAR_META`;
- retained initialized storage emits compatible `VAR_META`, `CONST_POOL`, and
  `RETAIN_INIT` entries;
- direct variables emit `IO_MAP` bindings and the corresponding
  `RESOURCE_META` process-image sizes described in sections 6.7 and 6.8;
- declared resource and task identities are preserved so applying the encoded
  module materializes the same named runtime resource and tasks;
- source locations emit `DEBUG_MAP` entries with the owning POU, bytecode
  offset, file, line, column, and statement kind;
- a label with an empty statement emits an explicit `NOP`, preserving a
  valid instruction location;
- IF/ELSIF, CASE, WHILE, REPEAT, and FOR control flow emits validated relative
  branch instructions and explicit comparison/stack operations; and
- instance and method field access emits `LOAD_SELF`, `REF_FIELD`, `LOAD`, and
  `STORE` operations as required by the resolved lvalue or expression.

An enum initializer must use a constant payload compatible with its enum base
type. Any unsupported source construct fails lowering; it is not replaced with
an unrelated placeholder instruction.

###### 7.5.1.1 Recursive Lowering Classification

The encoder's support and required-seam classification traverses complete
expression, lvalue, statement, and call-argument shapes. This includes call
targets, value and writable-target arguments, indexed and dereferenced
lvalues, `REF` expressions, and `SIZEOF` targets nested inside a returned or
assigned expression. A plain name with no such descendant is not classified as
containing a call or `SIZEOF`. This recursive classification prevents a nested
unsupported construct from being hidden by an otherwise supported parent.

###### 7.5.1.2 RETURN, REF, and Partial-Access Lowering

A bare `RETURN` emits opcode `0x06`. A value-bearing `RETURN` is emitted only
when the current POU has a resolved return slot; without that slot it remains
unsupported and no value-return code is emitted. The `REF` builtin requires
exactly one addressable target. Wrong arity rejects lowering, while one
resolved writable target emits `LOAD_REF_ADDR` (`0x22`) followed by its
four-byte reference-table index.

A static partial read first resolves and loads its target, then emits
`PARTIAL_READ` (`0x62`) followed by the packed four-byte partial-access
operand. A partial write emits `PARTIAL_WRITE` (`0x63`) with the same operand
encoding. The kind byte is 0 for bit, 1 for byte, 2 for word, and 3 for dword;
the selected zero-based index occupies the low byte.

###### 7.5.1.3 Literal Index Projection

Static index projection accepts every signed integer, unsigned integer, and
bit-string literal width represented by the runtime value model and preserves
its mathematical value in an `i64` index. A nonliteral expression or a
nonnumeric literal is not a static literal index and is left for dynamic
lowering. An unsigned `ULINT` or `LWORD` value above `i64::MAX` rejects static
projection with an `index literal overflow` diagnostic; it is never wrapped or
truncated.

###### 7.5.1.4 Public Constructor Debug Projection

All three public `BytecodeModule` runtime constructors emit the same supported
bytecode version and a module that passes validation. The source-free
constructor emits no debug sections. When source text is supplied, every
emitted statement location produces debug sections and uses the deterministic
fallback label `file_<file_id>`. When matching source paths are also supplied,
the same debug entry instead resolves to the supplied path. Supplying a source
and path count mismatch rejects module construction.

###### 7.5.1.5 Decoded Module and Class-Like Encoder Structure

`trust_runtime::bytecode::BytecodeModule` is the public decoded-container
representation. Its `version`, `flags`, and ordered `sections` fields expose
the values decoded from or destined for the container described in sections 4
through 6. Constructing or decoding this structure does not by itself assert
semantic validity; callers must pass it through the validator before runtime
application. Rust field layout is not a wire-format or stable ABI promise.
Wire compatibility is defined only by the encoded byte sequence in this
specification. Structural equality compares all three exposed components.

The encoder's private `ClassLike` adapter is a structural projection shared by
class and function-block metadata emission:

- `name()` borrows the exact declared class or function-block identity;
- `base_name()` returns the declared base identity without changing spelling
  or assigning different encoder semantics to a function-block base and class
  base; absence remains `None`; and
- `interfaces()` borrows the complete declared interface list in source order;
  and
- `methods()` borrows the complete method list in declaration order.

These signatures create no independent runtime behavior. Their authority is
the preservation of already-specified source identities and relationships
while the class metadata paths in section 7.5.1 emit `TYPE_TABLE` and
`POU_INDEX`. They must not synthesize a base, reorder or filter methods, or
rename the owner. The public type declaration and the four private trait
signatures are structural code facts: their acceptance evidence is exact
source-to-spec identity and downstream encoder behavior proof, not an invented
unit test for the existence of a Rust declaration.

#### 7.6 Source-to-Bytecode Fail-Closed Boundary

Source analysis and bytecode lowering are separate acceptance boundaries. A
source construct may be valid in the analyzed runtime model while the bytecode
encoder does not yet implement its executable semantics. In that case,
bytecode-module construction must fail visibly and return no module. The
encoder must not replace the construct with `NOP`, discard the unsupported
subtree, or return a module containing only the successfully emitted prefix.

The reviewed lowering partitions are:

- supported `EXIT` and `CONTINUE` statements inside active loops emit their
  defined jump paths and remain executable;
- a source `JMP` statement, which is accepted by source analysis but has no
  reviewed bytecode lowering, rejects bytecode-module construction; and
- an executable array-initializer assignment, including one following an
  otherwise supported statement, rejects bytecode-module construction while
  that expression remains unsupported by the encoder; and
- a function declared with the reviewed explicit `: VOID` return type rejects
  bytecode-module construction with a diagnostic containing
  `unsupported generic type`. This failure occurs before the function body's
  `VAR_IN_OUT` expressions execute and therefore proves no copy-back or
  conversion behavior.

During source lowering, an explicit empty label is the sole reviewed
intentional no-action statement that may emit `NOP`. The presence of the
encoded `NOP` instruction in an already constructed module is governed by the
normal instruction contract and does not authorize fail-open source lowering.

This boundary requires a visible compiler build error and is not a runtime
failure surface. The stable bytecode and VM identifiers in Sections 7.4 and
7.4.1 apply after bytecode production; they do not replace source-lowering
diagnostics.

#### 7.7 VM Call Binding and Copy-Back

VM calls bind arguments deterministically to the callee's declared parameter
order. Positional arguments consume the next available parameters; named
arguments bind by the declared name without changing declaration order.
Duplicate or unknown names, excess positional arguments, missing required
arguments, and holes in a variadic suffix reject before callee execution or
caller mutation.

An omitted function-block input preserves the instance's stored field value.
An omitted output or `VAR_IN_OUT` parameter creates no copy-back binding and
does not attempt to resolve a target. A supplied output or `VAR_IN_OUT` target
must be writable and compatible with the declared parameter type.
`VAR_IN_OUT` copy-in and copy-back preserve the same caller target and require
exact family and bounded-string capacity compatibility. A rejected binding or
copy-back leaves the caller target and unaffected instance fields unchanged.
The complete binding set is validated before any copy-in or field mutation, so
a failure on a later argument also leaves earlier inputs and fields unchanged.

Standard-function fixed and variadic parameters use the same declaration-order,
duplicate-name, arity, and variadic-hole rules. Native split functions write
each declared output once and return `NULL`; `SPLIT_DATE` requires one input and
three writable outputs, and named split outputs match formal names
case-insensitively. Runtime clock functions accept exactly zero arguments.
Integer output helpers preserve the target's declared integer width and reject
a negative value for unsigned storage before writing.

A native-call symbol descriptor contains the target followed by ordered `E`
expression and `T` writable-target descriptors, each optionally carrying a
formal name. Payload decoding preserves descriptor and argument order,
distinguishes values from writable references, and removes the declared
receiver separately. An unknown descriptor kind rejects as
`vm_invalid_native_call`.

#### 7.8 VM Reference Resolution

Every VM reference route resolves the same logical storage location and
produces the same value, mutation, or trap. An empty path may use direct global,
instance, or current-frame storage. Nested paths, inherited fields, and dynamic
references may use generic resolution, but the selected access route is not
observable in the program result.

Field and array-path extension preserves the existing path and applies each
additional segment against the declared aggregate shape. Index arithmetic is
overflow-checked, including arrays with extreme signed lower bounds. A null
reference, missing field, incompatible aggregate shape, or invalid index faults
without mutating storage. Function and method interface slots that have no
materialized value begin as `NULL`.

#### 7.9 Optimized Backend Semantic Equivalence

The stack executor defines the VM's observable semantics. Register-IR and
tier-1 execution must produce the same return value, declared runtime types,
storage mutations, instruction-budget boundary, and runtime-error category
from the same module and initial state. Nested calls share the top-level
instruction budget. Deadline and budget failure occur before the guarded
operation commits an observable mutation.

An optimized backend may decline an instruction, block, or POU and fall back
only before that backend has made an observable mutation. Cache state,
profiling counters, allocation reuse, polling stride, and diagnostic prose are
implementation details and are not part of this semantic-equivalence oracle.

##### 7.9.1 Register-IR lowering, verification, and fusion

Register-IR lowering preserves bytecode operands, control flow, stack
semantics, and original instruction costs. It rejects stack underflow,
inconsistent merge depths, invalid block targets, undefined register reads,
out-of-range destinations, and missing or inconsistent original-cost metadata.
`RETURN` ends propagation and does not acquire a synthetic fallthrough edge.

Fusion may replace only a complete reviewed instruction window. The fused form
preserves operand order, read dependencies, branch behavior, runtime faults,
and the sum of the original bytecode instruction costs. A partial, guarded, or
otherwise unmatched window remains unfused without changing the surrounding
instructions. Unsupported lowering preserves the complete original operands
for a pre-mutation stack-executor fallback.

##### 7.9.2 Tier-1 specialization

Tier-1 compilation either accepts a reviewed register block and executes it
with register-interpreter semantics, or declines it before mutation. A guard
mismatch is a decline rather than a reinterpretation of the operands. Accepted
blocks preserve arithmetic and comparison results, boolean and branch
behavior, dynamic and inherited reference semantics, function and
function-block calls, runtime traps, and original instruction costs.

##### 7.9.3 Optimized-backend operational observability

Operational controls and telemetry remain separate from the semantic-
equivalence oracle above, but their own runtime contract is deterministic:

- absent, explicit true/false, and invalid Boolean environment tokens resolve
  to their documented defaults; tier-1 is disabled by default, and valid
  threshold and capacity tokens select the requested positive bounds;
- lowering and specialized-block caches reuse prior results, cache lowering
  failures, respect configured capacity, evict when bounded capacity is
  exceeded, stay cold until the hot threshold, and clear all entries and
  counters on reset;
- pooled register files and execution buffers preserve requested capacity,
  return cleared frames and registers, and never retain more than the
  configured pool limit;
- profile snapshots count the executed register, reference, call, fallback,
  cache, and value-movement operations actually taken. Direct scalar and
  borrowed-reference paths do not increment clone counters that those paths
  avoid;
- stack and register deadline polling check the first guarded operation and
  the documented stride boundaries. Unsupported lowering or specialization
  records a bounded reason and falls back before an observable mutation;
- debug-map lookup returns the source location owned by the current bytecode
  instruction, and diagnostic corpus probes report register execution,
  lowering-cache use, and fallback reasons without changing program results;
  and
- native-call descriptor parsing and resolved native-function lookup may be
  cached, but a cached parse failure preserves its original error and cached
  success preserves the same ordered descriptor and resolved function
  identity.

These observations specify configuration, resource, and diagnostic behavior
only. Cache hits, counters, buffer identity, polling frequency, and prose are
not Structured Text program outputs and remain excluded from optimized-backend
semantic equivalence.

#### 7.10 VM Module Materialization and Lookup

After container and semantic validation, VM materialization builds one
immutable execution view before any POU runs. `STRING_TABLE`, `TYPE_TABLE`,
`CONST_POOL`, `REF_TABLE`, `POU_INDEX`, and `POU_BODIES` are required at this
boundary. A missing required section, invalid table reference, duplicate POU
identity, duplicate owner-local method identity, invalid code range, or
fixed-limit violation rejects the complete view as `vm_bytecode_decode`; no
partially indexed module is returned.

POU names are indexed case-insensitively by their declared kind. Program,
function, function-block, and class names occupy distinct lookup maps. Method
lookup is case-insensitive within its owning class or function block and does
not search an unrelated owner. Parameter order, direction, type, default
constant, return-slot presence, and local-reference range are preserved from
`POU_INDEX`.

`VAR_META` binds at most one declared type to each reference index. A duplicate
reference index rejects materialization rather than allowing the later row to
replace the earlier type. Missing `VAR_META` yields an empty optional type map;
it does not invent types.

Each `REF_TABLE` row materializes exactly one VM reference:

- GLOBAL, RETAIN, and INSTANCE preserve their offset and path;
- LOCAL additionally preserves its owning frame identity;
- IO owner 0, 1, or 2 selects input, output, or memory respectively; any other
  IO owner rejects materialization;
- index segments preserve every signed path index; and
- field segments must resolve their string-table entry and preserve its exact
  text.

The VM may infer a primary instance owner for a POU only when every statically
referenced instance slot in that POU names the same owner. Zero owners,
multiple owners, an unknown opcode, or a truncated operand yields no inferred
owner. Scanning must consume the complete operand width of every recognized
opcode, including partial-access operands, so operand bytes are never
misinterpreted as reference instructions.

Native-call symbol descriptors are parsed once during materialization.
Successful descriptors preserve the normalized target, ordered expression and
writable-target arguments, optional formal names, conversion identity, and
resolved function POU. A parse failure is retained as a failure and produces
`vm_invalid_native_call` when selected; it is not reparsed into a different
result during execution.

##### 7.10.1 LOAD_NULL and instance-owner inference

The shared operand-width table includes `LOAD_NULL` (0x25) as a zero-operand instruction.
Owner inference must continue beyond NULL literals. A function-block body assigning NULL to
a reference and reading an instance field must resolve that field against the invoked instance,
including when two instances of the same FB are called. This corrects the former hosted scan
abort at LOAD_NULL; it is a recorded A2 execution correction, not merely loader relocation.
NULL reference semantics are unchanged; this is a product-specific bytecode owner-inference
correction, not a change to IEC reference semantics. Register lowering uses the same width table without an override.

#### 7.11 Declared Local and Static Initialization

Every stack or optimized VM call initializes its declared frame state before
the first POU instruction. The initialization plan is selected by exact POU
identity and declaration kind and is cached only for the current materialized
module. Replacing or invalidating the module clears the cache; a plan from a
different module must never be reused.

Frame layout is deterministic:

- a function or value-returning method places its return slot first;
- function and method parameters follow in declaration order;
- automatic locals follow the parameters;
- function-block parameters remain instance fields rather than frame locals;
  and
- external locals are not overwritten by automatic initialization.

A NULL return slot receives the declared default; a caller-supplied non-NULL
return slot is preserved. Parameters already copied into the frame are
preserved. Automatic locals receive their explicit initializer or declared
default. Interface-typed slots default to NULL. Class and function-block locals
create their declared instance; a class local rejects an explicit initializer,
while a function-block structure initializer applies only reviewed,
case-insensitively unique input fields. Unknown, duplicate, unwritable, or
wrong-typed initializer fields reject the call.

Function and method static locals use their qualified static owner and
initialize at most once. A method's static owner includes both the declaring
class or function block and method name. Instance-backed static storage stays
on that runtime instance; otherwise it stays in global storage. A later call
preserves the stored value and does not re-evaluate the initializer.

Initializer expressions use the same runtime value operations and stable
errors as executable expressions, with the following closed behavior:

- literals, unary and binary expressions, structures, arrays, `THIS`,
  `SUPER`, `SIZEOF`, field/index access, dereference, standard functions, and
  conversions are evaluated from the visible frame and runtime state;
- frame names resolve case-insensitively in return, parameter, then local
  order, followed by static, recursive instance, global, and retain storage;
- a local declared later than the current visible-slot boundary is not
  readable by an earlier initializer;
- array repeat groups accept non-negative integer literal counts, reject named
  repeat arguments, negative counts, and counts that do not fit the host, and
  preserve the repeated argument order;
- named fixed and variadic standard-function arguments are reordered to formal
  order, reject duplicates, unknown or unnamed entries, enforce the required
  prefix start, and reject holes in a variadic suffix;
- array index rank and bounds are checked with overflow-safe signed arithmetic;
  string and wide-string access accepts exactly one integer index; an unsigned
  or bit-string index above `i64::MAX` reports overflow and is never wrapped to
  a negative index;
- a dereferenced NULL reference reports `runtime_null_reference`, while
  incompatible values and aggregate shapes report the applicable stable type
  or bounds error; and
- `SIZEOF` returns DINT, rejects an unknown or unsupported type as a type
  mismatch, and reports overflow when the byte size does not fit DINT.
  Runtime-value sizing permits at most 128 nested values, counting the leaf;
  the first excess returns the same type-mismatch fault. Hosted and portable
  consumers use this shared traversal limit. This is a truST execution bound,
  not an IEC type-size rule.

An initialization failure identifies the owning POU and variable and aborts
the call before the first instruction. It does not leave a partially
initialized automatic frame. Existing persistent static storage and unrelated
runtime storage remain unchanged.

#### 7.12 Execution Buffers, Budget, Deadline, and Debug Location

The stack and register executors acquire reusable buffers only as an
allocation optimization. Acquisition returns empty logical stacks, frames,
registers, and temporary values. Release clears all contents and retains no
more than the documented pool limit; reuse must not expose a prior execution's
values or frame identities.

The top-level instruction budget is shared by nested VM calls. Charging exactly
the remaining budget succeeds and leaves zero; the next charge rejects with
`runtime_instruction_budget_exceeded`. A rejected charge does not underflow or
otherwise change the remaining budget. Original bytecode instruction cost is
charged before the guarded operation commits its mutation, including fused and
tier-1 forms.

Deadline polling occurs on the first guarded operation and at the documented
stack/register stride boundaries. A missing or future deadline permits
execution; an expired deadline rejects before the guarded mutation with
`runtime_deadline_exceeded`.

Debug lookup is optional and fail-soft. Valid variable metadata creates
case-preserving symbol-to-reference and first-symbol reference-to-symbol
lookups. Valid debug entries create exact `(pou_id, code_offset)` source
locations. A missing optional section or an entry whose string index is
invalid is omitted without inventing a symbol, file, line, or column. Debug
lookup never changes execution results.

### 8. Versioning

- Major version changes are breaking and must be rejected by older runtimes.
- Minor version changes may be accepted if the runtime recognizes all required sections and opcodes.
- New sections and opcodes must be added in reserved ID/opcode ranges.

Version 1.1 additions:
- TYPE_TABLE offset index for O(1) lookup
- DEBUG_STRING_TABLE for debug-only strings
- VAR_META and RETAIN_INIT sections
- Param default values (`default_const_idx`)
- STRING_TABLE entry padding
- Header CRC32 flag (`flags & 0x0001`)

### 9. Metadata Integration Requirements

The loader must populate runtime metadata from:
- RESOURCE_META -> resources, tasks, process image sizes
- IO_MAP -> I/O bindings
- STRING_TABLE -> names for tasks/programs/resources
- REF_TABLE -> FB instance references
- POU_INDEX -> method tables, inheritance, interface dispatch mapping
- VAR_META / RETAIN_INIT -> variable type metadata and retain initialization (if present)

Loading preserves resource and task names, task priority and interval,
single-trigger identity, program order, program-to-POU associations, and
function-block reference associations. Applying validated encoded bytes
materializes those associations and process-image sizes before execution.
Unsupported major versions are rejected before any live metadata, process
image, retain state, task configuration, or executable module is replaced.

### 10. Debugging Data

The DEBUG_MAP section provides a deterministic mapping between bytecode offsets and source locations. Debug entries must refer to valid POU IDs and code offsets.
For version >= 1.1, file paths are stored in DEBUG_STRING_TABLE and referenced by `file_idx`.

### 11. Planned Source-Free Initialization Format (STBC 2.0)

**Status:** implementation contract for
[Runtime Portability](34-runtime-portability.md), not an implemented format.
The current reader/writer supports 1.1. No 2.0 support or passing migration tests
are claimed by this document update. The STBC container is a truST product
format; IEC 61131-3:2013 variable/POU initialization rules (Tables 14, 19, 40,
47, and 48) and the existing §7.11 runtime behavior remain the semantic baseline.

#### 11.1 Version and compatibility boundary

Use major version 2, minor version 0 for the first complete source-free
construction/initialization contract. This is a compatibility decision, not a
claim that a separately specified minor-version design is impossible. A major
change gives existing readers a hard rejection boundary; the plan does not
depend on optional unknown 1.x sections carrying mandatory execution behavior.
Old 1.x readers reject 2.0 before live state changes. The new hosted authoring
reader supports 1.1 plus 2.0 during the following bounded release window;
source-free runtime/firmware consumers accept 2.0 only.

| Release stage | Hosted reader/import contract | Source-free runtime contract |
|---|---|---|
| Development and first production 2.0-capable release P | Read 2.0; retain 1.1 import/execution with matching authoring/construction context and the existing 1.0 metadata compatibility surface. New source-free emission defaults to 2.0. | Reject 1.x; accept only a complete supported 2.0 artifact/profile. |
| P maintenance/patch cycle | Preserve dual-major compatibility; publish the deprecation and migration instructions. A patch release does not remove 1.1 support. | Same 2.0 contract. |
| Next announced compatibility-breaking feature release Q, after that migration cycle | Retire direct 1.x runtime loading and its compatibility reader. Original source/configuration can be rebuilt to 2.0; retained P tooling can provide legacy import when the required construction context exists. | Same 2.0 contract, extended only through its own versioned rules. |

Before P is released, its release plan names the actual P/Q product versions
and migration window. Q requires documented migration instructions, preserved
legacy fixtures, qualified source/context-based regeneration of representative
applications, and stable rejection of 1.x at the retired runtime surface. If
these prerequisites fail, Q cannot claim retirement; revise and announce the
window explicitly rather than silently dropping or indefinitely promising
compatibility. This retirement is a separate breaking release change, not part
of Scope A. It does not authorize replacing the original 1.1 fixture in A.

A 1.1 artifact missing initialization data cannot always be losslessly converted
from bytes alone. Rebuild from original source/configuration or supply the
matching trusted authoring context. Report missing context as an error; do not
invent zero defaults or claim that HIR availability by itself reconstructs it.

Where the legacy hosted API supplies source-derived construction context, move
its initializer lowering into authoring/preparation and pass the resulting
validated bytecode plan to the shared engine. Compatibility is not permission
to keep an independent Expr evaluator inside the final portable executor or to
call incomplete 1.1 loading source-free.

Scope A begins by specifying the exact encoded record layouts, reserved section
IDs, counts, flags, and compatibility fixtures described below before changing
the producer/decoder. This is format-design work inside that scope, not
permission to emit an undocumented minor extension. All required construction
sections are mandatory in 2.0 even when their entry counts are zero; supported
versions, presence, uniqueness, and flags are validated explicitly.

#### 11.2 Required construction and initialization records

| Data | Required representation and validation |
|---|---|
| Storage and instance roots | Typed global/program/FB/class/static roots, declared capacities, ownership, and binding to existing reference/type/POU identities. No host pointer, HIR ID, or pre-registered runtime object is an encoded identity. Validate counts, unique identities, alignment-independent offsets, and target demand before allocating/preparing state. |
| Default and construction recipes | Reuse TYPE_TABLE/CONST_POOL for primitive/compound shapes and constants. Encode declaration-specific defaults, member/instance construction, and restart/retain rules not already represented there. TYPE_TABLE shape alone is not assumed to encode every user initializer. Include bounded construction nesting and resource demand. |
| Executable initializer index | Each record identifies its owner POU/type/declaration, execution phase, declaration order, destination type/reference, visible-frame boundary, and an ordinary STBC code range. Index initialization bodies in POU_BODIES; do not serialize Expr trees or call a second AST evaluator. |
| Static/instance once-state | Identify the owner of prepared storage and the once-initialized state. Storage exists before RUN; the value of a call-dependent initializer is still evaluated at the required first-use boundary. Do not identify a module by its address. |

The exact field widths/layout belong to this specification's Scope A amendment,
and the encoder, loader, verifier, disassembler, and format tests change together.
Existing signatures, vtables, interface slots, task records, process-image sizes,
I/O maps, parameter defaults, typed references, and RETAIN_INIT data are reused
where complete. They must not be duplicated in a parallel runtime object format
merely to avoid adapting the loader.

#### 11.3 Execution and failure behavior

Compile executable initializers with the existing bytecode lowering and run
their admitted ranges through the same VM dispatcher, value operations, stable
faults, and shared nested work/depth budgets as the POU body. Optimized plans
must preserve this contract. Preparation resolves identities and reserves
storage; it does not evaluate dynamic inputs/globals early.

Preserve §7.11's declaration order, return/parameter preservation, earlier-local
visibility, external storage exclusion, FB/class construction, static ownership,
and cold/warm restart behavior. Initialization failure follows that section's
transactional frame/state contract and occurs before the POU body. Record enough
bounded context to identify the owning POU/declaration. A compiler-free consumer
does not link the HIR initializer catalog or runtime Expr walker; remove that
execution path after its migrated behavior is covered, leaving HIR in authoring.

Validate initializer instruction boundaries, range ownership and overlaps,
control-flow destinations, typed references/imports, frame visibility, return
shape, and all required resource counts before publishing executable state.
Ordinary calls may enter other admitted POU bodies under the same checks and
budget; a raw branch cannot escape its admitted initialization range. Invalid
or missing records reject the candidate and cannot activate outputs. Keep
runtime checks needed for dynamic bounds, reference lifetimes, and faults.

#### 11.4 Scope A acceptance

Load the same saved complete artifact into a fresh headless consumer without
CompileSession, source, HIR, or prebuilt instances. Cover nonzero scalar/compound
defaults, changing-input local initialization on repeated calls, static
first-use/restart, two FB instances through an interface, parameter/return
defaults, and initializer failures. Mutate required-record presence, duplicate
IDs, types, order/visibility, body ranges/jumps, count expansion, and version
fields; none may produce a partial executable generation or output activation.

Retain the 1.1 format/host behavior corpus. Recompiling a source program into 2.0
does not by itself demonstrate equivalence: compare saved behavioral oracles and
identify deliberate numeric compatibility changes separately. Scope B/M2E uses
the exact same 2.0 fixture bytes as Scope A. Test execution follows the single
batch authorized for each actual scope; this section launches no tests.

Relocate the full decoder/validator tests with their implementation, preserving
the public host wrapper regressions and byte-serialization round trips. The
version-pair matrix includes existing 1.0 metadata cases, hosted 1.1 with/without
required context, accepted 2.0, source-free rejection of 1.x, foreign majors,
unsupported layouts, and mandatory-record failures. Keep
`tests/fixtures/oscat/core/program.stbc` as the legacy fixture and add a separate
reproducibly generated 2.0 fixture. Preserve malformed-input/error assertions;
regeneration is not permission to overwrite the pre-migration oracle.

#### 11.5 STBC 2.0 wire layout (A3)

The 2.0 container uses the 24-byte header and section table from §4. All existing
sections use the **1.1 payload layout**, including padded strings, type offsets
and parameter-default indices; minor zero in 2.0 does not select the 1.0 layout.
CRC32 is mandatory in 2.0. Header flags other than CRC32 and nonzero section
flags are rejected. Only 2.0 is supported in major 2; a future minor requires
an explicit contract. The legacy default producer remains 1.1 during A3.

Four new sections are mandatory, including when empty: STORAGE_LAYOUT (0x000D),
CONSTRUCTION_ROOTS (0x000E), INITIALIZERS (0x000F), ACCESS_BINDINGS (0x0010).
Each begins with a u32 count.
Indices are zero-based positions; optional indices use 0xFFFFFFFF. Fields are
little-endian and records contain no native alignment padding. Decoder count
bounds are checked before reservation; trailing bytes in these payloads reject.
Each table is limited to 65,536 entries and consumes the shared validation budget.
Declared construction demand is limited to 1,000,000 logical nodes per value; the
sum of top-level persistent-root demands is also limited to 1,000,000. These are
container ceilings, not MCU admission limits or native byte-size estimates.

STORAGE_LAYOUT has 40-byte records:

| Field | Wire type | Meaning |
|---|---|---|
| owner_kind | u8 | 0 global storage, 1 instance template, 2 POU frame |
| role | u8 | 0 variable, 1 program root, 2 return slot, 3 parameter, 4 static, 5 external binding, 6 compiler scratch bank, 7 edge phase, 8 native state |
| retain | u8 | Existing VAR_META retention codes 0–3 |
| flags | u8 | bit 0 constant, bit 1 input, bit 2 output, bit 3 in-out; edge-phase records use exactly bit 4 rising or bit 5 falling; other bits zero |
| owner_pou_id | u32? | POU identity; absent only for ordinary resource globals |
| name_idx | u32 | Storage name in STRING_TABLE; hidden method-static names remain qualified |
| type_id | u32? | TYPE_TABLE identity; absent only for construction-only program roots and untyped compiler scratch banks |
| slot | u32 | Global/frame offset or declared instance-template slot, not byte offset |
| ref_idx | u32? | Existing base REF_TABLE binding; absent for relative instance templates |
| default_const_idx | u32? | Typed declaration constant; absent means typed construction plus any initializer body |
| construction_nodes | u32 | Bounded logical value/instance-node demand for one construction; never a native byte size |
| related_declaration_idx | u32? | Edge-phase association with its qualified input; absent for other roles |
| source_name_idx | u32? | Original lexical name for static storage; absent for other roles |

Program roots identify their program POU template, whose member declarations
are typed. They have no expression initializer. Instance templates include
private variables, FB parameters and hidden method statics; POU_INDEX remains
signature/dispatch authority. Method-static ownership follows its method's
owner POU. Inherited layouts follow POU_INDEX parent links. Frame slots and
visibility derive from emitted LocalScope references; external declarations
consume no local slot. A scratch-bank record covers the remaining compiler-generated frame
slots, initialized to NULL; its slot plus construction_nodes equals local_ref_count and it
has no initializer, retention or PLC type. Supplied parameters and non-NULL return slots are preserved.

CONSTRUCTION_ROOTS has 24-byte records: declaration_idx:u32, binding_ref_idx:u32?,
instance_owner_id:u32?, parent_root_idx:u32?, template_pou_id:u32?, flags:u32.
Flags are zero for a bound value/instance; bit 0 denotes an inheritance-parent
instance with no independent variable binding. Root order is parent-before-child;
parent indices must be smaller than the current index. Every concrete Instance
owner used by a reference has exactly one construction mapping. The owner is an
artifact identity, never a required pre-existing host instance. Frame-local and
nested future instances are constructed from templates per activation; the table
does not enumerate future calls. Roots and templates come from declarations,
not a snapshot of values after program execution.

INITIALIZERS has 60-byte records: declaration_idx:u32?, owner_pou_id:u32?,
result_ref_idx:u32, code_offset:u32, code_length:u32, visible_local_count:u32,
visible_static_count:u32, phase:u8, once_scope:u8, stage:u8, trigger:u8,
target_idx:u32?, partial_kind:u8, target_kind:u8, target_reserved:[u8;2], partial_index:u32,
context_initializer_idx:u32?, recipe_type_id:u32?, recipe_member_idx:u32?,
body_kind:u8, recipe_reserved:[u8;3].
Body kinds are 0 action, 1 type-default recipe and 2 member-default recipe.
Recipe descriptors use phase 7, stage Default, no once-state, declaration or
configuration target. They are callable value bodies, never scheduled lifecycle
actions. Action descriptors have no recipe type/member identity. Their context
index is absent for a canonical context, or names an earlier canonical action
with the same owner, phase, stage, trigger, once scope and visibility frontier.
Context chains and cycles are rejected. Recipes name that canonical action.
Sharing applies to immutable code and descriptors only; each invocation has fresh
staging and preserves expression evaluation frequency and lifecycle once-state. A type recipe names the exact TYPE_TABLE entry, including aliases. A
member recipe additionally names the zero-based STRUCT member or UNION variant on that entry.
Recipe result types must match that type or member respectively. Reserved bytes
are zero. There is at most one recipe per (root context, type, optional member).
Phases are 0 resource startup, 1 instance construction, 2 frame entry,
3 static first use, 4 return-slot default, 5 configuration action,
6 omitted-parameter default, 7 on-demand typed value default. Once scopes are 0 none, 1 module,
2 current instance. Trigger 0 selects the ordinary owner lifecycle; trigger 1
selects a function static's first invocation after restart. Trigger 1 is legal
only for module-owned function-static actions and their callable recipes. Recipes
inherit their root action's trigger but are never scheduled independently. Each
function-static declaration has a separate default/optional-explicit action pair
for each trigger. Frame entry repeats on each call; static first use is keyed
by declaration and module/instance ownership, not module address. Cold/warm
restart follows the declared retention contract and §7.11.

A 2.0-only REF_TABLE location 5, InitializerResult, addresses a staged typed
result: owner_id is the INITIALIZERS index and base offset is zero. Its base
VAR_META record carries the declaration type for declaration actions, or the selected
target type for configuration actions, or the declared recipe result type, and
no retained/constant initializer.
Only the owning initializer may use these references; ordinary POU bodies and
other initializers reject them. Derived field/index paths are allowed. Addresses
into this temporary result cannot escape through assignments, returns or calls.
Frame-local references stored as result values must also satisfy the final
storage declaration's lifetime, not merely the temporary result's lifetime.

Each initializer range contains ordinary STBC instructions operating on the
staged result and the original invocation context. It starts and finishes with
an empty operand stack. Branches stay within that range; it overlaps neither
another initializer nor an ordinary POU body. Initialization preserves the existing evaluation order, default precedence and
destination-type coercion. For aggregate overrides, explicit RHS expressions are
evaluated before merging type/member defaults; precedence alone must not reorder
observable faults or function calls. Intrinsic TYPE_TABLE construction precedes
initializer execution without evaluating user expressions. Aggregate lowering captures values and invokes the shared typed construction
helpers specified in §11.5.8; no Expr tree or alternate interpreter is serialized.
Compile-time evaluation cannot replace a dynamic initializer or its faults.
Default expressions may be specialized per declaration context, preserving
frequency and ordering, with expansion checked before allocating/emitting code.

Earlier frame locals are visible; later locals do not shadow outer storage.
Static storage reservation does not make later uninitialized statics visible.
Result commit belongs to A4's transaction boundary, after successful execution;
A3 validation is not evidence that initialization or hardware outputs ran.

These are product-format mechanisms for the initialization semantics in IEC
61131-3 Ed.3 §6.4.4.1.2, §6.5.1 and the existing §7.11 contract. A3 does not
change legacy execution: the existing hosted executor rejects 2.0 explicitly
until A4 supports its construction and initialization requirements. Portable
source-free validation rejects every 1.x artifact, even if HIR is available.

#### 11.5.1 Authoring boundary and startup order

The 2.0 producer selects source-free authoring before runtime materialization.
Parsing, semantic analysis and expression lowering are shared with the legacy
producer. Their owned result retains global and configuration initializer
expressions, original program template identities and configured instance names.
No initializer is evaluated merely to obtain a stored value for serialization;
compile-time constants, type bounds and task configuration remain compile-time work.
Parameter defaults are subject to the same rule.

Startup preserves the existing observable ordering: global defaults (including
FB construction overrides), scalar global explicit initializers, program instance
construction, ordered VAR_CONFIG actions, then module static initialization where
required by the existing contract. A configuration action targets one concrete
binding; its RHS resolves names in global scope, not in the target instance's
scope. Repeated, indexed, partial and direct-I/O configuration targets must retain
their original order and access restrictions. A per-template default cannot stand
in for a per-instance configuration action. Configuration actions use phase 5 in INITIALIZERS, in table order. Their
owner_pou_id is absent, visibility counts and once_scope are zero, and
target_kind 1 selects a REF_TABLE target_idx; kind 2 selects a STRING_TABLE
direct-address target_idx. Direct addresses preserve area, width, bit and hierarchical
path information using the shared IoAddress syntax, with no wildcard. declaration_idx identifies
the owning storage declaration, or is absent for direct I/O. The result VAR_META
type is the selected target type, which may differ from its containing declaration.
partial_kind is 0 none, 1 bit, 2 byte, 3 word or 4 double-word; partial_index selects
the part. A partial commit merges into the existing target value. Repeated configuration actions
remain distinct. Declaration initialization has one default action (stage 0) and
an optional explicit action (stage 1); configuration actions use stage 1.
Address-only configuration entries emit I/O bindings without executable actions.
For all other phases target_kind is 0, target_idx is absent and partial fields are zero.
Direct-address actions have no declaration index and no separate partial access.
As in the hosted configuration initializer, input-image addresses can be initialized
at startup; this does not authorize input writes during PLC execution. Reserved
bytes are always zero. Omitted-parameter plans use phase 6, execute in parameter
order only for unsupplied arguments, and cannot see later parameter slots.

In 2.0, task program names resolve to configured ProgramRoot declaration names,
whose root records identify the executable program template. The wire representation separates program templates from mutable root state.
A3 retains the producer's existing restriction against multiple configured instances
of one PROGRAM type; lifting that restriction requires a separate behavior decision. The
legacy 1.x rule continues to resolve task program names directly to program POUs.

#### 11.5.2 Edge-qualified inputs

A PROGRAM/FB edge-qualified BOOL input has one associated EdgePhase declaration
in the same instance template. The phase declaration identifies its input through
related_declaration_idx; exactly one of flags bit 4 (rising) or bit 5 (falling) is set.
The hidden declaration is BOOL and has a typed constant seed: FALSE for rising,
TRUE for falling. Phase storage contributes to construction demand and is allocated
before execution, never lazily. Its retention policy matches the associated input;
program warm restart preserves the previous-input state when that input is retained.

At each invocation, after argument binding and before the body, the shared engine
updates the phase and substitutes the edge pulse. It restores raw inputs before
output copy-back on both success and body failure; a body failure does not roll back
the phase update. The binding uses original POU identity and concrete instance
identity. This corrects the legacy name-lookup defect for renamed configured
programs, whose qualifiers can otherwise be missed. A native renamed-program case
must pin that distinction before A4 claims execution. A3 validates and serializes
these associations; it does not claim to have executed edge transactions.

#### 11.5.3 Initialization action ordering

An initializer record is one ordered action, not a fused default-plus-override
recipe. Within an invocation boundary, the producer emits actions in execution
order; the consumer must not sort them by declaration or stage. Each value declaration
other than a program root, external binding, scratch bank or NativeState slot has a
default action and may have an explicit action. A default action may have an empty
code range when intrinsic/constant construction supplies the complete value. Program
roots and scratch banks use intrinsic construction; external bindings allocate no
value; NativeState slots retain the first-use protocol in §11.5.5. Explicit actions follow their
own default actions, but other declarations may intervene. The default and explicit
actions of a static declaration share one once-state, completed only after the
whole initialization sequence succeeds.

Existing construction groups preserve these orders:

- Globals: all defaults/construction, with immediate global FB overrides; then
  scalar global explicit initializers.
- FB instance: inherited parent construction, parameter group, variable group,
  then method-static group.
- Each FB parameter or instance-variable group: all defaults/nested construction,
  then explicit initializers.
- Method statics: defaults across all methods/statics, then explicit initializers
  across all methods/statics.
- Function statics: defaults across all functions/statics, then explicit
  initializers across all functions/statics, in original declaration order.
- Frame entry: bind/default parameters, prepare the return slot, initialize
  required statics, then automatic locals in declaration order. Group insertion
  order in the artifact does not replace these lifecycle dependencies.

Nested construction happens at its original point in the owning group. Moving
all defaults ahead of all explicit actions across the entire resource changes
behavior and is prohibited. Values committed by earlier actions are visible at
the same boundaries as in the current constructor. Aggregate expressions retain
the separate evaluation/coercion ordering described above.

Edge-phase retention in this format preserves the existing PROGRAM warm-restart
contract. It does not assert that hidden phase state is included in serialized
retain snapshots or that nested FB phases currently survive warm restart. An edge
transaction applies only the invoked POU's qualifier set; it does not automatically
apply ancestor qualifiers or run twice across dispatcher wrappers. Cold restart
and non-retained program inputs reset their associated phase state.

#### 11.5.4 Generic counter state

2.0 reserves primitive descriptor id `0x0100` for the existing native counter
`ANY_INT` state constraint. It is an internal native-library slot, not a new IEC
elementary type. max_length is zero; intrinsic construction yields NULL until the
native counter binds its integer width from the call. Its construction demand is
one Value slot. It has no constant-pool payload and is rejected in legacy 1.x.
Construction and shared assignment validation accept NULL or a concrete SINT,
INT, DINT, LINT, USINT, UINT, UDINT or ULINT value for this descriptor, including
through an alias. Validation preserves the concrete integer width; it does not
coerce Boolean, bit-string, real, enumeration, reference or aggregate values into
counter state. This is the product's internal native-state representation contract.
The source producer uses it only for existing generic native-counter declarations;
ordinary unsupported generic source declarations remain rejected. Execution still
requires A4's matching native import admission contract.

#### 11.5.5 Native function-block state reservation

Storage role 8 reserves an internal native FB state slot with a concrete declared
type, zero flags and no initializer or constant seed. It belongs to an FB template
and contributes one logical storage slot (plus any declared value demand). The
slot initially has no initialized value. First-use initialization by the admitted
native import fills that existing slot; it must not allocate or reinterpret NULL
as an already-initialized timestamp. This preserves the first-call timer timestamp
and existing trigger/counter first-use seeds.

The producer obtains internal slot names/types from the same built-in FB registry
that owns the native implementation. A4 import admission must match the declared
native state layout before execution; a successful A3 format validation is not
native-import admission. Hidden native state is not exposed as a user parameter.

A4 separately caps cumulative typed construction nodes for each construction,
restart, cycle or between-cycle access entry. Scalar and aggregate helper visits
and temporary aggregate-builder slots consume this budget; alias traversal adds
no value and an FB/class identity is charged only at reservation. Retiring a
frame or staging value does not refund the entry's budget. Fixed declarations
and persistent roots are also checked against the profile using independently
validated TYPE_TABLE construction demand before instance execution. Copy work
and allocation demand have their separate pre-copy charges.


#### 11.5.6 Access aliases

`ACCESS_BINDINGS` (section `0x0010`) is mandatory in 2.0, including an empty table
when no VAR_ACCESS declaration exists. Its u32 count is followed by fixed 20-byte
records: name_idx:u32, type_id:u32, ref_idx:u32, partial_kind:u8, flags:u8,
reserved:u16, partial_index:u32. The same 65,536-record bound applies.

Names are unique case-insensitively. ref_idx identifies the underlying storage;
type_id is the exposed value type (the selected part's type for partial access).
partial_kind uses the initializer partial-access codes; partial_index is zero for
whole-value aliases. flags bit 0 means writable; other bits and reserved are zero.
Read-only aliases remain readable but reject writes through the alias. Aliases do
not allocate value slots or override ordinary declaration ownership/permissions.
The producer retains existing VAR_TEMP/VAR_EXTERNAL/VAR_IN_OUT exposure rejection
and constant read-only rules. Source-free host access/debug surfaces restore this
map from the artifact; IO_MAP alone does not encode these names or permissions.

#### 11.5.7 Disabled-call result defaults

2.0 adds `0x65 DEFAULT_VALUE u32 initializer_id` (four operand bytes, stack effect
0 → 1). Its target is a phase-7 default action with no declaration/POU owner,
no target index, zero visibility counts, stage 0 and no once-state. The action's
result VAR_META supplies its type. The same dispatcher executes its initializer
range with an empty private operand stack and returns its staged value to the
caller; nested evaluation shares instruction, deadline and call-depth budgets.
Only the completed value escapes, never an address into the temporary result.
Legacy 1.x validation rejects this opcode.

The 2.0 producer uses this operation in the disabled branch of a call with EN.
It evaluates the declared return-type default in global context each time that
branch is taken, without binding/invoking the callee or evaluating its ordinary
arguments. An enabled call does not evaluate the disabled-result recipe. Calls
without a value result keep their existing NULL result. Method result selection
uses the resolved receiver type; unrelated methods sharing a name do not supply
one another's defaults.

This is an explicit 2.0 compatibility decision: legacy 1.x encoding currently
captures the disabled result at authoring time, even when EN is true. Source-free
2.0 cannot depend on initialized authoring storage. It preserves declared defaults
rather than substituting intrinsic zero/NULL, and moves their evaluation/faults to
the disabled invocation. A3 must prove emission without evaluation and the version
boundary; A4 must pin enabled/disabled frequency and fault behavior before claiming
execution equivalence for this format.

Action boundaries are the resource startup sequence, a concrete instance/template
construction, one staged invocation frame, or an on-demand value evaluation.
Earlier automatic-local results become visible through the staged frame; final
frame commit remains governed by §7.11. For a static declaration, default/explicit
stages share a staged result and the once-state is completed only after that
declaration's final stage succeeds. A failed explicit stage discards its staged
default and leaves that declaration uninitialized for a later attempt. Earlier
successfully initialized static declarations remain initialized.

#### 11.5.8 Typed initialization and aggregate values

Default recipes execute ordinary STBC through the same dispatcher as POU bodies.
TYPE_TABLE-driven construction and coercion are shared value operations, not a
second expression interpreter. Recipes return activation-local staged values;
they never commit declarations or change static-once state. Nested calls cannot
reuse the caller's staged result. The context index identifies lexical lookup
rules, not a mutable frame, instance identity or singleton result slot.

Type/member defaults see globals and the current construction instance, without
frame-local or static aliases. Explicit declaration expressions retain their
original visibility frontier. A recipe preserves its root action's current
instance and construction frontier, including which members already exist;
reservation alone does not expose an uninitialized later member. An on-demand
disabled-call action supplies global context. A recipe cannot select an unrelated
root context. Callback nesting counts toward the same call-depth limit, and
helper traversal, copying and allocation consume admitted work and memory limits
in addition to dispatcher instruction fuel and deadlines.

The following 2.0-only instructions are restricted to initializer bodies. Each
has a four-byte operand. Ordinary POU bodies and all 1.x modules reject them.

| Opcode | Operand | Stack effect | Operation |
| --- | --- | --- | --- |
| `0x66 DEFAULT_TYPED` | TYPE_TABLE id | 0 → 1 | Construct the declared default, invoking the exact alias/type default recipe before following aliases. |
| `0x67 COERCE_INIT_VALUE` | TYPE_TABLE id | 1 → 1 | Coerce an already evaluated value, including member defaults and missing array tails, without a top-level type-default merge. |
| `0x68 APPLY_INIT_VALUE` | TYPE_TABLE id | 1 → 1 | Apply an explicit initializer: perform the applicable exact-type aggregate-default merge, then shared coercion. |
| `0x69 ARRAY_NEW` | element count | 0 → 1 | Allocate a temporary untyped aggregate of exactly that many NULL elements, bounded by admitted construction memory. |
| `0x6A ARRAY_SET` | zero-based element index | 2 → 1 | Consume aggregate then value, replace the selected element and return the aggregate; reject non-arrays or an out-of-range index. |
| `0x6B STRUCT_NEW` | reserved zero | 0 → 1 | Produce an empty temporary struct value. |
| `0x6C STRUCT_SET` | STRING_TABLE name id | 2 → 1 | Consume aggregate then value, insert the named member and return the aggregate; reject a non-struct or a duplicate case-insensitive member name. |

These operations compose one shared typed default/coercion implementation.
They must not implement three independent recursive algorithms. Type-default
recipes evaluate their expression then use COERCE_INIT_VALUE; member recipes
with explicit defaults use APPLY_INIT_VALUE. Absent expressions use DEFAULT_TYPED.
Recipe lookup uses the root action context and exact type/member identity.
Admission rejects invalid declared associations, member/result types and foreign
context identities. Recipe absence denotes an intrinsic default; producer
regressions must separately prove that every source default is represented.
Union recipes use TYPE_TABLE variant order and the existing all-variant default
materialization semantics. Runtime
construction-cycle detection and admitted callback depth also apply: a cyclic
default cannot bypass limits by crossing a helper/dispatcher boundary.

Aggregate builders evaluate each explicit expression exactly once in source
order before typed coercion begins. Coercion preserves canonical destination
member order when a type-default merge creates that order. Missing array tails
evaluate defaults separately from provided elements; ordinary array defaults
evaluate each element's default independently. Array repeat expansion is bounded
before code emission and retains repeated-expression frequency. FB member
overrides keep their distinct sequential evaluate/coerce/write behavior.
A failure discards the staged value without partially committing its destination.
Initializer native calls remain restricted to §7.11 standard functions and
conversions; recipes do not authorize user-function, method or FB calls.

This replaces the unshipped draft REVERSE_VALUES instruction. A3 validates and
emits the representation; A4 implements and proves its execution before any
source-free application is admitted for operation.

Static reference checks reject definite escapes but cannot prove the contents of
values loaded from mutable slots. A4 must recursively check references in every
escaping write and commit, including aggregate members, recipe returns and values
introduced by coercion callbacks. Valid longer-lived references remain valid;
storing an aggregate in scratch and reloading it cannot erase its runtime lifetime.
These checks are mandatory before execution support, with scratch/reload and
callback-introduced-reference regressions in the A4 oracle.

Initializer execution uses a restricted storage view: reads resolve through the
root action's visibility frontier; default recipes exclude frame/static aliases
and unavailable construction members even when bytecode encodes a direct reference.
Writes may target only the activation's staged result and admitted scratch, never
unrelated globals or instances. Native output arguments use the same restriction.
The lifecycle owner alone commits the completed result. Admission rejects definite
foreign staged-result references. Every dynamic store destination and native
writable argument must be proven to derive from the current staged result on
every reachable control-flow path; a global, instance, frame, loaded or unknown
destination is rejected. A mixed staging/non-staging merge is not proof. Native
argument encodings must match the operand count; input arguments cannot expose
staging references. Dynamic values still pass the runtime visibility/write checks. A4 must prove those checks before
admitting source-free execution.

For source-free execution, runtime protection failures have distinct stable identities:
`runtime_reference_lifetime`, `runtime_visibility_violation`,
`runtime_staging_violation`, `runtime_constant_write`,
`runtime_program_root_replacement` and `runtime_invalid_alias`. Actual value-type
mismatches and null dereferences retain their existing type/null codes. A valid
artifact unsupported by the selected profile reports `runtime_profile_unsupported`;
preparation-limit exhaustion reports `runtime_preparation_limit`. Decoder allocation
and work exhaustion report `bytecode_decode_memory_limit` and
`bytecode_decode_work_limit`, rather than malformed-header errors. Defensive
violations of already-admitted execution metadata report `runtime_invalid_execution_state`.
These are truST diagnostic contracts, not changes to IEC language rules.

These runtime checks supplement bytecode admission and do not grant access merely
because a value was previously validated. A reference to an ancestor local remains
live while a nested call executes, and writes through it update the same caller local.

#### 11.5.9 Function-static startup and restart contexts

Initial source construction defaults all function statics, then evaluates their
explicit expressions, using globals without invocation parameters or local/static
aliases. This is trigger 0. Method statics likewise initialize during their owning
instance construction in that instance's context, without method-frame aliases.

The existing restart path reconstructs globals and programs but does not recreate
function-static backing slots. Consequently the first invocation after restart
initializes that function's statics in declaration order with bound parameters
and earlier initialized static aliases visible. This is trigger 1, with separate
compiled bodies/recipes and shared per-declaration once-state. Automatic locals
still follow static initialization. A failed static initializer preserves earlier
completed statics and leaves the failing declaration uninitialized.

STBC 2.0 preserves this observable distinction explicitly. It does not normalize
restart into eager startup. Admission requires both trigger plans for each
module-owned function static, validates their visibility separately and rejects
trigger 1 on unrelated declarations. A3 source tests pin emitted contexts; A4
must replay initial load, warm/cold restart and a failing first invocation before
claiming equivalent static initialization.

The opt-in STBC 2.0 producer also accepts explicit member overrides on
function-static FB instances. It compiles them into the same staged Explicit
actions for Ordinary and AfterRestart triggers; a failed override does not
publish partially updated member fields. This is a 2.0 authoring capability:
legacy 1.x hosted construction continues to reject explicit initializers on
function-static FB instances. Explicit function-static class-instance initializers
remain unsupported. This producer distinction is a truST contract, not a change
to IEC source lifetime rules.

Inherited method statics belong to their declaring template and its concrete
ancestor instance. STBC 2.0 resolves that physical owner instead of creating a
second hidden slot on a derived receiver. This corrects the legacy non-recursive
first-use lookup; the A4 inherited-method regression must pin single-slot identity
and initializer frequency alongside the restart cases above.

For automatic locals, omitted call-local parameters and after-restart statics,
an explicit scalar/aggregate initializer replaces the separate declared-default
evaluation. Their Default action is intrinsic-only (empty code); the Explicit
action performs APPLY_INIT_VALUE and therefore retains its own aggregate-default
callbacks. An overridden scalar alias default must not run or fault. FB-valued
locals still construct the nested instance before sequential member overrides.
Return-slot initialization preserves the legacy intrinsic default, without
executing user type/member default expressions.

A source-free TASK SINGLE name must resolve to a declared BOOL global, including
an alias of BOOL. Missing names and other types reject admission. Flat direct
VAR_CONFIG writes contribute their complete extent to RESOURCE_META; admission
rejects writes outside that image. Hierarchical addresses retain their separate
address space and typed-value semantics. Global CONSTANT qualifiers survive
lowering into STORAGE_LAYOUT and cannot be bypassed through configuration or
writable access aliases.

A 2.0 artifact describes one selected resource, matching the existing Runtime
composition. RESOURCE_META therefore contains exactly one resource; construction
and direct initialization identities belong to it. Each flat image size retains
the existing 16 MiB container ceiling, with stricter limits imposed by admission
profiles. This avoids ambiguous cross-resource construction or I/O ownership.

The hosted opt-in entry point is
`CompileSession::build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)`.
The existing `build_bytecode_module` and byte-array helpers keep producing 1.1.
The explicit selector accepts only the supported producer contracts 1.1 and 2.0;
future minors and foreign majors fail without evaluating startup code. The
tracked OSCAT 1.1 fixture remains unchanged. The separately named
`tests/fixtures/portability/stbc-2.0/program-v2.stbc` is regenerated by the Rust
`portability_fixture` example and checked against its source and portable reader.

Static storage records additionally carry source_name_idx, the original lexical
variable name; other roles leave it absent. name_idx remains the distinct storage
identity, including private backing names. Known FB-call signature lookup in 2.0
uses scoped declarations (frame, static, current/inherited instance, access/global)
rather than selecting a concrete instance from VAR_META names. This covers future
frame instances and rootless templates. Dynamic receiver types are checked by the
runtime rather than guessed from an unrelated global or POU name.

Source-free construction accepts only the primitive IDs defined by this spec
(1–27 and the native-state marker 0x0100). max_length is zero for non-string
primitives. Unknown primitive IDs are not executable construction descriptors.

Storage slots are dense from zero within each physical owner. A scratch-bank
record covers its entire contiguous tail; an arbitrary large slot cannot be
represented as a one-node construction demand. Typed frame declarations must
match their base VAR_META type. Static lexical names are unique within their
semantic POU owner, independently of private storage names. Access aliases and
resource-global storage names cannot collide case-insensitively in a 2.0 artifact.

Frame layout roles agree with POU_INDEX: its optional return slot is first,
followed by parameters in signature order, then automatic locals and any scratch
tail. Parameter declaration types, names and direction flags match the signature;
a normal-variable record cannot replace a parameter and overwrite a supplied
argument during initialization.

In 2.0, an INSTANCE reference names its concrete construction-root identity;
SELF/SUPER instructions express invocation-relative access. A POU may therefore
use explicit references to several roots, for example through access aliases.
It must not infer or remap a primary instance owner from those references. The
legacy one-owner validation/materialization rule remains confined to 1.x.

Static visibility counts are checked against the declaration table, not accepted
as producer assertions. Ordinary static actions have a zero frontier; after-restart
function-static actions expose exactly the preceding statics of that declaring
function. Other declaration actions carry the owner's complete static count;
their lifecycle and restricted storage view still determine which initialized
values may be read. Callable type/member recipes retain zero frame/static-alias
visibility. The validator builds the owner/declaration index once and performs
bounded prefix lookups for these checks.

### 11.5.10 Source admission and typed binding preservation

The opt-in 2.0 producer uses the same source diagnostics as the legacy producer.
Executable initializer support is not permission to accept mutable scalar
initializers that the IEC source checker rejects. The changing-input fixture uses
a method-local `REF_TO INT := REF(history[delta])`: the addressed array belongs
to the persistent FB instance, while its index changes between calls. This is the
reference-initialization case permitted by IEC 61131-3 Ed.3 §6.4.4.10.2, respecting
§6.4.4.10.3's prohibition on references to temporary storage. Scalar initializer
fixtures intended to fault at runtime must first satisfy the source checker;
the authoring boundary must retain the accepted expression without evaluating it.

Local VAR_META retains §7.11's reserved `@local/<pou>/<slot>/<label>` identity.
The 2.0 producer reuses the local-scope metadata rather than inventing an alternate
name. Initializer-result references are distinct from locals and do not use that
reserved prefix.

For STRING direct I/O, the hosted binding's normalized STRING tag and its
`Bytes(n)` extent are separate facts. A 2.0 IO_MAP entry must select a bounded
STRING TYPE_TABLE entry with capacity `n`, including aliases and nested aggregate
leaves. Serializing `%QB<offset>` must not discard the capacity. The existing
hosted normalized tag is preserved, and admission still rejects an unbounded
string or an image smaller than the complete declared binding.

#### 11.5.11 Compact construction metadata and stable tooling

The 2.0 producer emits standard-library block templates only when reachable from
application roots, callable frames and their transitive declared types, including
array/aggregate members, aliases, inheritance and references. User-defined entry
points remain available. Type/member recipes are interned per equivalent canonical
visibility context and exact type/member identity. Sharing never caches a value or
changes when an expression executes. Admission rejects cross-context recipes.

Configuration and access paths resolve every leading identifier: a recognized
configuration/resource prefix may qualify a program or global, but an unknown
prefix must not be skipped to bind a later matching global. This tightening is
limited to the opt-in 2.0 producer; legacy 1.1 behavior remains unchanged.

Construction disassembly uses explicit field names and stable enum spellings, not
derived Rust Debug output. Native source-authoring coverage includes edge-qualified
inputs, retained declarations, partial-access aliases/configuration writes and
classes. The saved A4 replay fixture includes both an edge input and retained state.
These records preserve IEC 61131-3 Ed.3 declaration semantics; artifact admission
is not evidence of initializer execution or hardware operation.

The lowered/wire construction model also represents partial configuration targets.
Current source VAR_CONFIG admission remains limited to symbolic variable access
paths (IEC 61131-3 Ed.3 Table 62); a `%B` selection in such a target is rejected.
Native tests distinguish source partial-access aliases and whole-value configuration
writes from explicitly constructed lowered partial configuration actions. A wire
capability does not silently expand accepted IEC source syntax.

#### 11.5.12 Ordinary aggregate assignments

The source-free external global-write API accepts mutable resource-variable
declarations only. Program roots and function-static lifecycle slots cannot be
replaced through that engineering API; VAR_ACCESS retains its declared binding
and permission checks.

Source-free ordinary assignments, call bindings and external engineering writes
validate the complete destination TYPE_TABLE shape before committing. Array bounds
and element count (wildcard formal dimensions preserve the supplied concrete bounds), structure/union identity and complete case-insensitive member
sets, enum type/name/value identity, and nested reference/instance compatibility
must agree. Scalar/string normalization applies recursively, including declared
string capacities and subranges. This operation never evaluates type/member
initializers or fills missing members with defaults. A failed assignment leaves
the destination unchanged. Recursive traversal and copied values consume the
admitted work/allocation limits, and reference lifetime checks remain mandatory.

Partial field/element writes charge any copy-on-write structure backing and owned
array fields copied by that operation before mutation, including newly shared
children after an ancestor copy. String-element replacement charges its character
buffer and replacement text. These failures retain their budget/deadline error;
a multi-output copyback failure restores every destination in that output group.

The source-free runtime enforces STORAGE_LAYOUT constant declarations at every
ordinary store boundary, including static/dynamic bytecode stores and native
output copyback. Selecting a member or element does not make constant storage
writable. Compiler acceptance is not an authority to bypass this check: a
structurally admitted forged artifact faults before modifying the constant.
Private initializer-result staging and explicit lifecycle declaration commits
remain the construction paths; ordinary execution cannot use them to overwrite
an initialized constant.
Construction-only program-root identities also reject ordinary stores, even
when their reference has no TYPE_TABLE identity. Only lifecycle construction
installs or replaces these roots.

External binding records allocate no physical slot. Runtime slot/type/visibility
and constant-permission lookup excludes them, even if an admitted External record
carries the same owner/slot as a physical declaration. Record ordering cannot
change the physical declaration's type, initialization status or write permission.

Typed reference checks resolve each selected path segment from the admitted
construction declarations as well as TYPE_TABLE. POU members include inherited
members; an untyped program-root binding starts from its declared program template.
External aliases cannot replace a physical member's type. Equivalent direct-instance
and global-root-plus-member references have the same selected type. Traversal uses
the active operation's work/deadline budget; restart queries charge the staged
replacement, not the state being preserved.
