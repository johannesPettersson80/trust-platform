# Bytecode Format

**Status:** Implemented container + execution format. The runtime validates STBC sections and executes bytecode instructions through the VM backend.

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

##### 4.6.1 Exact Boundaries and Counter Arithmetic

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

### 11. Future Tasks (Deferred)

No deferred items at this time.
