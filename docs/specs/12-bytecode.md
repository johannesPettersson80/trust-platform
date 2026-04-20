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
- Offsets must be 4-byte aligned.
- Sections must not overlap.
- Unknown section IDs are ignored unless marked required by the runtime configuration.

#### 4.3 Section Flags

| Bit | Name | Meaning |
|-----|------|---------|
| `0x0001` | `COMPRESSED_ZSTD` | section payload is zstd compressed |
| all others | reserved | ignore if unknown |

#### 4.4 Header Flags

| Bit | Name | Meaning |
|-----|------|---------|
| `0x0001` | `CRC32` | header `checksum` is CRC32 of the section table and section payloads |

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
| 0x000B | VAR_META | No | Variable metadata (globals) |
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
- STRING/WSTRING: `u32 string_idx` (string table reference).
- TIME/LTIME: `i64` nanoseconds.
- DATE/TOD/DT: `i64` ticks in the runtime `DateTimeProfile` resolution.
- LDATE/LTOD/LDT: `i64` nanoseconds.
- REFERENCE: `u32 ref_idx` or `0xFFFFFFFF` for NULL.
- ARRAY: `u32 elem_count` followed by `elem_count` element constant payloads.
- STRUCT/UNION: `u32 field_count` followed by `field_count` field constant payloads in field order.
- ENUM: `i64` numeric value.

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
supported minor version. Default values are applied only for `IN` parameters.

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

VarMeta entries describe global variables and their retain policies.

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

#### 7.3 Baseline Instruction Set

Control flow:
- `0x00 NOP`
- `0x01 HALT`
- `0x02 JMP i32`
- `0x03 JMP_TRUE i32` (pop bool)
- `0x04 JMP_FALSE i32` (pop bool)
- `0x05 CALL u32` (POU id)
- `0x06 RET`
- `0x07 CALL_METHOD u32` (pop instance ref, call method by vtable slot)
- `0x08 CALL_VIRTUAL u32 u32` (interface_type_id, interface_method_slot)

Stack and constants:
- `0x10 CONST u32` (const pool index)
- `0x11 DUP`
- `0x12 POP`
- `0x13 SWAP`
- `0x14 OVER` (a b -- a b a)
- `0x15 ROT` (a b c -- b c a)
- `0x16 PICK u8` (copy nth item to top; 0 = top)

Static references:
- `0x20 LOAD_REF u32` (ref table index)
- `0x21 STORE_REF u32` (ref table index)
- `0x22 PUSH_REF u32` (push `Value::Reference`)
- `0x23 PUSH_SELF` (push `THIS`/`SELF` reference in a method)

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
- `0x4A SHL`
- `0x4B SHR`
- `0x4C EXPT`
- `0x4D ROL`
- `0x4E ROR`

Comparison:
- `0x50 EQ`
- `0x51 NE`
- `0x52 LT`
- `0x53 LE`
- `0x54 GT`
- `0x55 GE`

Type conversion:
- `0x60 CAST u32` (type id)

Standard library:
- `0x70 CALL_STD u32` (standard function id; resolved by the runtime stdlib)

Reserved opcode ranges:
- `0x80-0xEF` reserved for future core extensions.
- `0xF0-0xFF` vendor/experimental.

#### 7.4 Fault Semantics

The executor must fault on:
- Type mismatches (e.g., BOOL in arithmetic)
- Invalid references or out-of-bounds indexes
- Divide by zero
- FOR loop step expressions that evaluate to 0 (encoder emits a step==0 guard that executes `HALT` before loop entry)
- Invalid jump targets
- Method/interface dispatch on NULL or incompatible references

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
- VAR_META / RETAIN_INIT -> global variable metadata and retain initialization (if present)

### 10. Debugging Data

The DEBUG_MAP section provides a deterministic mapping between bytecode offsets and source locations. Debug entries must refer to valid POU IDs and code offsets.
For version >= 1.1, file paths are stored in DEBUG_STRING_TABLE and referenced by `file_idx`.

### 11. Future Tasks (Deferred)

No deferred items at this time.
