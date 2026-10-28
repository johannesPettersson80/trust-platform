//! The image's tables (specification 44 §5.4 to §5.10): areas and starting memory, tasks, I/O
//! bindings, symbols, types, statements, the retain map, files and the memory figure.

use smol_str::SmolStr;

use super::emit::{Generated, PouFrame, PouInstance};
use super::front::{first_token_start, last_token_end, Front, StatementIds};
use super::init::StartingMemory;
use super::model::Model;
use super::module::Module;
use super::stack::StackFigure;
use super::tools::Tool;
use super::types::{Kind, Member, Ty, TypeInfo};
use super::NativeError;
use trust_plc_abi as abi;
use trust_runtime_core::io_address::IoSize;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::image::{
    type_kind, AreaRecord, EntryKind, EntryRecord, FileRecord, Image, ImageWriter, IoBindRecord,
    MemberDetail, MemoryFigure, Record, RetainRecord, SectionId, StmtRecord, StringPool,
    SymbolRecord, TaskRecord, TypeRecord, AREA_HAS_INIT, AREA_RETAINED, NONE, SYMBOL_CONSTANT,
    SYMBOL_INSTANCE, SYMBOL_PROGRAM, SYMBOL_RETAINED, TABLE_PREFIX_BYTES,
};
use trust_runtime_core::native::retain::payload_bound;
use trust_runtime_core::native::tool::TOOL_SYMBOL_PREFIX;

/// A symbol before the table is sorted.
struct Pending {
    canonical: String,
    display: String,
    type_index: u32,
    area: u8,
    flags: u8,
    offset: u32,
    size: u32,
    parent: Option<String>,
    /// The program instance's number for a program root (§5.7), [`NONE`] otherwise.
    pou: u32,
}

/// The symbols of every variable and its members, and the retained variables in declaration order.
struct Symbols<'m, 'a> {
    model: &'m mut Model<'a>,
    pending: Vec<Pending>,
}

/// One variable to add with its members.
struct Variable {
    canonical: String,
    display: String,
    ty: Ty,
    area: u8,
    offset: u32,
    flags: u8,
    parent: Option<String>,
    /// The POU that declares a frame variable (§5.7), [`NONE`] for any other variable.
    pou: u32,
}

impl Symbols<'_, '_> {
    fn add(&mut self, variable: Variable) {
        let info = self.model.types.info(variable.ty).clone();
        let members: Vec<Member> = match &info.kind {
            Kind::Struct { members } | Kind::Fb { members, .. } => members.clone(),
            Kind::Overlap { members, bits } => self.model.types.overlap_members(members, bits),
            Kind::StdFb(fb) => self.model.types.std_fb_members(*fb),
            _ => Vec::new(),
        };
        let instance = if matches!(info.kind, Kind::Fb { .. } | Kind::StdFb(_)) {
            SYMBOL_INSTANCE
        } else {
            0
        };
        // A library block's own state (`__LAST` of a timer) is no variable the program declares:
        // the type table keeps it for its layout, the symbol table names none of it (§5.7).
        for member in members
            .into_iter()
            .filter(|member| !member.name.starts_with("__"))
        {
            self.add(Variable {
                canonical: format!(
                    "{}.{}",
                    variable.canonical,
                    member.name.to_ascii_uppercase()
                ),
                display: format!("{}.{}", variable.display, member.name),
                ty: member.ty,
                area: variable.area,
                offset: variable.offset + member.offset,
                flags: (variable.flags & SYMBOL_RETAINED) | member.flags,
                parent: Some(variable.canonical.clone()),
                pou: variable.pou,
            });
        }
        self.pending.push(Pending {
            canonical: variable.canonical,
            display: variable.display,
            type_index: variable.ty.0,
            area: variable.area,
            flags: variable.flags | instance,
            offset: variable.offset,
            size: info.size,
            parent: variable.parent,
            pou: variable.pou,
        });
    }

    /// The globals, then each program instance with its variables and its retained variables.
    fn collect(&mut self) {
        let globals: Vec<(SmolStr, Ty, bool, bool)> = self
            .model
            .globals
            .iter()
            .map(|global| {
                (
                    global.name.clone(),
                    global.ty,
                    global.retained,
                    global.constant,
                )
            })
            .collect();
        for (name, ty, retained, constant) in globals {
            let (area, layout) = if retained {
                (abi::AREA_RETAIN, &self.model.area_r)
            } else {
                (abi::AREA_GLOBALS, &self.model.area_g)
            };
            let Some(offset) = layout.field(&name).map(|field| field.offset) else {
                continue;
            };
            let mut flags = if constant { SYMBOL_CONSTANT } else { 0 };
            if retained {
                flags |= SYMBOL_RETAINED;
            }
            self.add(Variable {
                canonical: name.to_ascii_uppercase(),
                display: name.to_string(),
                ty,
                area,
                offset,
                flags,
                parent: None,
                pou: NONE,
            });
        }
        let type_count = u32::try_from(self.model.types.all().count()).unwrap_or(u32::MAX);
        let instances: Vec<(SmolStr, usize)> = self
            .model
            .instances
            .iter()
            .map(|instance| (instance.name.clone(), instance.program))
            .collect();
        for (number, (name, program)) in instances.into_iter().enumerate() {
            let number = u32::try_from(number).unwrap_or(NONE);
            self.instance(&name, program, number, type_count);
        }
    }

    /// A program instance: its root symbol in `P`, which carries the instance's number (§5.7), its
    /// variables in `P` and its retained variables in `R`.
    fn instance(&mut self, name: &SmolStr, program: usize, number: u32, type_count: u32) {
        let canonical = name.to_ascii_uppercase();
        let p_offset = self
            .model
            .area_p
            .field(name)
            .map_or(0, |field| field.offset);
        let r_offset = self
            .model
            .area_r
            .field(name)
            .map_or(0, |field| field.offset);
        let (members, retained, size) = {
            let program = &self.model.program_types[program];
            (
                program.members.clone(),
                program.retained.clone(),
                program.size,
            )
        };
        self.pending.push(Pending {
            canonical: canonical.clone(),
            display: name.to_string(),
            type_index: type_count + u32::try_from(program).unwrap_or(0),
            area: abi::AREA_INSTANCES,
            flags: SYMBOL_PROGRAM,
            offset: p_offset,
            size,
            parent: None,
            pou: number,
        });
        for member in members {
            self.add(Variable {
                canonical: format!("{canonical}.{}", member.name.to_ascii_uppercase()),
                display: format!("{name}.{}", member.name),
                ty: member.ty,
                area: abi::AREA_INSTANCES,
                offset: p_offset + member.offset,
                flags: member.flags,
                parent: Some(canonical.clone()),
                pou: NONE,
            });
        }
        for member in retained {
            self.add(Variable {
                canonical: format!("{canonical}.{}", member.name.to_ascii_uppercase()),
                display: format!("{name}.{}", member.name),
                ty: member.ty,
                area: abi::AREA_RETAIN,
                offset: r_offset + member.offset,
                flags: member.flags | SYMBOL_RETAINED,
                parent: Some(canonical.clone()),
                pou: NONE,
            });
        }
    }
}

impl Symbols<'_, '_> {
    /// A debug image's POU symbols, each named `@` and the POU's canonical name, and the variables
    /// of each POU's frame, at their offsets in the frame (§5.7, §13.2).
    fn pous(&mut self, pous: &[PouFrame]) {
        let type_count = u32::try_from(self.model.types.all().count()).unwrap_or(u32::MAX);
        for pou in pous {
            let canonical = format!("@{}", pou.name.to_ascii_uppercase());
            let type_index = match pou.instance {
                Some(PouInstance::Program(index)) => {
                    type_count.saturating_add(u32::try_from(index).unwrap_or(u32::MAX))
                }
                Some(PouInstance::Block(ty)) => ty.0,
                None => NONE,
            };
            let id = u32::from(pou.id);
            self.pending.push(Pending {
                canonical: canonical.clone(),
                display: pou.name.clone(),
                type_index,
                area: 0,
                flags: 0,
                offset: 0,
                size: pou.size,
                parent: None,
                pou: id,
            });
            for (var, offset) in &pou.vars {
                self.add(Variable {
                    canonical: format!("{canonical}.{}", var.name.to_ascii_uppercase()),
                    display: format!("{}.{}", pou.name, var.name),
                    ty: var.ty,
                    area: abi::AREA_STACK,
                    offset: *offset,
                    flags: 0,
                    parent: Some(canonical.clone()),
                    pou: id,
                });
            }
        }
    }

    /// The tool symbols, each named `@TOOL.` and its interface's and its method's canonical names,
    /// of the interface's type and the size of its call block, and the parts of each block at their
    /// offsets in the stack area: `THIS`, `__GIVEN`, each input, flagged as one, and the result by
    /// the method's name (§5.7, §13.3).
    fn tools(&mut self, tools: &[Tool]) {
        for tool in tools {
            self.pending.push(Pending {
                canonical: tool.canonical.clone(),
                display: tool.display.clone(),
                type_index: tool.interface.0,
                area: 0,
                flags: 0,
                offset: 0,
                size: tool.block.as_ref().map_or(0, |block| block.size),
                parent: None,
                pou: NONE,
            });
            let Some(block) = &tool.block else {
                continue;
            };
            for part in &block.parts {
                self.add(Variable {
                    canonical: format!("{}.{}", tool.canonical, part.name.to_ascii_uppercase()),
                    display: format!("{}.{}", tool.display, part.name),
                    ty: part.ty,
                    area: abi::AREA_STACK,
                    offset: part.offset,
                    flags: part.flags,
                    parent: Some(tool.canonical.clone()),
                    pou: NONE,
                });
            }
        }
    }
}

/// The sorted symbols and the lookup of a canonical name's index.
struct Sorted(Vec<Pending>);

impl Sorted {
    fn index(&self, canonical: &str) -> Option<u32> {
        self.0
            .binary_search_by(|symbol| symbol.canonical.as_str().cmp(canonical))
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }
}

/// What the tables need beyond the model.
pub(super) struct Inputs<'g> {
    pub(super) front: &'g Front,
    pub(super) ids: &'g StatementIds,
    pub(super) generated: &'g Generated,
    pub(super) module: &'g Module,
    pub(super) memory: &'g StartingMemory,
    /// The stack bound and the call level (specification 44 §8.5).
    pub(super) stack: StackFigure,
}

fn align8(value: usize) -> usize {
    value.div_ceil(8) * 8
}

/// The entry table, in the order of the generated entries: the program bodies by instance first;
/// a tool entry's index is its tool symbol's in `sorted` (§5.4).
fn entries(
    writer: &mut ImageWriter,
    inputs: &Inputs<'_>,
    sorted: &Sorted,
) -> Result<(), NativeError> {
    let mut entries = Vec::with_capacity(inputs.generated.entries.len());
    for entry in &inputs.generated.entries {
        let Some(offset) = inputs.module.entries.get(&entry.symbol) else {
            return Err(NativeError::Module(format!(
                "no entry point {}",
                entry.symbol
            )));
        };
        let index = if entry.kind == EntryKind::Method {
            inputs
                .generated
                .tools
                .iter()
                .find(|tool| {
                    tool.block
                        .as_ref()
                        .is_some_and(|block| block.entry == entry.symbol)
                })
                .and_then(|tool| sorted.index(&tool.canonical))
                .ok_or_else(|| {
                    NativeError::Image(format!(
                        "the tool entry {} has no tool symbol",
                        entry.symbol
                    ))
                })?
        } else {
            entry.index
        };
        entries.push(EntryRecord {
            kind: entry.kind as u32,
            index,
            code_offset: *offset,
        });
    }
    writer.table(&entries, &[]);
    Ok(())
}

/// The areas and their starting memory, in `INIT` in the order markers, globals, retain,
/// instances; returns the areas and the bytes of `INIT`.
fn areas(
    writer: &mut ImageWriter,
    model: &Model<'_>,
    memory: &StartingMemory,
) -> ([AreaRecord; 7], u64) {
    let mut init = Vec::new();
    let mut place = |bytes: &[u8]| -> u32 {
        let offset = align8(init.len());
        init.resize(offset, 0);
        init.extend_from_slice(bytes);
        u32::try_from(offset).unwrap_or(u32::MAX)
    };
    let markers = place(&memory.markers);
    let globals = place(&memory.globals);
    let retain = place(&memory.retain);
    let instances = place(&memory.instances);
    let area = |id: u8, length: u32, flags: u32, init_offset: u32| AreaRecord {
        id: u32::from(id),
        length: u64::from(length),
        alignment: 8,
        flags,
        init_offset,
    };
    let areas = [
        area(abi::AREA_INPUTS, model.images[0], 0, 0),
        area(abi::AREA_OUTPUTS, model.images[1], 0, 0),
        area(abi::AREA_MARKERS, model.images[2], AREA_HAS_INIT, markers),
        area(abi::AREA_GLOBALS, model.area_g.size, AREA_HAS_INIT, globals),
        area(
            abi::AREA_RETAIN,
            model.area_r.size,
            AREA_RETAINED | AREA_HAS_INIT,
            retain,
        ),
        area(
            abi::AREA_INSTANCES,
            model.area_p.size,
            AREA_HAS_INIT,
            instances,
        ),
        area(abi::AREA_STACK, model.stack, 0, 0),
    ];
    writer.table(&areas, &[]);
    let bytes = init.len() as u64;
    writer.section(SectionId::Init, init);
    (areas, bytes)
}

/// The symbol table, sorted by canonical name, and the retain map in the order of specification 11
/// §6.7 (specification 44 §5.10).
///
/// # Errors
///
/// [`NativeError::Compile`] for a tool symbol or one of its parts whose name another symbol has.
fn symbols(
    writer: &mut ImageWriter,
    pool: &mut StringPool,
    model: &mut Model<'_>,
    pous: &[PouFrame],
    tools: &[Tool],
) -> Result<Sorted, NativeError> {
    let mut symbols = Symbols {
        model,
        pending: Vec::new(),
    };
    symbols.collect();
    symbols.pous(pous);
    symbols.tools(tools);
    let Symbols { mut pending, .. } = symbols;
    pending.sort_by(|a, b| a.canonical.cmp(&b.canonical));
    // A tool's call writes the parts its tool symbol names, so no other symbol may share a name
    // with them, as a POU's symbols could under a namespace or a block named `TOOL` (§5.7).
    if let Some(pair) = pending.windows(2).find(|pair| {
        pair[0].canonical == pair[1].canonical && pair[0].canonical.starts_with(TOOL_SYMBOL_PREFIX)
    }) {
        return Err(NativeError::Compile(format!(
            "the symbol {} names a tool's call of a method and another symbol (specification 44 \
             §5.7)",
            pair[0].canonical
        )));
    }
    pending.dedup_by(|a, b| a.canonical == b.canonical);
    let sorted = Sorted(pending);
    let records: Vec<SymbolRecord> = sorted
        .0
        .iter()
        .map(|symbol| SymbolRecord {
            canonical: pool.add(&symbol.canonical),
            display: pool.add(&symbol.display),
            type_index: symbol.type_index,
            area: symbol.area,
            flags: symbol.flags,
            offset: symbol.offset,
            size: symbol.size,
            parent: symbol
                .parent
                .as_deref()
                .and_then(|parent| sorted.index(parent))
                .unwrap_or(NONE),
            pou: symbol.pou,
        })
        .collect();
    writer.table(&records, &[]);
    let retain: Vec<RetainRecord> = super::retain_map::retain_map(model)
        .into_iter()
        .map(|retained| RetainRecord {
            key: pool.add(&retained.key),
            area: u32::from(retained.area),
            offset: retained.offset,
            type_index: retained.ty.0,
        })
        .collect();
    writer.table(&retain, &[]);
    Ok(sorted)
}

/// The I/O bindings, each to its variable's symbol.
fn bindings(
    writer: &mut ImageWriter,
    pool: &mut StringPool,
    model: &Model<'_>,
    sorted: &Sorted,
) -> Result<(), NativeError> {
    let mut records = Vec::with_capacity(model.bindings.len());
    for binding in &model.bindings {
        let canonical = binding
            .path
            .iter()
            .map(|part| part.to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join(".");
        let Some(symbol) = sorted.index(&canonical) else {
            return Err(NativeError::Compile(format!(
                "the binding of {} names no variable",
                binding.path.join(".")
            )));
        };
        let address = &binding.address;
        let hierarchical = address.path.len() > 1;
        records.push(IoBindRecord {
            symbol,
            area: match address.area {
                IoArea::Input => 1,
                IoArea::Output => 2,
                IoArea::Memory => 3,
            },
            size: match address.size {
                IoSize::Bit => 0,
                IoSize::Byte => 1,
                IoSize::Word => 2,
                IoSize::DWord => 3,
                IoSize::LWord => 4,
                IoSize::Bytes(_) => {
                    return Err(NativeError::unsupported("a byte window address".into()));
                }
            },
            // A hierarchical address is its text, which the kernel parses (§5.6).
            byte: if hierarchical {
                pool.add(&address.to_string())
            } else {
                address.byte
            },
            bit: address.bit,
            kind: if hierarchical {
                trust_runtime_core::native::image::BIND_HIERARCHICAL
            } else {
                trust_runtime_core::native::image::BIND_FLAT
            },
            flags: binding.codec
                | (binding.leaf << trust_runtime_core::native::image::BIND_OFFSET_SHIFT),
        });
    }
    writer.table(&records, &[]);
    Ok(())
}

/// The tasks with their program lists of entry indexes.
fn tasks(
    writer: &mut ImageWriter,
    pool: &mut StringPool,
    model: &Model<'_>,
    inputs: &Inputs<'_>,
    sorted: &Sorted,
) {
    let mut records = Vec::with_capacity(model.tasks.len());
    let mut list = Vec::new();
    for task in &model.tasks {
        let first = u32::try_from(list.len() / 4).unwrap_or(u32::MAX);
        for program in &task.programs {
            let entry = inputs
                .generated
                .entries
                .iter()
                .position(|entry| {
                    entry.kind == EntryKind::ProgramBody && entry.index as usize == *program
                })
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(NONE);
            list.extend_from_slice(&entry.to_le_bytes());
        }
        // The function block instances it runs on their own, after its programs (IEC §6.8.2).
        for block in &task.blocks {
            let symbol = format!("trust_entry_b{block}");
            let entry = inputs
                .generated
                .entries
                .iter()
                .position(|entry| entry.symbol == symbol)
                .and_then(|index| u32::try_from(index).ok())
                .unwrap_or(NONE);
            list.extend_from_slice(&entry.to_le_bytes());
        }
        records.push(TaskRecord {
            name: pool.add(&task.name),
            interval_ns: task.interval_ns,
            priority: task.priority,
            single: task
                .single
                .as_deref()
                .and_then(|name| sorted.index(&name.to_ascii_uppercase()))
                .unwrap_or(NONE),
            program_count: u32::try_from(task.programs.len() + task.blocks.len())
                .unwrap_or(u32::MAX),
            program_first: first,
            flags: 0,
        });
    }
    writer.table(&records, &list);
}

/// The detail of a structure's, function block's or program's members, at `base` plus its offset
/// in `details`.
fn member_detail(
    pool: &mut StringPool,
    details: &mut Vec<u8>,
    base: usize,
    members: &[Member],
) -> u32 {
    let offset = u32::try_from(base + details.len()).unwrap_or(u32::MAX);
    for member in members {
        details.extend_from_slice(
            &MemberDetail {
                name: pool.add(&member.name),
                type_index: member.ty.0,
                offset: member.offset,
                flags: u32::from(member.flags),
            }
            .bytes(),
        );
    }
    offset
}

/// What follows a function block's or class's members in the detail (specification 44 §5.8): the
/// type it extends (`NONE` without one), the number of interfaces it implements and each one's name.
fn class_detail(
    pool: &mut StringPool,
    details: &mut Vec<u8>,
    extends: Option<Ty>,
    interfaces: &[SmolStr],
) {
    details.extend_from_slice(&extends.map_or(NONE, |ty| ty.0).to_le_bytes());
    details.extend_from_slice(&u32::try_from(interfaces.len()).unwrap_or(0).to_le_bytes());
    for name in interfaces {
        details.extend_from_slice(&pool.add(name).to_le_bytes());
    }
}

/// The record of one type and its detail.
fn type_record(
    pool: &mut StringPool,
    details: &mut Vec<u8>,
    base: usize,
    info: &TypeInfo,
    model: &mut Model<'_>,
) -> TypeRecord {
    let start = u32::try_from(base + details.len()).unwrap_or(u32::MAX);
    let count = |items: usize| u32::try_from(items).unwrap_or(0);
    let (kind, detail, items) = match &info.kind {
        Kind::Elem(elem) => (elem.kind(), 0, 0),
        Kind::Str(capacity) => (type_kind::STRING, 0, *capacity),
        Kind::WStr(capacity) => (type_kind::WSTRING, 0, *capacity),
        // A type with named values of an integer type is its base, by its own name (specification
        // 02 §3.2): a tool reads and writes every value of the base.
        Kind::Enum {
            base: elem,
            named: true,
            ..
        } => (elem.kind(), 0, 0),
        Kind::Enum { values, .. } => {
            for (value_name, value) in values {
                details.extend_from_slice(&pool.add(value_name).to_le_bytes());
                details.extend_from_slice(&0u32.to_le_bytes());
                details.extend_from_slice(&value.to_le_bytes());
            }
            (type_kind::ENUM, start, count(values.len()))
        }
        Kind::Subrange {
            base: elem,
            lower,
            upper,
        } => {
            details.extend_from_slice(&elem.kind().to_le_bytes());
            details.extend_from_slice(&0u32.to_le_bytes());
            details.extend_from_slice(&lower.to_le_bytes());
            details.extend_from_slice(&upper.to_le_bytes());
            (type_kind::SUBRANGE, start, 0)
        }
        Kind::Array { elem, dims } => {
            details.extend_from_slice(&elem.0.to_le_bytes());
            details.extend_from_slice(&0u32.to_le_bytes());
            for (lower, upper) in dims {
                details.extend_from_slice(&lower.to_le_bytes());
                details.extend_from_slice(&upper.to_le_bytes());
            }
            (type_kind::ARRAY, start, count(dims.len()))
        }
        // An open array has the sentinel bounds `(0, i64::MAX)` in each dimension (specification
        // 12 §4); no variable holds one, only a reference to one does.
        Kind::Open { elem, rank } => {
            details.extend_from_slice(&elem.0.to_le_bytes());
            details.extend_from_slice(&0u32.to_le_bytes());
            for _ in 0..*rank {
                details.extend_from_slice(&0i64.to_le_bytes());
                details.extend_from_slice(&i64::MAX.to_le_bytes());
            }
            (type_kind::ARRAY, start, count(*rank))
        }
        Kind::Struct { members } => (
            type_kind::STRUCT,
            member_detail(pool, details, base, members),
            count(members.len()),
        ),
        // The members of a `STRUCT OVERLAP` share bytes; a `BOOL` is the type of its bit.
        Kind::Overlap { members, bits } => {
            let members = model.types.overlap_members(members, bits);
            (
                type_kind::STRUCT,
                member_detail(pool, details, base, &members),
                count(members.len()),
            )
        }
        // A `BOOL` that takes bit n of its byte counts n + 1 (specification 44 §5.8).
        Kind::Bit(bit) => (type_kind::BOOL, 0, u32::from(*bit) + 1),
        Kind::Fb {
            def,
            members,
            base: extends,
            is_class,
            ..
        } => {
            let detail = member_detail(pool, details, base, members);
            let interfaces = model.types.implemented_interfaces(def);
            class_detail(pool, details, *extends, &interfaces);
            (
                if *is_class {
                    type_kind::CLASS
                } else {
                    type_kind::FUNCTION_BLOCK
                },
                detail,
                count(members.len()),
            )
        }
        Kind::Interface => (type_kind::INTERFACE, 0, 0),
        Kind::StdFb(fb) => {
            let members = model.types.std_fb_members(*fb);
            let detail = member_detail(pool, details, base, &members);
            class_detail(pool, details, None, &[]);
            (type_kind::FUNCTION_BLOCK, detail, count(members.len()))
        }
        // A reference's or a pointer's detail is its target type (specification 44 §5.8).
        Kind::Ref { target, pointer } => {
            details.extend_from_slice(&target.0.to_le_bytes());
            details.extend_from_slice(&0u32.to_le_bytes());
            let kind = if *pointer {
                type_kind::POINTER
            } else {
                type_kind::REFERENCE
            };
            (kind, start, 0)
        }
    };
    TypeRecord {
        kind,
        name: pool.add(&info.name),
        size: info.size,
        alignment: info.align,
        detail,
        count: items,
    }
}

/// The types: every type of the table, then the program types.
fn types(writer: &mut ImageWriter, pool: &mut StringPool, model: &mut Model<'_>) {
    let all: Vec<TypeInfo> = model.types.all().map(|(_, info)| info.clone()).collect();
    let base = TABLE_PREFIX_BYTES + (all.len() + model.program_types.len()) * TypeRecord::BYTES;
    let mut details = Vec::new();
    let mut records = Vec::with_capacity(all.len() + model.program_types.len());
    for info in &all {
        records.push(type_record(pool, &mut details, base, info, model));
    }
    for program in &model.program_types {
        let detail = member_detail(pool, &mut details, base, &program.members);
        records.push(TypeRecord {
            kind: type_kind::PROGRAM,
            name: pool.add(&program.def.name),
            size: program.size,
            alignment: program.align,
            detail,
            count: u32::try_from(program.members.len()).unwrap_or(0),
        });
    }
    writer.table(&records, &details);
}

/// The statements in id order, with their files and positions, and the files.
fn statements(writer: &mut ImageWriter, pool: &mut StringPool, inputs: &Inputs<'_>) {
    let mut records = Vec::with_capacity(inputs.ids.order.len());
    for (id, (file, node)) in inputs.ids.order.iter().enumerate() {
        let source = &inputs.front.sources[*file];
        let (start_line, start_column) = source.position(first_token_start(node));
        let (end_line, end_column) = source.position(last_token_end(node));
        records.push(StmtRecord {
            file: u16::try_from(*file).unwrap_or(u16::MAX),
            pou: inputs
                .generated
                .statement_pous
                .get(id)
                .copied()
                .unwrap_or(u16::MAX),
            start_line,
            start_column: u16::try_from(start_column).unwrap_or(u16::MAX),
            end_line,
            end_column: u16::try_from(end_column).unwrap_or(u16::MAX),
        });
    }
    writer.table(&records, &[]);
    let files: Vec<FileRecord> = inputs
        .front
        .sources
        .iter()
        .map(|source| FileRecord {
            label: pool.add(&source.label),
            source_offset: NONE,
            source_length: 0,
        })
        .collect();
    writer.table(&files, &[]);
}

/// Writes every table of the image into `writer`, and returns the memory figure.
///
/// # Errors
///
/// [`NativeError`] when a binding names no variable or an entry point is missing.
pub(super) fn write(
    writer: &mut ImageWriter,
    model: &mut Model<'_>,
    inputs: &Inputs<'_>,
) -> Result<MemoryFigure, NativeError> {
    let mut pool = StringPool::new();
    let (area_records, init_bytes) = areas(writer, model, inputs.memory);
    let sorted = symbols(
        writer,
        &mut pool,
        model,
        &inputs.generated.pous,
        &inputs.generated.tools,
    )?;
    entries(writer, inputs, &sorted)?;
    bindings(writer, &mut pool, model, &sorted)?;
    tasks(writer, &mut pool, model, inputs, &sorted);
    types(writer, &mut pool, model);
    statements(writer, &mut pool, inputs);
    // The `RESOURCE` the program's `CONFIGURATION` declares, which a computer compares with the
    // name its `runtime.toml` gives (§5.2, specification 10 §4.7).
    if let Some(resource) = inputs
        .front
        .lowered
        .configuration
        .as_ref()
        .and_then(|configuration| configuration.resource_name.as_ref())
    {
        writer.resource = pool.add(resource).saturating_add(1);
    }

    writer.section(SectionId::Strings, pool.into_bytes());
    let area_bytes: u64 = area_records.iter().map(|area| area.length).sum();
    let hierarchical = model
        .bindings
        .iter()
        .filter(|binding| binding.address.path.len() > 1)
        .count() as u64;
    let labels: Vec<&str> = inputs
        .front
        .sources
        .iter()
        .map(|source| source.label.as_str())
        .collect();
    let bookkeeping = bookkeeping(
        model.tasks.len() as u64,
        inputs.ids.order.len() as u64,
        hierarchical,
        model.images,
        (!writer.release).then_some(labels.as_slice()),
    );
    let mut figure = MemoryFigure {
        areas: area_bytes,
        stack: inputs.stack.bound,
        bookkeeping,
        retain_capture: 0,
        total: area_bytes + inputs.stack.bound + bookkeeping,
        code: inputs.module.code.code.len() as u64,
        rodata: inputs.module.code.rodata.len() as u64,
        init: init_bytes,
        call_level: inputs.stack.call_level,
    };
    writer.section(SectionId::MemFig, figure.bytes().to_vec());
    // The capture of the retained values holds the largest checkpoint payload twice, the one it
    // fills and the one a save leaves it (specification 34 MEM-09). The payload's bound is the
    // kernel's, read from the tables as a device reads them.
    let bound = retain_capture(writer)?;
    figure.retain_capture = 2 * bound;
    figure.total += figure.retain_capture;
    writer.section(SectionId::MemFig, figure.bytes().to_vec());
    Ok(figure)
}

/// The kernel's per-program memory beside the areas, the stack bound and the retain capture
/// (specification 44 §10.4), in bytes: 64 per task, a breakpoint bit per statement, an 8-byte slot of
/// the hierarchical image per binding at a hierarchical address (§5.6) and 256 for the program's
/// own record; the cycle's copies of the process images, the last scan's inputs, outputs and markers
/// (`images`, in that order) and the outputs the drivers last took; and, for a debug image, whose
/// file `labels` are given, the debugger's line table: 28 bytes per statement and, per file, its
/// label of 24 bytes and the label's text with 16 bytes beside it.
fn bookkeeping(
    tasks: u64,
    statements: u64,
    hierarchical: u64,
    images: [u32; 3],
    labels: Option<&[&str]>,
) -> u64 {
    let [inputs, outputs, markers] = images.map(u64::from);
    let line_table = labels.map_or(0, |labels| {
        28 * statements
            + labels
                .iter()
                .map(|label| 24 + align8(16 + label.len()) as u64)
                .sum::<u64>()
    });
    64 * tasks
        + statements.div_ceil(8)
        + 8 * hierarchical
        + 256
        + inputs
        + 2 * outputs
        + markers
        + line_table
}

/// The largest checkpoint payload of the image `writer` holds, by the kernel's bound
/// (specification 44 §11).
fn retain_capture(writer: &ImageWriter) -> Result<u64, NativeError> {
    let image = writer
        .clone()
        .finish()
        .map_err(|error| NativeError::Image(format!("{error:?}")))?;
    let parsed = Image::parse(&image).map_err(|error| NativeError::Image(error.to_string()))?;
    let bound = payload_bound(&parsed).map_err(|error| NativeError::Image(error.to_string()))?;
    Ok(u64::try_from(bound).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod bookkeeping_tests {
    use super::bookkeeping;

    /// The kernel's per-program memory of specification 44 §10.4, worked out by hand for 2 tasks, 168
    /// statements, 10 bindings at hierarchical addresses and process images of 16 input, 8 output and
    /// 32 marker bytes: 128 + 21 + 80 + 256 = 485 for the record, the tasks and the breakpoint bits
    /// (the reference workload's figure), 16 + 2 * 8 + 32 = 64 for the cycle's copies of the images,
    /// and for a debug image with the files `src/main.st` (11 bytes) and
    /// `src/vat/pasteurizer_control.st` (30 bytes) the line table: 28 * 168 = 4,704 and
    /// (24 + 32) + (24 + 48) = 128 for the labels. Catches a figure that leaves out the copies a
    /// cycle keeps or a debug image's line table, which a chip then lacks the memory for at the load.
    #[test]
    fn the_bookkeeping_counts_the_image_copies_and_a_debug_images_line_table() {
        let images = [16, 8, 32];
        assert_eq!(bookkeeping(2, 168, 10, images, None), 485 + 64);
        let labels = ["src/main.st", "src/vat/pasteurizer_control.st"];
        assert_eq!(
            bookkeeping(2, 168, 10, images, Some(&labels)),
            485 + 64 + 4_704 + 128
        );
    }
}
