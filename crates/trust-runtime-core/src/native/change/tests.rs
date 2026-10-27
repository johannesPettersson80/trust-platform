//! The online change of a compiled program's values (specification 44 §12.2; specification 11
//! §6.7.1), over two images written by hand, the running program's and the new one's, each laid
//! out as the build lays a program out (specification 44 §5, §7.2): every variable at the next
//! offset of its area that its alignment gives, in declaration order, and a structure's or an
//! instance's members likewise. Each test's comment gives the declarations and where they lie.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use trust_plc_abi::{
    AREA_GLOBALS, AREA_INPUTS, AREA_INSTANCES, AREA_MARKERS, AREA_OUTPUTS, AREA_RETAIN,
};

use super::carry;
use super::fit::convert;
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::native::change::CarriedValueEvent;
use crate::native::image::{
    type_kind as kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, Image,
    ImageWriter, IoBindRecord, MemberDetail, MemoryFigure, Record, RetainRecord, SectionId,
    StmtRecord, StringPool, SymbolRecord, Target, TaskRecord, TypeRecord, NONE, SYMBOL_CONSTANT,
    SYMBOL_INSTANCE, SYMBOL_PROGRAM, SYMBOL_RETAINED, TABLE_PREFIX_BYTES,
};
use crate::native::memory::ProgramMemory;
use crate::native::values::width;
use crate::value::Value;

mod aggregates;
mod references;

/// One member of a structure or an instance type: its name, its type, its offset and its flags.
#[derive(Clone)]
struct Field {
    name: String,
    ty: u32,
    offset: u32,
    flags: u8,
}

/// A symbol before the table is sorted.
struct Row {
    canonical: String,
    display: String,
    ty: u32,
    area: u8,
    flags: u8,
    offset: u32,
    size: u32,
    parent: Option<String>,
}

/// A program written by hand: its types, the symbols of its variables and their members, and the
/// bytes of its areas.
struct Program {
    strings: StringPool,
    /// Each type's record and, for a type with a detail, where it starts in `details`.
    types: Vec<(TypeRecord, Option<usize>)>,
    details: Vec<u8>,
    /// The members of each structure, union, function block, class and program type.
    fields: BTreeMap<u32, Vec<Field>>,
    rows: Vec<Row>,
    /// Each area's bytes by its id; index 0 is no area.
    areas: [Vec<u8>; 8],
}

/// `items` as a count of a record.
fn count(items: usize) -> u32 {
    u32::try_from(items).expect("a few items")
}

impl Program {
    fn new() -> Self {
        Self {
            strings: StringPool::new(),
            types: Vec::new(),
            details: Vec::new(),
            fields: BTreeMap::new(),
            rows: Vec::new(),
            areas: Default::default(),
        }
    }

    /// A type of `kind` named `name`, anonymous when it is empty, with its `detail`.
    fn add(
        &mut self,
        kind: u32,
        name: &str,
        (size, alignment): (u32, u32),
        count: u32,
        detail: &[u8],
    ) -> u32 {
        let name = if name.is_empty() {
            0
        } else {
            self.strings.add(name)
        };
        let detail = (!detail.is_empty()).then(|| {
            self.details.extend_from_slice(detail);
            self.details.len() - detail.len()
        });
        let record = TypeRecord {
            kind,
            name,
            size,
            alignment,
            detail: 0,
            count,
        };
        self.types.push((record, detail));
        u32::try_from(self.types.len() - 1).expect("a few types")
    }

    /// The bytes and the alignment of a value of `ty`.
    fn size_of(&self, ty: u32) -> (u32, u32) {
        let (record, _) = self.types[ty as usize];
        (record.size, record.alignment)
    }

    /// An elementary type of `kind`.
    fn elementary(&mut self, kind: u32) -> u32 {
        let size = u32::try_from(width(kind).expect("an elementary kind")).expect("8 at most");
        self.add(kind, "", (size, size), 0, &[])
    }

    /// A `STRING[capacity]`, 2 + capacity bytes rounded up to even, or a `WSTRING[capacity]`,
    /// 2 + 2 × capacity bytes (specification 44 §6.2).
    fn text(&mut self, wide: bool, capacity: u32) -> u32 {
        if wide {
            self.add(kind::WSTRING, "", (2 + 2 * capacity, 2), capacity, &[])
        } else {
            let size = (2 + capacity).next_multiple_of(2);
            self.add(kind::STRING, "", (size, 2), capacity, &[])
        }
    }

    /// An enumeration of `size` bytes with its values' names and numbers.
    fn enumeration(&mut self, name: &str, size: u32, variants: &[(&str, i64)]) -> u32 {
        let mut detail = Vec::new();
        for &(variant, number) in variants {
            detail.extend_from_slice(&self.strings.add(variant).to_le_bytes());
            detail.extend_from_slice(&[0; 4]);
            detail.extend_from_slice(&number.to_le_bytes());
        }
        self.add(
            kind::ENUM,
            name,
            (size, size),
            count(variants.len()),
            &detail,
        )
    }

    /// A subrange of the elementary kind `base`.
    fn subrange(&mut self, base: u32, lower: i64, upper: i64) -> u32 {
        let size = u32::try_from(width(base).expect("an elementary kind")).expect("8 at most");
        let mut detail = Vec::new();
        detail.extend_from_slice(&base.to_le_bytes());
        detail.extend_from_slice(&[0; 4]);
        detail.extend_from_slice(&lower.to_le_bytes());
        detail.extend_from_slice(&upper.to_le_bytes());
        let name = format!("({lower}..{upper})");
        self.add(kind::SUBRANGE, &name, (size, size), 0, &detail)
    }

    /// An array of `element` with the bounds of each dimension.
    fn array(&mut self, element: u32, dims: &[(i64, i64)]) -> u32 {
        let (size, alignment) = self.size_of(element);
        let mut detail = Vec::new();
        detail.extend_from_slice(&element.to_le_bytes());
        detail.extend_from_slice(&[0; 4]);
        let mut elements = 1;
        for &(lower, upper) in dims {
            detail.extend_from_slice(&lower.to_le_bytes());
            detail.extend_from_slice(&upper.to_le_bytes());
            elements *= u32::try_from(upper - lower + 1).expect("a small array");
        }
        let layout = (size * elements, alignment);
        self.add(kind::ARRAY, "", layout, count(dims.len()), &detail)
    }

    /// A structure, function block, class or program type whose members lie in their order, each
    /// at the next offset its alignment gives, the whole padded to its largest alignment
    /// (specification 44 §7.2).
    fn structure(&mut self, kind: u32, name: &str, members: &[(&str, u32, u8)]) -> u32 {
        let (mut offset, mut alignment) = (0, 1);
        let mut fields = Vec::new();
        for &(member, ty, flags) in members {
            let (size, align) = self.size_of(ty);
            offset = u32::next_multiple_of(offset, align);
            fields.push(Field {
                name: member.to_string(),
                ty,
                offset,
                flags,
            });
            offset += size;
            alignment = alignment.max(align);
        }
        let layout = (offset.next_multiple_of(alignment), alignment);
        self.layout(kind, name, layout, fields)
    }

    /// A structure or union type whose members lie where `fields` puts them.
    fn layout(&mut self, kind: u32, name: &str, layout: (u32, u32), fields: Vec<Field>) -> u32 {
        let mut detail = Vec::new();
        for field in &fields {
            let member = MemberDetail {
                name: self.strings.add(&field.name),
                type_index: field.ty,
                offset: field.offset,
                flags: u32::from(field.flags),
            };
            detail.extend_from_slice(&member.bytes());
        }
        let index = self.add(kind, name, layout, count(fields.len()), &detail);
        self.fields.insert(index, fields);
        index
    }

    /// A `REF_TO`, or with `kind::POINTER` a `POINTER TO`, of `target`: 16 bytes (§7.3).
    fn reference(&mut self, kind: u32, target: u32) -> u32 {
        let mut detail = Vec::new();
        detail.extend_from_slice(&target.to_le_bytes());
        detail.extend_from_slice(&[0; 4]);
        self.add(kind, "", (16, 4), 0, &detail)
    }

    /// An interface type: its values are references of kind instance, 16 bytes (§6.2).
    fn interface(&mut self, name: &str) -> u32 {
        self.add(kind::INTERFACE, name, (16, 4), 0, &[])
    }

    /// The next offset of `area` at which a value of `ty` lies; the area grows to hold it.
    fn allocate(&mut self, area: u8, ty: u32) -> u32 {
        let (size, alignment) = self.size_of(ty);
        let bytes = &mut self.areas[usize::from(area)];
        let offset = bytes.len().next_multiple_of(alignment as usize);
        bytes.resize(offset + size as usize, 0);
        u32::try_from(offset).expect("a small area")
    }

    /// The global `name` of type `ty` with `flags` in `area` (globals or retain).
    fn global(&mut self, name: &str, ty: u32, area: u8, flags: u8) {
        let offset = self.allocate(area, ty);
        self.symbol(Row {
            canonical: name.to_ascii_uppercase(),
            display: name.to_string(),
            ty,
            area,
            flags,
            offset,
            size: 0,
            parent: None,
        });
    }

    /// The program instance `name` of the program type `type_name` with its variables: name,
    /// type, area and flags. The variables of the instance area lie in the instance's block in
    /// their order; each retained one at the next offset of the retain area (specification 44
    /// §7.1).
    fn program(&mut self, name: &str, type_name: &str, variables: &[(&str, u32, u8, u8)]) {
        let block: Vec<(&str, u32, u8)> = variables
            .iter()
            .filter(|&&(_, _, area, _)| area == AREA_INSTANCES)
            .map(|&(variable, ty, _, flags)| (variable, ty, flags))
            .collect();
        let program = self.structure(kind::PROGRAM, type_name, &block);
        let base = self.allocate(AREA_INSTANCES, program);
        let size = self.size_of(program).0;
        let canonical = name.to_ascii_uppercase();
        self.rows.push(Row {
            canonical: canonical.clone(),
            display: name.to_string(),
            ty: program,
            area: AREA_INSTANCES,
            flags: SYMBOL_PROGRAM,
            offset: base,
            size,
            parent: None,
        });
        for &(variable, ty, area, flags) in variables {
            let (offset, flags) = if area == AREA_INSTANCES {
                let field = self.fields[&program]
                    .iter()
                    .find(|field| field.name == variable)
                    .expect("the variable lies in the block");
                (base + field.offset, flags)
            } else {
                (self.allocate(area, ty), flags | SYMBOL_RETAINED)
            };
            self.symbol(Row {
                canonical: format!("{canonical}.{}", variable.to_ascii_uppercase()),
                display: format!("{name}.{variable}"),
                ty,
                area,
                flags,
                offset,
                size: 0,
                parent: Some(canonical.clone()),
            });
        }
    }

    /// The symbol of `row`'s variable and, as the build adds them, those of its members, but none
    /// of a library block's own state (specification 44 §5.7).
    fn symbol(&mut self, mut row: Row) {
        let (record, _) = self.types[row.ty as usize];
        row.size = record.size;
        if matches!(record.kind, kind::FUNCTION_BLOCK | kind::CLASS) {
            row.flags |= SYMBOL_INSTANCE;
        }
        if matches!(
            record.kind,
            kind::STRUCT | kind::FUNCTION_BLOCK | kind::CLASS
        ) {
            let fields = self.fields.get(&row.ty).cloned().unwrap_or_default();
            for field in fields.iter().filter(|field| !field.name.starts_with("__")) {
                self.symbol(Row {
                    canonical: format!("{}.{}", row.canonical, field.name.to_ascii_uppercase()),
                    display: format!("{}.{}", row.display, field.name),
                    ty: field.ty,
                    area: row.area,
                    flags: (row.flags & SYMBOL_RETAINED) | field.flags,
                    offset: row.offset + field.offset,
                    size: 0,
                    parent: Some(row.canonical.clone()),
                });
            }
        }
        self.rows.push(row);
    }

    /// The bytes of a process image, `%I`, `%Q` or `%M`.
    fn process_image(&mut self, area: u8, bytes: &[u8]) {
        self.areas[usize::from(area)] = bytes.to_vec();
    }

    /// The program's image: its tables and an idle module (specification 44 §5).
    fn image(&self) -> Vec<u8> {
        let mut strings = self.strings.clone();
        let base = TABLE_PREFIX_BYTES + self.types.len() * TypeRecord::BYTES;
        let types: Vec<TypeRecord> = self
            .types
            .iter()
            .map(|&(record, detail)| TypeRecord {
                detail: detail.map_or(0, |at| u32::try_from(base + at).expect("a small table")),
                ..record
            })
            .collect();
        let mut rows: Vec<&Row> = self.rows.iter().collect();
        rows.sort_by(|a, b| a.canonical.cmp(&b.canonical));
        let index = |canonical: &str| {
            rows.binary_search_by(|row| row.canonical.as_str().cmp(canonical))
                .map_or(NONE, |position| {
                    u32::try_from(position).expect("a few symbols")
                })
        };
        let symbols: Vec<SymbolRecord> = rows
            .iter()
            .map(|row| SymbolRecord {
                canonical: strings.add(&row.canonical),
                display: strings.add(&row.display),
                type_index: row.ty,
                area: row.area,
                flags: row.flags,
                offset: row.offset,
                size: row.size,
                parent: row.parent.as_deref().map_or(NONE, index),
                pou: NONE,
            })
            .collect();
        let mut writer = ImageWriter::new(
            Target::this_cpu().unwrap_or(Target::X86_64),
            u16::try_from(trust_plc_abi::INTERFACE_VERSION).expect("a small version"),
        );
        writer.module(CodeImage {
            code: alloc::vec![0; 64],
            rodata: Vec::new(),
            rodata_distance: 64,
        });
        writer.table(
            &[EntryRecord {
                kind: EntryKind::ColdInit as u32,
                index: 0,
                code_offset: 0,
            }],
            &[],
        );
        let areas: Vec<AreaRecord> = (1..=7u8)
            .map(|id| AreaRecord {
                id: u32::from(id),
                length: self.areas[usize::from(id)].len() as u64,
                alignment: 8,
                flags: 0,
                init_offset: 0,
            })
            .collect();
        writer.table(&areas, &[]);
        writer.section(SectionId::Init, Vec::new());
        writer.table::<TaskRecord>(&[], &[]);
        writer.table::<IoBindRecord>(&[], &[]);
        writer.table(&symbols, &[]);
        writer.table(&types, &self.details);
        writer.table::<StmtRecord>(&[], &[]);
        writer.table::<RetainRecord>(&[], &[]);
        writer.table::<FileRecord>(&[], &[]);
        writer.section(SectionId::MemFig, MemoryFigure::default().bytes().to_vec());
        writer.section(SectionId::Strings, strings.into_bytes());
        writer.finish().expect("the image is written")
    }
}

/// A program's memory between scans: its image and each area's bytes by its id.
struct Memory {
    image: Vec<u8>,
    areas: [Vec<u8>; 8],
}

impl Memory {
    /// The memory of `program`, whose image's every table reads as a device admits it
    /// (specification 44 §10.2).
    fn of(program: &Program) -> Self {
        let memory = Self::unchecked(program);
        Image::parse(&memory.image)
            .expect("the image parses")
            .check_tables()
            .expect("the image's tables are sound");
        memory
    }

    /// The memory of `program`, whatever its tables hold.
    fn unchecked(program: &Program) -> Self {
        Self {
            image: program.image(),
            areas: program.areas.clone(),
        }
    }

    /// The area and the offset of the variable whose canonical name is `canonical`.
    fn place(&self, canonical: &str) -> (usize, usize) {
        let image = self.image();
        let (_, symbol) = image
            .find_symbol(canonical)
            .expect("the variable is declared");
        (usize::from(symbol.area), symbol.offset as usize)
    }

    /// Writes `bytes` into the variable `canonical`, `skip` bytes into it.
    fn set_at(&mut self, canonical: &str, skip: usize, bytes: &[u8]) {
        let (area, offset) = self.place(canonical);
        let start = offset + skip;
        self.areas[area][start..start + bytes.len()].copy_from_slice(bytes);
    }

    /// Writes `bytes` into the variable `canonical`.
    fn set(&mut self, canonical: &str, bytes: &[u8]) {
        self.set_at(canonical, 0, bytes);
    }

    /// The `length` bytes of the variable `canonical` from `skip` bytes into it.
    fn get_at(&self, canonical: &str, skip: usize, length: usize) -> Vec<u8> {
        let (area, offset) = self.place(canonical);
        let start = offset + skip;
        self.areas[area][start..start + length].to_vec()
    }

    /// The first `length` bytes of the variable `canonical`.
    fn get(&self, canonical: &str, length: usize) -> Vec<u8> {
        self.get_at(canonical, 0, length)
    }
}

impl ProgramMemory for Memory {
    fn image(&self) -> Image<'_> {
        Image::parse(&self.image).expect("the image parses")
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        let source = self.areas.get(usize::from(id)).and_then(|bytes| {
            offset
                .checked_add(out.len())
                .and_then(|end| bytes.get(offset..end))
        });
        match source {
            Some(source) => {
                out.copy_from_slice(source);
                true
            }
            None => false,
        }
    }

    fn write_area(&mut self, id: u8, offset: usize, data: &[u8]) -> bool {
        let target = self.areas.get_mut(usize::from(id)).and_then(|bytes| {
            offset
                .checked_add(data.len())
                .and_then(|end| bytes.get_mut(offset..end))
        });
        match target {
            Some(target) => {
                target.copy_from_slice(data);
                true
            }
            None => false,
        }
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch)
    }
}

fn new_variable(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::New { name: name.into() }
}

fn converted(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::Converted { name: name.into() }
}

fn not_kept(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::NotKept { name: name.into() }
}

fn cleared(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::Cleared { name: name.into() }
}

/// The bytes of a reference of `kind` (specification 44 §7.3): area, kind, character index,
/// offset, extent and generation 1.
fn reference(area: u8, kind: u8, character: u16, offset: u32, extent: u32) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0] = area;
    bytes[1] = kind;
    bytes[2..4].copy_from_slice(&character.to_le_bytes());
    bytes[4..8].copy_from_slice(&offset.to_le_bytes());
    bytes[8..12].copy_from_slice(&extent.to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    bytes
}

/// The bytes of `INT` values.
fn ints(values: &[i16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// The bytes of `DINT` values.
fn dints(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// The process images are carried by address: the bytes both programs have, from the start, and a
/// longer image of the new program keeps its starting bytes beyond them (specification 44 §12.2).
/// The running program's `%I` has 4 bytes, its `%Q` 4 and its `%M` 8; the new program's 6, 2 and 8.
/// Catches an image left behind, a copy past the shorter image, the tail of a longer new image
/// overwritten, and an event recorded for an address.
#[test]
fn the_process_images_are_carried_by_address() {
    let mut old = Program::new();
    old.process_image(AREA_INPUTS, &[1, 2, 3, 4]);
    old.process_image(AREA_OUTPUTS, &[5, 6, 7, 8]);
    old.process_image(AREA_MARKERS, &[9, 10, 11, 12, 13, 14, 15, 16]);
    let mut new = Program::new();
    new.process_image(AREA_INPUTS, &[0xAA; 6]);
    new.process_image(AREA_OUTPUTS, &[0xBB; 2]);
    new.process_image(AREA_MARKERS, &[0xCC; 8]);
    let (old, mut new) = (Memory::of(&old), Memory::of(&new));

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert!(events.is_empty());
    assert_eq!(new.areas[1], [1, 2, 3, 4, 0xAA, 0xAA]);
    assert_eq!(new.areas[2], [5, 6]);
    assert_eq!(new.areas[3], [9, 10, 11, 12, 13, 14, 15, 16]);
}

/// The globals are carried by name, whatever their order and spelling, from where they lay to where
/// they lie; a new global keeps its starting value and records `New`; a constant keeps the new
/// program's value and records nothing (specification 44 §12.2; specification 11 §6.7.1).
///
/// ```text
/// running: VAR_GLOBAL count : DINT := 7; Level : INT := 3; Total : LINT := 99; END_VAR
///          VAR_GLOBAL CONSTANT Limit : INT := 5; END_VAR
///          (* G 0, 4, 8, 16 *)
/// new:     VAR_GLOBAL Level : INT; Count : DINT; Fresh : INT := 11; END_VAR    (* G 0, 4, 8 *)
///          VAR_GLOBAL CONSTANT Limit : INT := 6; END_VAR                       (* G 10 *)
///          VAR_GLOBAL RETAIN Total : LINT; END_VAR                             (* R 0 *)
/// ```
///
/// Catches a global matched by its place or by its spelling, one that moved to the retain area
/// left behind, a new global overwritten or not reported, and a constant carried.
#[test]
fn globals_are_carried_by_name_wherever_they_lie() {
    let mut old = Program::new();
    let (int, dint, lint) = (
        old.elementary(kind::INT),
        old.elementary(kind::DINT),
        old.elementary(kind::LINT),
    );
    old.global("count", dint, AREA_GLOBALS, 0);
    old.global("Level", int, AREA_GLOBALS, 0);
    old.global("Total", lint, AREA_GLOBALS, 0);
    old.global("Limit", int, AREA_GLOBALS, SYMBOL_CONSTANT);
    let mut new = Program::new();
    let (int, dint, lint) = (
        new.elementary(kind::INT),
        new.elementary(kind::DINT),
        new.elementary(kind::LINT),
    );
    new.global("Level", int, AREA_GLOBALS, 0);
    new.global("Count", dint, AREA_GLOBALS, 0);
    new.global("Fresh", int, AREA_GLOBALS, 0);
    new.global("Limit", int, AREA_GLOBALS, SYMBOL_CONSTANT);
    new.global("Total", lint, AREA_RETAIN, SYMBOL_RETAINED);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("COUNT", &7i32.to_le_bytes());
    old.set("LEVEL", &3i16.to_le_bytes());
    old.set("TOTAL", &99i64.to_le_bytes());
    old.set("LIMIT", &5i16.to_le_bytes());
    new.set("FRESH", &11i16.to_le_bytes());
    new.set("LIMIT", &6i16.to_le_bytes());

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(events, [new_variable("Fresh")]);
    assert_eq!(new.areas[4], [3, 0, 0, 0, 7, 0, 0, 0, 11, 0, 6, 0]);
    assert_eq!(new.areas[5], [99, 0, 0, 0, 0, 0, 0, 0]);
}

/// A variable of a program instance is carried from the running program's variable of the same
/// name in the instance of the same name, from where it lay to where it lies; a variable the
/// instance did not have records `New`; an instance the running program does not have records one
/// `New` for itself and none for its variables, as the interpreter records it (specification 44
/// §12.2; specification 11 §6.7.1).
///
/// ```text
/// running: P1 : Main with Speed : INT := 40 (P 0) and RETAIN Total : DINT := 1000 (R 0);
///          P2 : Aux with Count : INT := 5 (P 2)
/// new:     P1 : Main with RETAIN Speed : INT (R 0), Total : DINT (P 0) and Mode : INT := 2 (P 4);
///          P3 : Aux with Count : INT := 9 (P 8)
/// ```
///
/// Catches a variable matched across instances or by place, one that moved between the instance
/// and the retain area left behind, a renamed instance taking another instance's values, and a
/// `New` per variable of a new instance.
#[test]
fn program_variables_are_carried_by_their_instance_and_name() {
    let mut old = Program::new();
    let (int, dint) = (old.elementary(kind::INT), old.elementary(kind::DINT));
    old.program(
        "P1",
        "Main",
        &[
            ("Speed", int, AREA_INSTANCES, 0),
            ("Total", dint, AREA_RETAIN, 0),
        ],
    );
    old.program("P2", "Aux", &[("Count", int, AREA_INSTANCES, 0)]);
    let mut new = Program::new();
    let (int, dint) = (new.elementary(kind::INT), new.elementary(kind::DINT));
    new.program(
        "P1",
        "Main",
        &[
            ("Speed", int, AREA_RETAIN, 0),
            ("Total", dint, AREA_INSTANCES, 0),
            ("Mode", int, AREA_INSTANCES, 0),
        ],
    );
    new.program("P3", "Aux", &[("Count", int, AREA_INSTANCES, 0)]);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("P1.SPEED", &40i16.to_le_bytes());
    old.set("P1.TOTAL", &1000i32.to_le_bytes());
    old.set("P2.COUNT", &5i16.to_le_bytes());
    new.set("P1.MODE", &2i16.to_le_bytes());
    new.set("P3.COUNT", &9i16.to_le_bytes());

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(events, [new_variable("P1.Mode"), new_variable("P3")]);
    assert_eq!(new.areas[5], [40, 0]);
    assert_eq!(new.areas[6], [0xE8, 0x03, 0, 0, 2, 0, 0, 0, 9, 0]);
}

/// An elementary value of a changed type is kept when the value converted to the new type and
/// back comes out unchanged, and else keeps the new program's starting value (specification 44
/// §12.2; specification 11 §6.7.1, which names `INT` 5 to `DINT`, `DINT` 5 to `INT`, `DINT` 70,000
/// into an `INT` and 100 into `INT(0..10)`). Each global is declared in both programs:
///
/// ```text
/// A : INT 5 to DINT              kept: 5, Converted
/// B : DINT 70000 to INT := 77    not kept: 77, NotKept
/// C : DINT 5 to INT              kept: 5, Converted
/// D : LREAL 0.1 to REAL := 2.5   not exact as a REAL: 2.5, NotKept
/// E : REAL 1.5 to LREAL          kept: 1.5, Converted
/// F : WORD 16#00FF to INT := 1   a bit string never converts to an integer: 1, NotKept
/// G : TIME T#1.5s to LTIME       kept: 1,500,000,000 ns, Converted
/// H : INT 7 to INT(0..10)        within the bounds: 7, Converted
/// I : INT 100 to INT(0..10)      outside them: the starting 0, NotKept
/// J : INT 9 to INT               the same type: copied, nothing recorded
/// ```
///
/// Catches a value cut to fit, a bit string read as a number, a real rounded, a subrange's bounds
/// ignored, an event for an unchanged type, and a missing one for a changed type.
#[test]
fn elementary_values_are_kept_when_the_new_type_holds_them() {
    let declarations = [
        ("A", kind::INT, kind::DINT),
        ("B", kind::DINT, kind::INT),
        ("C", kind::DINT, kind::INT),
        ("D", kind::LREAL, kind::REAL),
        ("E", kind::REAL, kind::LREAL),
        ("F", kind::WORD, kind::INT),
        ("G", kind::TIME, kind::LTIME),
        ("H", kind::INT, 0),
        ("I", kind::INT, 0),
        ("J", kind::INT, kind::INT),
    ];
    let mut old = Program::new();
    let mut new = Program::new();
    let level = new.subrange(kind::INT, 0, 10);
    for (name, from, to) in declarations {
        let ty = old.elementary(from);
        old.global(name, ty, AREA_GLOBALS, 0);
        let ty = if to == 0 { level } else { new.elementary(to) };
        new.global(name, ty, AREA_GLOBALS, 0);
    }
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("A", &5i16.to_le_bytes());
    old.set("B", &70_000i32.to_le_bytes());
    old.set("C", &5i32.to_le_bytes());
    old.set("D", &0.1f64.to_le_bytes());
    old.set("E", &1.5f32.to_le_bytes());
    old.set("F", &0x00FFu16.to_le_bytes());
    old.set("G", &1_500_000_000i64.to_le_bytes());
    old.set("H", &7i16.to_le_bytes());
    old.set("I", &100i16.to_le_bytes());
    old.set("J", &9i16.to_le_bytes());
    new.set("B", &77i16.to_le_bytes());
    new.set("D", &2.5f32.to_le_bytes());
    new.set("F", &1i16.to_le_bytes());

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(
        events,
        [
            converted("A"),
            not_kept("B"),
            converted("C"),
            not_kept("D"),
            converted("E"),
            not_kept("F"),
            converted("G"),
            converted("H"),
            not_kept("I"),
        ]
    );
    assert_eq!(new.get("A", 4), 5i32.to_le_bytes());
    assert_eq!(new.get("B", 2), 77i16.to_le_bytes());
    assert_eq!(new.get("C", 2), 5i16.to_le_bytes());
    assert_eq!(new.get("D", 4), 2.5f32.to_le_bytes());
    assert_eq!(new.get("E", 8), 1.5f64.to_le_bytes());
    assert_eq!(new.get("F", 2), 1i16.to_le_bytes());
    assert_eq!(new.get("G", 8), 1_500_000_000i64.to_le_bytes());
    assert_eq!(new.get("H", 2), 7i16.to_le_bytes());
    assert_eq!(new.get("I", 2), 0i16.to_le_bytes());
    assert_eq!(new.get("J", 2), 9i16.to_le_bytes());
}

/// The rule of an elementary value on raw bytes gives the outcomes of the interpreter's coercion
/// of an initializer, converted to the new kind and back (`vm::construction::values::scalar::
/// coerce`), each worked out from it: a number fits the new kind's range, a bit string and an
/// integer never convert to each other, nor an integer and a real, since the way back refuses it;
/// a `REAL` that is not finite does not become an `LREAL`, as the coercion refuses a value that is
/// not finite; an `LREAL` becomes a `REAL` only when it is exact and finite; a `TIME` and an
/// `LTIME` convert both ways, a date only into its own kind, a `CHAR` never into a `WCHAR`, a
/// `BOOL` only into a `BOOL`. Catches each outcome that differs from the coercion's.
#[test]
fn every_coercion_outcome_holds_on_raw_bytes() {
    let minus_one = u64::from((-1i16).cast_unsigned());
    let minus_five = u64::from((-5i32).cast_unsigned());
    let cases = [
        (kind::INT, minus_one, kind::UINT, None),
        (kind::INT, 5, kind::UINT, Some(5)),
        (kind::USINT, 200, kind::SINT, None),
        (kind::BYTE, 200, kind::WORD, Some(200)),
        (kind::WORD, 300, kind::BYTE, None),
        (kind::INT, 5, kind::WORD, None),
        (kind::WORD, 5, kind::INT, None),
        (
            kind::DINT,
            minus_five,
            kind::LINT,
            Some((-5i64).cast_unsigned()),
        ),
        (kind::ULINT, u64::MAX, kind::LINT, None),
        (
            kind::LINT,
            i64::MAX.cast_unsigned(),
            kind::ULINT,
            Some(i64::MAX.cast_unsigned()),
        ),
        (kind::INT, 5, kind::REAL, None),
        (kind::REAL, u64::from(5.0f32.to_bits()), kind::INT, None),
        (kind::REAL, u64::from(f32::NAN.to_bits()), kind::LREAL, None),
        (
            kind::REAL,
            u64::from(f32::INFINITY.to_bits()),
            kind::LREAL,
            None,
        ),
        (
            kind::REAL,
            u64::from(1.5f32.to_bits()),
            kind::LREAL,
            Some(1.5f64.to_bits()),
        ),
        (
            kind::LREAL,
            2.5f64.to_bits(),
            kind::REAL,
            Some(u64::from(2.5f32.to_bits())),
        ),
        (kind::LREAL, 0.1f64.to_bits(), kind::REAL, None),
        (kind::LREAL, 1e300f64.to_bits(), kind::REAL, None),
        (kind::LTIME, 5, kind::TIME, Some(5)),
        (kind::DATE, 5, kind::LDATE, None),
        (kind::CHAR, u64::from(b'A'), kind::WCHAR, None),
        (kind::BOOL, 1, kind::BYTE, None),
        (kind::BOOL, 1, kind::BOOL, Some(1)),
    ];
    for (old, bits, new, expected) in cases {
        assert_eq!(
            convert(old, bits, new),
            expected,
            "kind {old} {bits:#x} to kind {new}"
        );
    }
}

/// An enumeration keeps its value by the name of its value, compared without regard to case, with
/// the new program's number for it, and a value whose name the new type does not declare keeps the
/// new program's starting value (specification 44 §12.2; specification 11 §6.7.1).
///
/// ```text
/// running: TYPE Mode : (Off, On, Fault) INT; State : (Idle, Busy, Done) INT;
///          TYPE Light : (Red, Green) INT;
///          M1 : Mode := On (G 0); M2 : State := Done (G 2); M3 : Light := Green (G 4)
/// new:     TYPE Mode : (Fault, Off, ON := 5, Service) DINT; State : (Idle, Busy) INT;
///          Light unchanged;
///          M1 : Mode := Fault (G 0); M2 : State := Busy (G 4); M3 : Light := Red (G 6)
/// ```
///
/// Catches a value kept by its number (1, Off in the new type), a name compared with its case, a
/// removed value kept, and an event for an unchanged type.
#[test]
fn an_enumeration_keeps_its_value_by_name() {
    let mut old = Program::new();
    let mode = old.enumeration("Mode", 2, &[("Off", 0), ("On", 1), ("Fault", 2)]);
    let state = old.enumeration("State", 2, &[("Idle", 0), ("Busy", 1), ("Done", 2)]);
    let light = old.enumeration("Light", 2, &[("Red", 0), ("Green", 1)]);
    old.global("M1", mode, AREA_GLOBALS, 0);
    old.global("M2", state, AREA_GLOBALS, 0);
    old.global("M3", light, AREA_GLOBALS, 0);
    let mut new = Program::new();
    let mode = new.enumeration(
        "Mode",
        4,
        &[("Fault", 0), ("Off", 1), ("ON", 5), ("Service", 6)],
    );
    let state = new.enumeration("State", 2, &[("Idle", 0), ("Busy", 1)]);
    let light = new.enumeration("Light", 2, &[("Red", 0), ("Green", 1)]);
    new.global("M1", mode, AREA_GLOBALS, 0);
    new.global("M2", state, AREA_GLOBALS, 0);
    new.global("M3", light, AREA_GLOBALS, 0);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("M1", &1i16.to_le_bytes());
    old.set("M2", &2i16.to_le_bytes());
    old.set("M3", &1i16.to_le_bytes());
    new.set("M2", &1i16.to_le_bytes());

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(events, [converted("M1"), not_kept("M2")]);
    assert_eq!(new.get("M1", 4), 5i32.to_le_bytes());
    assert_eq!(new.get("M2", 2), 1i16.to_le_bytes());
    assert_eq!(new.get("M3", 2), 1i16.to_le_bytes());
}

/// A text is kept when the new capacity holds it uncut, in bytes for a `STRING` and in code units
/// for a `WSTRING`, and never cut; a `CHAR` and a text of that one character convert both ways
/// (specification 44 §12.2; specification 11 §6.7.1, which names `STRING[10]` "abc" to
/// `STRING[5]`).
///
/// ```text
/// S1 : STRING[10] 'abc' to STRING[5]           kept, Converted
/// S2 : STRING[10] 'abcdefg' to STRING[5] 'xy'  never cut: 'xy', NotKept
/// S3 : STRING[8] 'abc' to WSTRING[10]          kept, three code units, Converted
/// S4 : STRING[4] 'é' (2 bytes) to WSTRING[1]   one code unit, kept, Converted
/// S5 : WSTRING[4] "é" (1 unit) to STRING[1]    2 bytes do not fit 1: '', NotKept
/// S6 : CHAR 'A' to STRING[3]                   kept, Converted
/// S7 : STRING[3] 'A' to CHAR                   kept, Converted
/// ```
///
/// Catches a text cut to fit, a capacity counted in the wrong unit, a text left in its old
/// encoding, and the rest of a capacity left with stale bytes.
#[test]
fn a_text_is_kept_only_when_the_new_capacity_holds_it() {
    let mut old = Program::new();
    let declarations = [
        ("S1", old.text(false, 10)),
        ("S2", old.text(false, 10)),
        ("S3", old.text(false, 8)),
        ("S4", old.text(false, 4)),
        ("S5", old.text(true, 4)),
        ("S6", old.elementary(kind::CHAR)),
        ("S7", old.text(false, 3)),
    ];
    for (name, ty) in declarations {
        old.global(name, ty, AREA_GLOBALS, 0);
    }
    let mut new = Program::new();
    let declarations = [
        ("S1", new.text(false, 5)),
        ("S2", new.text(false, 5)),
        ("S3", new.text(true, 10)),
        ("S4", new.text(true, 1)),
        ("S5", new.text(false, 1)),
        ("S6", new.text(false, 3)),
        ("S7", new.elementary(kind::CHAR)),
    ];
    for (name, ty) in declarations {
        new.global(name, ty, AREA_GLOBALS, 0);
    }
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("S1", &[3, 0, b'a', b'b', b'c']);
    old.set("S2", &[7, 0, b'a', b'b', b'c', b'd', b'e', b'f', b'g']);
    old.set("S3", &[3, 0, b'a', b'b', b'c']);
    old.set("S4", &[2, 0, 0xC3, 0xA9]);
    old.set("S5", &[1, 0, 0xE9, 0x00]);
    old.set("S6", b"A");
    old.set("S7", &[1, 0, b'A']);
    new.set("S1", &[0xFF; 8]);
    new.set("S2", &[2, 0, b'x', b'y']);

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(
        events,
        [
            converted("S1"),
            not_kept("S2"),
            converted("S3"),
            converted("S4"),
            not_kept("S5"),
            converted("S6"),
            converted("S7"),
        ]
    );
    // The length, the text and zeros for the rest of the capacity; the padding byte is the type's.
    assert_eq!(new.get("S1", 8), [3, 0, b'a', b'b', b'c', 0, 0, 0xFF]);
    assert_eq!(new.get("S2", 8), [2, 0, b'x', b'y', 0, 0, 0, 0]);
    let mut wide = alloc::vec![3, 0, b'a', 0, b'b', 0, b'c', 0];
    wide.resize(22, 0);
    assert_eq!(new.get("S3", 22), wide);
    assert_eq!(new.get("S4", 4), [1, 0, 0xE9, 0x00]);
    assert_eq!(new.get("S5", 4), [0, 0, 0, 0]);
    assert_eq!(new.get("S6", 6), [1, 0, b'A', 0, 0, 0]);
    assert_eq!(new.get("S7", 1), b"A");
}
