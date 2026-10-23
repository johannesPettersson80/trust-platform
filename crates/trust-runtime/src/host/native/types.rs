//! The types of a compiled program: what each IEC type is in the generated Rust and how it lies in
//! memory (specification 44 §6.2, §7.2).
//!
//! Every variable lies at its declared size and natural alignment, a structure or a function block
//! its members in order, padded to its alignment, as Rust's `#[repr(C)]` lays out the generated
//! types; the generated crate asserts every offset this module computes (§6.3), so the two cannot
//! disagree.

use std::collections::HashMap;

use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};
use trust_runtime_core::native::image::{
    type_kind, SYMBOL_CONSTANT, SYMBOL_INPUT, SYMBOL_INSTANCE, SYMBOL_IN_OUT, SYMBOL_OUTPUT,
};

pub(super) use super::classes::{Defs, Owner};
pub(super) use super::std_blocks::StdFb;
use super::NativeError;

/// An elementary type (IEC §6.4.2, Table 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Elem {
    Bool,
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Real,
    LReal,
    Byte,
    Word,
    DWord,
    LWord,
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    Dt,
    Ldt,
    Char,
    WChar,
}

impl Elem {
    /// The elementary type of a registry type, when it is one.
    fn of(ty: &Type) -> Option<Self> {
        Some(match ty {
            Type::Bool => Self::Bool,
            Type::SInt => Self::SInt,
            Type::Int => Self::Int,
            Type::DInt => Self::DInt,
            Type::LInt => Self::LInt,
            Type::USInt => Self::USInt,
            Type::UInt => Self::UInt,
            Type::UDInt => Self::UDInt,
            Type::ULInt => Self::ULInt,
            Type::Real => Self::Real,
            Type::LReal => Self::LReal,
            Type::Byte => Self::Byte,
            Type::Word => Self::Word,
            Type::DWord => Self::DWord,
            Type::LWord => Self::LWord,
            Type::Time => Self::Time,
            Type::LTime => Self::LTime,
            Type::Date => Self::Date,
            Type::LDate => Self::LDate,
            Type::Tod => Self::Tod,
            Type::LTod => Self::LTod,
            Type::Dt => Self::Dt,
            Type::Ldt => Self::Ldt,
            Type::Char => Self::Char,
            Type::WChar => Self::WChar,
            _ => return None,
        })
    }

    /// The bytes of a value.
    pub(super) fn size(self) -> u32 {
        match self {
            Self::Bool | Self::SInt | Self::USInt | Self::Byte | Self::Char => 1,
            Self::Int | Self::UInt | Self::Word | Self::WChar => 2,
            Self::DInt | Self::UDInt | Self::DWord | Self::Real => 4,
            _ => 8,
        }
    }

    /// The Rust type of a value: `bool` for `BOOL`, the integer or float of the declared size,
    /// `i64` nanoseconds for every temporal type (specification 10's storage).
    pub(super) fn rust(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::SInt => "i8",
            Self::Int => "i16",
            Self::DInt => "i32",
            Self::LInt
            | Self::Time
            | Self::LTime
            | Self::Date
            | Self::LDate
            | Self::Tod
            | Self::LTod
            | Self::Dt
            | Self::Ldt => "i64",
            Self::USInt | Self::Byte | Self::Char => "u8",
            Self::UInt | Self::Word | Self::WChar => "u16",
            Self::UDInt | Self::DWord => "u32",
            Self::ULInt | Self::LWord => "u64",
            Self::Real => "f32",
            Self::LReal => "f64",
        }
    }

    /// The Rust type a value is stored as: a `BOOL` as a byte, 0 or 1 (specification 44 §6.2).
    pub(super) fn stored(self) -> &'static str {
        match self {
            Self::Bool => "u8",
            other => other.rust(),
        }
    }

    /// The type table's kind (specification 44 §5.8).
    pub(super) fn kind(self) -> u32 {
        match self {
            Self::Bool => type_kind::BOOL,
            Self::SInt => type_kind::SINT,
            Self::Int => type_kind::INT,
            Self::DInt => type_kind::DINT,
            Self::LInt => type_kind::LINT,
            Self::USInt => type_kind::USINT,
            Self::UInt => type_kind::UINT,
            Self::UDInt => type_kind::UDINT,
            Self::ULInt => type_kind::ULINT,
            Self::Real => type_kind::REAL,
            Self::LReal => type_kind::LREAL,
            Self::Byte => type_kind::BYTE,
            Self::Word => type_kind::WORD,
            Self::DWord => type_kind::DWORD,
            Self::LWord => type_kind::LWORD,
            Self::Time => type_kind::TIME,
            Self::LTime => type_kind::LTIME,
            Self::Date => type_kind::DATE,
            Self::LDate => type_kind::LDATE,
            Self::Tod => type_kind::TOD,
            Self::LTod => type_kind::LTOD,
            Self::Dt => type_kind::DT,
            Self::Ldt => type_kind::LDT,
            Self::Char => type_kind::CHAR,
            Self::WChar => type_kind::WCHAR,
        }
    }

    /// The IEC name.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Bool => "BOOL",
            Self::SInt => "SINT",
            Self::Int => "INT",
            Self::DInt => "DINT",
            Self::LInt => "LINT",
            Self::USInt => "USINT",
            Self::UInt => "UINT",
            Self::UDInt => "UDINT",
            Self::ULInt => "ULINT",
            Self::Real => "REAL",
            Self::LReal => "LREAL",
            Self::Byte => "BYTE",
            Self::Word => "WORD",
            Self::DWord => "DWORD",
            Self::LWord => "LWORD",
            Self::Time => "TIME",
            Self::LTime => "LTIME",
            Self::Date => "DATE",
            Self::LDate => "LDATE",
            Self::Tod => "TIME_OF_DAY",
            Self::LTod => "LTIME_OF_DAY",
            Self::Dt => "DATE_AND_TIME",
            Self::Ldt => "LDATE_AND_TIME",
            Self::Char => "CHAR",
            Self::WChar => "WCHAR",
        }
    }

    pub(super) fn is_signed(self) -> bool {
        matches!(self, Self::SInt | Self::Int | Self::DInt | Self::LInt)
    }

    pub(super) fn is_unsigned(self) -> bool {
        matches!(self, Self::USInt | Self::UInt | Self::UDInt | Self::ULInt)
    }

    pub(super) fn is_bits(self) -> bool {
        matches!(self, Self::Byte | Self::Word | Self::DWord | Self::LWord)
    }

    pub(super) fn is_integer(self) -> bool {
        self.is_signed() || self.is_unsigned()
    }

    pub(super) fn is_real(self) -> bool {
        matches!(self, Self::Real | Self::LReal)
    }

    pub(super) fn is_duration(self) -> bool {
        matches!(self, Self::Time | Self::LTime)
    }
}

/// A type of the program's type table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct Ty(pub(super) u32);

/// One member of a structure or function block.
#[derive(Clone, Debug)]
pub(super) struct Member {
    /// The name as declared.
    pub(super) name: SmolStr,
    pub(super) ty: Ty,
    /// The offset in the owner.
    pub(super) offset: u32,
    /// The image's `SYMBOL_` flags.
    pub(super) flags: u8,
    /// The path to the base that declares it, `__BASE.` once per level, for a variable a function
    /// block or a class inherits (specification 44 §6.4); empty for its own.
    pub(super) via: SmolStr,
}

impl Member {
    /// The member's name in the generated Rust: the path to its base and the name in capitals.
    pub(super) fn field(&self) -> String {
        format!("{}{}", self.via, rust_field(&self.name))
    }
}

/// What a type is.
#[derive(Clone, Debug)]
pub(super) enum Kind {
    Elem(Elem),
    /// `STRING[n]`.
    Str(u32),
    /// `WSTRING[n]`.
    WStr(u32),
    /// An enumeration of the base type, with its values.
    Enum {
        base: Elem,
        values: Vec<(SmolStr, i64)>,
    },
    /// A subrange of the base type.
    Subrange {
        base: Elem,
        lower: i64,
        upper: i64,
    },
    /// An array: its element type and each dimension's bounds.
    Array {
        elem: Ty,
        dims: Vec<(i64, i64)>,
    },
    Struct {
        members: Vec<Member>,
    },
    /// A user function block or class: its definition's name, its variables (its base's first, by
    /// the path to them), its base's type, its class id (0 for a type that needs none) and whether it
    /// is a class, which has no body (specification 44 §6.4).
    Fb {
        def: SmolStr,
        members: Vec<Member>,
        base: Option<Ty>,
        class: u32,
        is_class: bool,
    },
    /// An interface (IEC §6.6.6): a reference to an instance of a class that implements it.
    Interface,
    StdFb(StdFb),
    /// A reference to a variable of the target type: `REF_TO`, and the `VAR_IN_OUT` of a function
    /// block, which holds a reference to its argument (IEC §6.4.4.10; specification 44 §6.4, §7.3).
    Ref {
        target: Ty,
    },
}

/// A type with its layout and its name in the generated Rust.
#[derive(Clone, Debug)]
pub(super) struct TypeInfo {
    pub(super) kind: Kind,
    pub(super) size: u32,
    pub(super) align: u32,
    /// The IEC name, empty for an anonymous type.
    pub(super) name: SmolStr,
    /// The Rust type of the storage.
    pub(super) rust: String,
}

impl TypeInfo {
    /// The elementary type of a scalar's values: its own, an enumeration's or a subrange's base.
    pub(super) fn scalar(&self) -> Option<Elem> {
        match &self.kind {
            Kind::Elem(elem) => Some(*elem),
            Kind::Enum { base, .. } | Kind::Subrange { base, .. } => Some(*base),
            _ => None,
        }
    }

    /// The members of a structure or function block.
    pub(super) fn members(&self) -> &[Member] {
        match &self.kind {
            Kind::Struct { members } | Kind::Fb { members, .. } => members,
            _ => &[],
        }
    }
}

/// A name in capitals, which is how generated Rust names a member or a variable.
pub(super) fn rust_field(name: &str) -> String {
    name.to_ascii_uppercase()
}

/// A qualified IEC name as a Rust identifier: capitals, `.` as `__`.
pub(super) fn mangle(name: &str) -> String {
    name.to_ascii_uppercase().replace('.', "__")
}

fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

/// The members `fields` laid out in order at their alignments, and the size and alignment of the
/// whole, padded to its alignment (specification 44 §7.2).
pub(super) fn lay_out(
    fields: Vec<(SmolStr, Ty, u8)>,
    types: &Types<'_>,
) -> (Vec<Member>, u32, u32) {
    let mut offset = 0;
    let mut align = 1;
    let mut members = Vec::with_capacity(fields.len());
    for (name, ty, flags) in fields {
        let info = types.info(ty);
        offset = align_up(offset, info.align);
        members.push(Member {
            name,
            ty,
            offset,
            flags,
            via: SmolStr::default(),
        });
        offset += info.size;
        align = align.max(info.align);
    }
    (members, align_up(offset, align), align)
}

/// The program's types, each once, by the registry's types and by structure.
pub(super) struct Types<'a> {
    registry: TypeRegistry,
    pub(super) defs: Defs<'a>,
    entries: Vec<TypeInfo>,
    by_registry: HashMap<TypeId, Ty>,
    by_rust: HashMap<String, Ty>,
    /// The types with a class id, by id less one.
    pub(super) class_ids: Vec<Ty>,
}

impl<'a> Types<'a> {
    pub(super) fn new(registry: TypeRegistry, defs: Defs<'a>) -> Self {
        Self {
            registry,
            defs,
            entries: Vec::new(),
            by_registry: HashMap::new(),
            by_rust: HashMap::new(),
            class_ids: Vec::new(),
        }
    }

    pub(super) fn info(&self, ty: Ty) -> &TypeInfo {
        &self.entries[ty.0 as usize]
    }

    pub(super) fn all(&self) -> impl Iterator<Item = (Ty, &TypeInfo)> {
        (0u32..)
            .zip(self.entries.iter())
            .map(|(index, info)| (Ty(index), info))
    }

    pub(super) fn registry(&self) -> &TypeRegistry {
        &self.registry
    }

    /// The type of `hir`, a type of the file whose symbols are `symbols`: trust-hir numbers the
    /// types of each file on its own, so the type is first found in the project's registry by name.
    pub(super) fn of_hir(
        &mut self,
        symbols: &trust_hir::symbols::SymbolTable,
        hir: TypeId,
    ) -> Result<Ty, NativeError> {
        let id = crate::harness::import_hir_type_to_runtime(&mut self.registry, symbols, hir)
            .map_err(|error| NativeError::Compile(error.to_string()))?;
        self.of(id)
    }

    /// The type whose Rust type is `rust`, when it is laid out.
    pub(super) fn by_rust(&self, rust: &str) -> Option<Ty> {
        self.by_rust.get(rust).copied()
    }

    /// The type that holds `info`, added once per Rust type; an enumeration or a subrange once
    /// per its own name or bounds, since its storage is its base's.
    pub(super) fn intern(&mut self, info: TypeInfo) -> Ty {
        let key = match &info.kind {
            // Elementary types of one Rust type are different types: LINT, TIME and DATE are all
            // i64.
            Kind::Elem(elem) => format!("elem {}", elem.name()),
            Kind::Enum { .. } => format!("enum {}", info.name),
            Kind::Subrange { .. } => format!("subrange {}", info.name),
            // Every reference is a `RefValue`; its target makes it a type of its own, as an
            // interface's name does.
            Kind::Ref { target } => format!("ref {}", target.0),
            Kind::Interface => format!("interface {}", info.name),
            // Arrays of one storage are different types when their elements or their bounds differ:
            // `ARRAY[0..31]` and `ARRAY[1..32] OF INT` index differently, and arrays of two
            // enumerations of one base hold different values.
            Kind::Array { elem, dims } => format!("array {} {dims:?}", elem.0),
            _ => info.rust.clone(),
        };
        if let Some(ty) = self.by_rust.get(&key) {
            return *ty;
        }
        let ty = Ty(u32::try_from(self.entries.len()).unwrap_or(u32::MAX));
        self.by_rust.insert(key, ty);
        self.entries.push(info);
        ty
    }

    pub(super) fn elem(&mut self, elem: Elem) -> Ty {
        self.intern(TypeInfo {
            kind: Kind::Elem(elem),
            size: elem.size(),
            align: elem.size(),
            name: SmolStr::new_static(elem.name()),
            rust: format!("Cell<{}>", elem.stored()),
        })
    }

    pub(super) fn string(&mut self, capacity: u32) -> Ty {
        self.intern(TypeInfo {
            kind: Kind::Str(capacity),
            size: align_up(2 + capacity, 2),
            align: 2,
            name: SmolStr::new(format!("STRING[{capacity}]")),
            rust: format!("Str<{capacity}>"),
        })
    }

    fn wstring(&mut self, capacity: u32) -> Ty {
        self.intern(TypeInfo {
            kind: Kind::WStr(capacity),
            size: 2 + 2 * capacity,
            align: 2,
            name: SmolStr::new(format!("WSTRING[{capacity}]")),
            rust: format!("WStr<{capacity}>"),
        })
    }

    /// The reference to a variable of type `target` (specification 44 §7.3): 16 bytes.
    pub(super) fn reference(&mut self, target: Ty) -> Ty {
        let name = SmolStr::new(format!("REF_TO {}", self.info(target).name));
        self.intern(TypeInfo {
            kind: Kind::Ref { target },
            size: 16,
            align: 4,
            name,
            rust: "Cell<abi::RefValue>".into(),
        })
    }

    pub(super) fn std_fb(&mut self, fb: StdFb) -> Ty {
        let (size, align) = fb.layout();
        let info = TypeInfo {
            kind: Kind::StdFb(fb),
            size: u32::try_from(size).unwrap_or(u32::MAX),
            align: u32::try_from(align).unwrap_or(8),
            name: SmolStr::new(fb.name()),
            rust: fb.rust(),
        };
        self.intern(info)
    }

    /// The members of a standard function block, with their types.
    pub(super) fn std_fb_members(&mut self, fb: StdFb) -> Vec<Member> {
        fb.members()
            .into_iter()
            .map(|(name, elem, offset, flags)| Member {
                name: SmolStr::new_static(name),
                ty: self.elem(elem),
                offset: u32::try_from(offset).unwrap_or(u32::MAX),
                flags,
                via: SmolStr::default(),
            })
            .collect()
    }

    /// The type of registry type `id`.
    pub(super) fn of(&mut self, id: TypeId) -> Result<Ty, NativeError> {
        if let Some(ty) = self.by_registry.get(&id) {
            return Ok(*ty);
        }
        let ty = self.make(id)?;
        self.by_registry.insert(id, ty);
        Ok(ty)
    }

    fn make(&mut self, id: TypeId) -> Result<Ty, NativeError> {
        let Some(ty) = self.registry.get(id).cloned() else {
            return Err(NativeError::unsupported(format!("type #{}", id.0)));
        };
        let ty = &ty;
        if let Some(elem) = Elem::of(ty) {
            return Ok(self.elem(elem));
        }
        match ty {
            Type::String { max_len } => Ok(self.string(max_len.unwrap_or(80))),
            Type::WString { max_len } => Ok(self.wstring(max_len.unwrap_or(80))),
            Type::Alias { target, .. } => self.of(*target),
            Type::Enum { name, base, values } => {
                let base_ty = self.of(*base)?;
                let Some(base) = self.info(base_ty).scalar().filter(|elem| elem.is_integer())
                else {
                    return Err(NativeError::unsupported(format!(
                        "enumeration {name} without an integer base"
                    )));
                };
                Ok(self.intern(TypeInfo {
                    kind: Kind::Enum {
                        base,
                        values: values.clone(),
                    },
                    size: base.size(),
                    align: base.size(),
                    name: name.clone(),
                    rust: format!("Cell<{}>", base.stored()),
                }))
            }
            Type::Subrange { base, lower, upper } => {
                let base_ty = self.of(*base)?;
                let Some(base) = self.info(base_ty).scalar() else {
                    return Err(NativeError::unsupported("subrange of a non-integer".into()));
                };
                Ok(self.intern(TypeInfo {
                    kind: Kind::Subrange {
                        base,
                        lower: *lower,
                        upper: *upper,
                    },
                    size: base.size(),
                    align: base.size(),
                    name: SmolStr::new(format!("{}({lower}..{upper})", base.name())),
                    rust: format!("Cell<{}>", base.stored()),
                }))
            }
            Type::Array {
                element,
                dimensions,
            } => {
                let elem = self.of(*element)?;
                let info = self.info(elem).clone();
                let mut count: u64 = 1;
                let mut rust = info.rust.clone();
                for (lower, upper) in dimensions.iter().rev() {
                    let length = upper
                        .checked_sub(*lower)
                        .and_then(|span| span.checked_add(1))
                        .filter(|length| *length > 0)
                        .ok_or_else(|| NativeError::unsupported("array of no elements".into()))?;
                    count = count.saturating_mul(u64::try_from(length).unwrap_or(u64::MAX));
                    rust = format!("[{rust}; {length}]");
                }
                let size = u64::from(info.size).saturating_mul(count);
                let size = u32::try_from(size)
                    .map_err(|_| NativeError::unsupported(format!("an array of {size} bytes")))?;
                Ok(self.intern(TypeInfo {
                    kind: Kind::Array {
                        elem,
                        dims: dimensions.clone(),
                    },
                    size,
                    align: info.align,
                    name: SmolStr::default(),
                    rust,
                }))
            }
            Type::Struct { name, fields } => {
                if self.registry.is_overlapping_struct_type(id) {
                    return Err(NativeError::unsupported(format!("STRUCT OVERLAP {name}")));
                }
                let mut laid = Vec::with_capacity(fields.len());
                for field in fields {
                    laid.push((field.name.clone(), self.of(field.type_id)?, 0));
                }
                let (members, size, align) = lay_out(laid, self);
                Ok(self.intern(TypeInfo {
                    kind: Kind::Struct { members },
                    size,
                    align,
                    name: name.clone(),
                    rust: format!("S_{}", mangle(name)),
                }))
            }
            Type::FunctionBlock { name } | Type::Class { name } => self.function_block(name),
            Type::Reference { target } => {
                let target = self.of(*target)?;
                Ok(self.reference(target))
            }
            Type::Pointer { .. } => Err(NativeError::unsupported("POINTER TO".into())),
            Type::Union { name, .. } => Err(NativeError::unsupported(format!("UNION {name}"))),
            Type::Interface { name } => self.interface(name),
            other => Err(NativeError::unsupported(format!("the type {other:?}"))),
        }
    }

    fn function_block(&mut self, name: &SmolStr) -> Result<Ty, NativeError> {
        // The standard function blocks are the library's, whatever the runtime registers for them.
        if let Some(fb) = StdFb::named(name) {
            return Ok(self.std_fb(fb));
        }
        let Some(owner) = self.owner(name) else {
            return Err(NativeError::unsupported(format!("function block {name}")));
        };
        self.class_like(owner)
    }

    /// The variables of `owner` as the members of its instances, after its base (or its class id)
    /// as `first`: the inputs, outputs and in-outs, then the variables.
    pub(super) fn own_members(
        &mut self,
        owner: Owner<'a>,
        first: Option<(SmolStr, Ty, u8)>,
    ) -> Result<Vec<(SmolStr, Ty, u8)>, NativeError> {
        let name = owner.name();
        let mut fields: Vec<(SmolStr, Ty, u8)> = first.into_iter().collect();
        for param in owner.params() {
            let ty = self.of(param.type_id)?;
            let (ty, flags) = match param.direction {
                ParamDirection::In => (ty, SYMBOL_INPUT),
                ParamDirection::Out => (ty, SYMBOL_OUTPUT),
                // The instance holds a reference to the argument of its last call (§6.4).
                ParamDirection::InOut => (self.reference(ty), SYMBOL_IN_OUT),
            };
            fields.push((param.name.clone(), ty, flags));
        }
        for var in owner.vars() {
            if var.external {
                continue;
            }
            if matches!(
                var.retain,
                crate::RetainPolicy::Retain | crate::RetainPolicy::Persistent
            ) {
                return Err(NativeError::unsupported(format!(
                    "VAR RETAIN {} inside function block {name}",
                    var.name
                )));
            }
            let ty = self.of(var.type_id)?;
            let mut flags = if var.constant { SYMBOL_CONSTANT } else { 0 };
            if matches!(self.info(ty).kind, Kind::Fb { .. } | Kind::StdFb(_)) {
                flags |= SYMBOL_INSTANCE;
            }
            fields.push((var.name.clone(), ty, flags));
        }
        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;

    /// The layout rules of specification 44 §7.2: each member at its natural alignment, the whole
    /// padded to its largest alignment. A `STRING[16]` (18 bytes, aligned 2) then a `REAL` (4) then
    /// a `TIME` (8): 0, 20, 24, size 32, alignment 8.
    #[test]
    fn members_lie_at_their_natural_alignment() {
        let registry = TypeRegistry::new();
        let (blocks, classes, interfaces) = (IndexMap::new(), IndexMap::new(), IndexMap::new());
        let mut types = Types::new(registry, Defs::new(&blocks, &classes, &interfaces));
        let name = types.string(16);
        let level = types.elem(Elem::Real);
        let hold = types.elem(Elem::Time);
        let (members, size, align) = lay_out(
            vec![
                ("Name".into(), name, 0),
                ("FillLevel".into(), level, 0),
                ("HoldTime".into(), hold, 0),
            ],
            &types,
        );
        let offsets: Vec<u32> = members.iter().map(|member| member.offset).collect();
        assert_eq!(offsets, [0, 20, 24]);
        assert_eq!((size, align), (32, 8));
    }

    /// A `STRING[n]` is 2 + n bytes rounded up to an even number, as `Str<n>` is in the interface.
    #[test]
    fn strings_take_their_length_and_capacity() {
        let registry = TypeRegistry::new();
        let (blocks, classes, interfaces) = (IndexMap::new(), IndexMap::new(), IndexMap::new());
        let mut types = Types::new(registry, Defs::new(&blocks, &classes, &interfaces));
        let five = types.string(5);
        assert_eq!(types.info(five).size, 8);
        assert_eq!(
            types.info(five).size as usize,
            size_of::<trust_plc_abi::Str<5>>()
        );
        let eighty = types.string(80);
        assert_eq!(types.info(eighty).size, 82);
    }

    /// The timers' members are the library's own: `TON`'s `ET` at 24, its size 48.
    #[test]
    fn standard_function_blocks_take_the_library_layout() {
        let registry = TypeRegistry::new();
        let (blocks, classes, interfaces) = (IndexMap::new(), IndexMap::new(), IndexMap::new());
        let mut types = Types::new(registry, Defs::new(&blocks, &classes, &interfaces));
        let ton = types.std_fb(StdFb::Ton);
        assert_eq!(types.info(ton).size, 48);
        let members = types.std_fb_members(StdFb::Ton);
        let et = members.iter().find(|member| member.name == "ET").unwrap();
        assert_eq!(et.offset, 24);
    }
}
