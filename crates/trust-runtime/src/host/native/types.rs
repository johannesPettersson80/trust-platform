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
    SYMBOL_CONSTANT, SYMBOL_INPUT, SYMBOL_INSTANCE, SYMBOL_IN_OUT, SYMBOL_OUTPUT,
};

pub(super) use super::classes::{Defs, Owner};
pub(super) use super::std_blocks::StdFb;
use super::NativeError;

mod elem;
mod region;

pub(super) use elem::Elem;

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
    /// An enumeration of the base type, with its values; with `named`, a type with named values
    /// of an integer type, which holds every value of its base (specification 02 §3.2).
    Enum {
        base: Elem,
        values: Vec<(SmolStr, i64)>,
        named: bool,
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
    /// An array whose bounds a caller supplies, `ARRAY[*]` of `rank` dimensions (IEC §6.5.3;
    /// specification 03 §6): a parameter is a view of the caller's array (`abi::Open`), a
    /// reference to one holds the bounds beside the reference (`abi::OpenRef`); no variable holds
    /// one itself.
    Open {
        elem: Ty,
        rank: usize,
    },
    Struct {
        members: Vec<Member>,
    },
    /// A `STRUCT OVERLAP` (IEC §6.4.4.7.2; specification 02 §3.6; specification 44 §6.2): its
    /// members over one byte backing, each at the byte its relative address names, a `BOOL` at
    /// its bit of that byte (`bits`, in member order), so that a write through one member is read
    /// through every member that shares its bits.
    Overlap {
        members: Vec<Member>,
        bits: Vec<Option<u8>>,
    },
    /// A `BOOL` member of a `STRUCT OVERLAP` that takes bit n of its byte, as the type table lists
    /// it (specification 44 §5.8); the generated code reaches it as a `BOOL`.
    Bit(u8),
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
    /// block, which holds a reference to its argument (IEC §6.4.4.10; specification 44 §6.4, §7.3);
    /// with `pointer`, a `POINTER TO`, which `ADR` gives (specification 02 §4.2).
    Ref {
        target: Ty,
        pointer: bool,
    },
}

impl Kind {
    /// Whether a value of the kind is an instance of a function block or a class.
    pub(super) fn is_instance(&self) -> bool {
        matches!(self, Kind::Fb { .. } | Kind::StdFb(_))
    }
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

/// A name in capitals, which is how generated Rust names a member or a variable; a namespace's
/// `.` as `__`, which no IEC identifier holds.
pub(super) fn rust_field(name: &str) -> String {
    mangle(name)
}

/// A qualified IEC name as a Rust identifier: capitals, `.` as `__`, the `::` of a static's hidden
/// name (`__STAT::Owner::name`) as `__`, and the `%` of a directly represented variable's name as
/// `__` (`%IX0.1` as `__IX0__1`), which no IEC identifier can start with (IEC §6.1.2).
pub(super) fn mangle(name: &str) -> String {
    name.to_ascii_uppercase()
        .replace("::", "__")
        .replace(['.', '%'], "__")
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
    /// The bit offset of each member of a structure with relatively located members, `AT %B2`
    /// or `AT %X3.1`, in member order; `None` for a member without one (IEC §6.4.4.7).
    pub(super) relative: HashMap<Ty, Vec<Option<u64>>>,
    /// The type a structure, or an array of structures, is in a `STRUCT OVERLAP`'s backing, by the
    /// type ([`Types::region`]).
    regions: HashMap<Ty, Ty>,
    /// The type a region type is outside a backing: the inverse of `regions`.
    naturals: HashMap<Ty, Ty>,
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
            relative: HashMap::new(),
            regions: HashMap::new(),
            naturals: HashMap::new(),
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

    /// The bytes `SIZEOF` gives for the type `hir` of a file whose symbols are `symbols`
    /// (specification 29 §1), in a program for a target whose addresses take `pointer_bytes`.
    pub(super) fn size_of_hir(
        &mut self,
        symbols: &trust_hir::symbols::SymbolTable,
        hir: TypeId,
        pointer_bytes: u64,
    ) -> Result<u64, NativeError> {
        let id = crate::harness::import_hir_type_to_runtime(&mut self.registry, symbols, hir)
            .map_err(|error| NativeError::Compile(error.to_string()))?;
        trust_runtime_core::value::size_of_type_for_target(id, &self.registry, pointer_bytes)
            .map_err(|error| NativeError::Compile(format!("SIZEOF: {error:?}")))
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
            Kind::Ref { target, pointer } => format!("ref {pointer} {}", target.0),
            Kind::Interface => format!("interface {}", info.name),
            // Arrays of one storage are different types when their elements or their bounds differ:
            // `ARRAY[0..31]` and `ARRAY[1..32] OF INT` index differently, and arrays of two
            // enumerations of one base hold different values.
            Kind::Array { elem, dims } => format!("array {} {dims:?}", elem.0),
            Kind::Open { elem, rank } => format!("open {} {rank}", elem.0),
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

    /// The reference to a variable of type `target` (specification 44 §7.3): 16 bytes, and the
    /// bounds of an open array's dimensions after them.
    pub(super) fn reference(&mut self, target: Ty) -> Ty {
        let name = SmolStr::new(format!("REF_TO {}", self.info(target).name));
        let (size, align, rust) = self.reference_storage(target);
        self.intern(TypeInfo {
            kind: Kind::Ref {
                target,
                pointer: false,
            },
            size,
            align,
            name,
            rust,
        })
    }

    /// The bytes, the alignment and the Rust type of a reference to `target`: a `RefValue`, or for
    /// an open array an `abi::OpenRef`, which holds its bounds too (specification 44 §7.3).
    fn reference_storage(&self, target: Ty) -> (u32, u32, String) {
        match self.info(target).kind {
            Kind::Open { rank, .. } => (
                16 + 16 * u32::try_from(rank).unwrap_or(u32::MAX / 32),
                8,
                format!("abi::OpenRef<{rank}>"),
            ),
            _ => (16, 4, "Cell<abi::RefValue>".into()),
        }
    }

    /// The array of `rank` dimensions of `elem` whose bounds a caller supplies (`ARRAY[*]`).
    fn open(&mut self, elem: Ty, rank: usize) -> Ty {
        let info = self.info(elem);
        let stars = vec!["*"; rank].join(", ");
        let name = SmolStr::new(format!("ARRAY[{stars}] OF {}", info.name));
        let rust = format!("abi::Open<'_, {}, {rank}>", info.rust);
        self.intern(TypeInfo {
            kind: Kind::Open { elem, rank },
            size: 0,
            align: 1,
            name,
            rust,
        })
    }

    /// The type of a `POINTER TO` the target type: a reference of kind pointer (specification 02
    /// §4.2; specification 44 §6.2), 16 bytes.
    pub(super) fn pointer(&mut self, target: Ty) -> Ty {
        let name = SmolStr::new(format!("POINTER TO {}", self.info(target).name));
        let (size, align, rust) = self.reference_storage(target);
        self.intern(TypeInfo {
            kind: Kind::Ref {
                target,
                pointer: true,
            },
            size,
            align,
            name,
            rust,
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
                        named: self.registry.is_named_value_type(id),
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
            } if dimensions
                .iter()
                .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX) =>
            {
                let elem = self.of(*element)?;
                Ok(self.open(elem, dimensions.len()))
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
                let reserved = self.reserve(id, name);
                if self.registry.is_overlapping_struct_type(id) {
                    return self.overlap(reserved, name, fields);
                }
                let mut laid = Vec::with_capacity(fields.len());
                for field in fields {
                    laid.push((field.name.clone(), self.of(field.type_id)?, 0));
                }
                let (members, size, align) = lay_out(laid, self);
                let relative: Vec<Option<u64>> = fields
                    .iter()
                    .map(|field| field.address.as_deref().and_then(relative_bit))
                    .collect();
                let ty = self.define(
                    reserved,
                    TypeInfo {
                        kind: Kind::Struct { members },
                        size,
                        align,
                        name: name.clone(),
                        rust: format!("S_{}", mangle(name)),
                    },
                );
                if relative.iter().any(Option::is_some) {
                    self.relative.insert(ty, relative);
                }
                Ok(ty)
            }
            Type::FunctionBlock { name } | Type::Class { name } => self.function_block(name),
            Type::Reference { target } => {
                let target = self.of(*target)?;
                Ok(self.reference(target))
            }
            Type::Pointer { target } => {
                let target = self.of(*target)?;
                Ok(self.pointer(target))
            }
            // A union holds each variant as a member of its own, which a write of another variant
            // leaves as it is (specification 02 §3.7; specification 44 §6.2).
            Type::Union { name, variants } => {
                let reserved = self.reserve(id, name);
                let mut laid = Vec::with_capacity(variants.len());
                for variant in variants {
                    laid.push((variant.name.clone(), self.of(variant.type_id)?, 0));
                }
                let (members, size, align) = lay_out(laid, self);
                Ok(self.define(
                    reserved,
                    TypeInfo {
                        kind: Kind::Struct { members },
                        size,
                        align,
                        name: name.clone(),
                        rust: format!("S_{}", mangle(name)),
                    },
                ))
            }
            Type::Interface { name } => self.interface(name),
            other => Err(NativeError::unsupported(format!("the type {other:?}"))),
        }
    }

    /// A `STRUCT OVERLAP`: each member at the bits its relative address names, a `BOOL` one bit
    /// and every other value whole bytes from a byte on, over a backing of the bytes up to the end
    /// of the last member, aligned to 1 (specification 02 §3.6; specification 34 MEM-12). A
    /// structure, an overlap or an array is the bytes of its own layout ([`Self::region`]); a
    /// string is refused (IEC §6.4.4.7.1).
    fn overlap(
        &mut self,
        reserved: Ty,
        name: &SmolStr,
        fields: &[trust_hir::types::StructField],
    ) -> Result<Ty, NativeError> {
        let mut members = Vec::with_capacity(fields.len());
        let mut bits = Vec::with_capacity(fields.len());
        let mut size = 0u32;
        for field in fields {
            let refused = |what: &str| {
                NativeError::unsupported(format!(
                    "the member {} of STRUCT OVERLAP {name}, {what}",
                    field.name
                ))
            };
            let ty = self.of(field.type_id)?;
            let at = field
                .address
                .as_deref()
                .and_then(relative_bit)
                .ok_or_else(|| refused("which has no relative address"))?;
            let byte = u32::try_from(at / 8).map_err(|_| refused("which lies past 4 GiB"))?;
            #[allow(clippy::cast_possible_truncation)]
            let bit = (at % 8) as u8;
            // A structure, an overlap or an array lies in the backing as the bytes of its own
            // layout (IEC §6.4.4.7.1, its example `Com_data`), a string not: "Overlapping of
            // strings is beyond the scope of this standard".
            let ty = match self.info(ty).kind {
                Kind::Str(_) | Kind::WStr(_) => {
                    return Err(refused(
                        "a string, whose overlapping IEC 61131-3 §6.4.4.7.1 leaves outside the \
                         standard",
                    ));
                }
                _ => self
                    .region(ty)
                    .map_err(|_| refused("which is no data value"))?,
            };
            let (bit, bytes) = if self.info(ty).scalar() == Some(Elem::Bool) {
                self.bit(bit);
                (Some(bit), 1)
            } else if bit == 0 {
                (None, self.info(ty).size)
            } else {
                return Err(refused("which does not start at a byte"));
            };
            size = size.max(
                byte.checked_add(bytes)
                    .ok_or_else(|| refused("which lies past 4 GiB"))?,
            );
            members.push(Member {
                name: field.name.clone(),
                ty,
                offset: byte,
                flags: 0,
                via: SmolStr::default(),
            });
            bits.push(bit);
        }
        Ok(self.define(
            reserved,
            TypeInfo {
                kind: Kind::Overlap { members, bits },
                size,
                align: 1,
                name: name.clone(),
                rust: format!("S_{}", mangle(name)),
            },
        ))
    }

    /// The type of the structure `name`, registry type `id`, before its members are laid out, so
    /// that a member that refers to it (`next : REF_TO Node`) finds it; [`Self::define`] gives it
    /// its layout.
    fn reserve(&mut self, id: TypeId, name: &SmolStr) -> Ty {
        let ty = self.intern(TypeInfo {
            kind: Kind::Struct {
                members: Vec::new(),
            },
            size: 0,
            align: 1,
            name: name.clone(),
            rust: format!("S_{}", mangle(name)),
        });
        self.by_registry.insert(id, ty);
        ty
    }

    /// Gives the reserved type `ty` its layout `info`.
    fn define(&mut self, ty: Ty, info: TypeInfo) -> Ty {
        if let Some(entry) = self.entries.get_mut(ty.0 as usize) {
            *entry = info;
        }
        ty
    }

    /// The type of a `BOOL` member of a `STRUCT OVERLAP` at bit `bit` of its byte, as the type
    /// table lists it (specification 44 §5.8).
    pub(super) fn bit(&mut self, bit: u8) -> Ty {
        self.intern(TypeInfo {
            kind: Kind::Bit(bit),
            size: 1,
            align: 1,
            name: SmolStr::new_static("BOOL"),
            rust: format!("Bit{bit}"),
        })
    }

    /// The members of the `STRUCT OVERLAP` of `members` and `bits` as the symbol and type tables
    /// list them: a `BOOL` member as the type of its bit.
    pub(super) fn overlap_members(
        &mut self,
        members: &[Member],
        bits: &[Option<u8>],
    ) -> Vec<Member> {
        members
            .iter()
            .zip(bits)
            .map(|(member, bit)| Member {
                ty: bit.map_or(member.ty, |bit| self.bit(bit)),
                ..member.clone()
            })
            .collect()
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
    /// as `first`: the inputs, outputs and in-outs, then the variables. A `RETAIN` variable is a
    /// member like the others: the runtime keeps no member of an instance across a warm restart and
    /// writes none into a checkpoint (specification 11 §6.7), and a compiled program does the same
    /// (specification 44 §11).
    pub(super) fn own_members(
        &mut self,
        owner: Owner<'a>,
        first: Option<(SmolStr, Ty, u8)>,
    ) -> Result<Vec<(SmolStr, Ty, u8)>, NativeError> {
        let mut fields: Vec<(SmolStr, Ty, u8)> = first.into_iter().collect();
        let edges = self.defs.edge_inputs(owner.name());
        for param in owner.params() {
            let ty = self.of(param.type_id)?;
            if param.direction == ParamDirection::In && self.info(ty).kind.is_instance() {
                return Err(NativeError::unsupported(format!(
                    "the function block instance {} as an input of {}",
                    param.name,
                    owner.name()
                )));
            }
            let (ty, flags) = match param.direction {
                ParamDirection::In => (ty, SYMBOL_INPUT),
                ParamDirection::Out => (ty, SYMBOL_OUTPUT),
                // The instance holds a reference to the argument of its last call (§6.4).
                ParamDirection::InOut => (self.reference(ty), SYMBOL_IN_OUT),
            };
            fields.push((param.name.clone(), ty, flags));
            // An edge-qualified input's trigger, which the body reads (IEC §6.5.2.2).
            if let Some(edge) = edges
                .iter()
                .find(|edge| edge.name.eq_ignore_ascii_case(&param.name))
            {
                fields.push((edge_trigger_name(&param.name), self.edge_trigger(edge), 0));
            }
        }
        for var in owner.vars() {
            if var.external {
                continue;
            }
            let ty = self.of(var.type_id)?;
            let mut flags = if var.constant { SYMBOL_CONSTANT } else { 0 };
            if matches!(self.info(ty).kind, Kind::Fb { .. } | Kind::StdFb(_)) {
                flags |= SYMBOL_INSTANCE;
            }
            fields.push((var.name.clone(), ty, flags));
        }
        // A method's `VAR_STAT` variables live in the instance, one of each per instance and method
        // (specification 01 §3), under their hidden names.
        for method in owner.methods() {
            for local in &method.static_locals {
                let ty = self.of(local.type_id)?;
                let mut flags = if local.constant { SYMBOL_CONSTANT } else { 0 };
                if matches!(self.info(ty).kind, Kind::Fb { .. } | Kind::StdFb(_)) {
                    flags |= SYMBOL_INSTANCE;
                }
                fields.push((
                    method_static_name(owner.name(), &method.name, &local.name),
                    ty,
                    flags,
                ));
            }
        }
        Ok(fields)
    }
}

/// The bit offset a relative member address (`%B2`, `%W4`, `%X3.1`) gives, from the start of its
/// structure (IEC §6.4.4.7); `None` for an absolute address (`%IX0.0`, `%Q*`).
fn relative_bit(address: &str) -> Option<u64> {
    let rest = address.trim().strip_prefix('%')?;
    let (unit, offset) = rest.split_at(1);
    if !matches!(unit, "X" | "B" | "W" | "D" | "L") {
        return None;
    }
    let (byte, bit) = match offset.split_once('.') {
        Some((byte, bit)) if unit == "X" => (byte, bit.parse::<u64>().ok().filter(|bit| *bit < 8)?),
        Some(_) => return None,
        None => (offset, 0),
    };
    Some(byte.parse::<u64>().ok()? * 8 + bit)
}

/// The hidden name of the trigger of edge-qualified input `input`, a member beside it, which no
/// IEC name can be (no identifier begins with `__`).
pub(super) fn edge_trigger_name(input: &str) -> SmolStr {
    SmolStr::new(format!("__TRIG_{input}"))
}

impl Types<'_> {
    /// The type of the trigger of `edge`: `R_TRIG` for `R_EDGE`, `F_TRIG` for `F_EDGE`.
    pub(super) fn edge_trigger(&mut self, edge: &crate::program_model::EdgeInput) -> Ty {
        self.std_fb(match edge.qualifier {
            trust_hir::symbols::EdgeQualifier::Rising => StdFb::RTrig,
            trust_hir::symbols::EdgeQualifier::Falling => StdFb::FTrig,
        })
    }
}

/// The hidden name of the `VAR_STAT` variable `local` of method `method` of `owner`, a member of
/// its instances.
pub(super) fn method_static_name(owner: &SmolStr, method: &SmolStr, local: &SmolStr) -> SmolStr {
    crate::program_model::static_storage_name(
        &crate::program_model::method_static_storage_owner(owner, method),
        local,
    )
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
        let (blocks, classes, interfaces, edges) = (
            IndexMap::new(),
            IndexMap::new(),
            IndexMap::new(),
            IndexMap::new(),
        );
        let mut types = Types::new(registry, Defs::new(&blocks, &classes, &interfaces, &edges));
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
        let (blocks, classes, interfaces, edges) = (
            IndexMap::new(),
            IndexMap::new(),
            IndexMap::new(),
            IndexMap::new(),
        );
        let mut types = Types::new(registry, Defs::new(&blocks, &classes, &interfaces, &edges));
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
        let (blocks, classes, interfaces, edges) = (
            IndexMap::new(),
            IndexMap::new(),
            IndexMap::new(),
            IndexMap::new(),
        );
        let mut types = Types::new(registry, Defs::new(&blocks, &classes, &interfaces, &edges));
        let ton = types.std_fb(StdFb::Ton);
        assert_eq!(types.info(ton).size, 48);
        let members = types.std_fb_members(StdFb::Ton);
        let et = members.iter().find(|member| member.name == "ET").unwrap();
        assert_eq!(et.offset, 24);
    }
}
