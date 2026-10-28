//! Function blocks and classes with their inheritance and interfaces in a compiled program
//! (IEC §6.6.5 to §6.6.8; specification 44 §6.2, §6.4): how an instance lies, which class id it
//! holds, which method a call reaches, and which types a dynamic call may meet.
//!
//! A derived type embeds its base as its first field, `__BASE`, so that an instance of it is an
//! instance of its base at the same address. The root of every hierarchy that a call can reach
//! dynamically, one that implements an interface or is extended, holds the instance's class id in
//! its first field, `__CLASS`; every type of such a hierarchy has an id of its own. The build knows
//! every type of the program (a closed world): a call through an interface, a `VAR_IN_OUT`, a
//! reference or `THIS` is a `match` on the class id over the types that can be there.

use indexmap::IndexMap;
use smol_str::SmolStr;

use super::types::{lay_out, mangle, Elem, Kind, Member, Ty, TypeInfo, Types};
use super::NativeError;
use crate::program_model::{
    ClassDef, EdgeInput, FunctionBlockBase, FunctionBlockDef, InterfaceDef, MethodDef, Param,
    VarDef,
};

/// The project's function blocks, classes and interfaces, by their names in capitals.
#[derive(Clone, Copy)]
pub(in crate::native) struct Defs<'a> {
    function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
    classes: &'a IndexMap<SmolStr, ClassDef>,
    interfaces: &'a IndexMap<SmolStr, InterfaceDef>,
    edges: &'a IndexMap<SmolStr, Vec<EdgeInput>>,
}

impl<'a> Defs<'a> {
    pub(in crate::native) fn new(
        function_blocks: &'a IndexMap<SmolStr, FunctionBlockDef>,
        classes: &'a IndexMap<SmolStr, ClassDef>,
        interfaces: &'a IndexMap<SmolStr, InterfaceDef>,
        edges: &'a IndexMap<SmolStr, Vec<EdgeInput>>,
    ) -> Self {
        Self {
            function_blocks,
            classes,
            interfaces,
            edges,
        }
    }

    /// The edge-qualified inputs of the program or function block named `owner` (IEC §6.5.2.2).
    pub(in crate::native) fn edge_inputs(&self, owner: &str) -> &'a [EdgeInput] {
        self.edges
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(owner))
            .map_or(&[], |(_, inputs)| inputs.as_slice())
    }

    /// The function block or class named `name`, compared without case.
    pub(in crate::native) fn owner(&self, name: &str) -> Option<Owner<'a>> {
        let named = |key: &SmolStr, def_name: &SmolStr| {
            key.eq_ignore_ascii_case(name) || def_name.eq_ignore_ascii_case(name)
        };
        self.function_blocks
            .iter()
            .find(|(key, def)| named(key, &def.name))
            .map(|(_, def)| Owner::Fb(def))
            .or_else(|| {
                self.classes
                    .iter()
                    .find(|(key, def)| named(key, &def.name))
                    .map(|(_, def)| Owner::Class(def))
            })
    }

    /// The interface named `name`, compared without case.
    pub(in crate::native) fn interface(&self, name: &str) -> Option<&'a InterfaceDef> {
        self.interfaces
            .iter()
            .find(|(key, def)| {
                key.eq_ignore_ascii_case(name) || def.name.eq_ignore_ascii_case(name)
            })
            .map(|(_, def)| def)
    }

    /// Every function block and class.
    fn owners(&self) -> impl Iterator<Item = Owner<'a>> + '_ {
        self.function_blocks
            .values()
            .map(Owner::Fb)
            .chain(self.classes.values().map(Owner::Class))
    }

    /// The base of `owner`, when it extends one.
    pub(in crate::native) fn base(&self, owner: Owner<'a>) -> Option<Owner<'a>> {
        self.owner(owner.base_name()?)
    }

    /// Whether `owner` is `ancestor` or derived from it.
    pub(in crate::native) fn is_a(&self, owner: Owner<'a>, ancestor: Owner<'a>) -> bool {
        let mut current = Some(owner);
        // A chain of more types than the project has is a cycle, which trust-hir refuses.
        for _ in 0..=self.function_blocks.len() + self.classes.len() {
            let Some(here) = current else {
                return false;
            };
            if here.same(ancestor) {
                return true;
            }
            current = self.base(here);
        }
        false
    }

    /// The declared names of the project's interfaces.
    fn interface_names(&self) -> impl Iterator<Item = &'a SmolStr> + '_ {
        self.interfaces.values().map(|def| &def.name)
    }

    /// The project's interfaces, in the order of their declarations.
    pub(in crate::native) fn interfaces(&self) -> impl Iterator<Item = &'a InterfaceDef> + '_ {
        self.interfaces.values()
    }

    /// Whether `interface` is `wanted` or extends it.
    fn interface_is_a(&self, interface: &str, wanted: &str) -> bool {
        let mut current = self.interface(interface);
        for _ in 0..=self.interfaces.len() {
            let Some(here) = current else {
                return false;
            };
            if here.name.eq_ignore_ascii_case(wanted) {
                return true;
            }
            current = here.base.as_deref().and_then(|base| self.interface(base));
        }
        false
    }

    /// Whether `owner` implements interface `wanted`: it or a base names it, or an interface that
    /// extends it (IEC §6.6.6.4).
    pub(in crate::native) fn implements(&self, owner: Owner<'a>, wanted: &str) -> bool {
        let mut current = Some(owner);
        for _ in 0..=self.function_blocks.len() + self.classes.len() {
            let Some(here) = current else {
                return false;
            };
            if here
                .interfaces()
                .iter()
                .any(|interface| self.interface_is_a(interface, wanted))
            {
                return true;
            }
            current = self.base(here);
        }
        false
    }

    /// Whether some type extends `owner`.
    fn is_extended(&self, owner: Owner<'a>) -> bool {
        self.owners()
            .any(|other| self.base(other).is_some_and(|base| base.same(owner)))
    }

    /// Whether instances of `owner` hold a class id: it or a type of its hierarchy implements an
    /// interface or extends another, so that a call can reach it dynamically (specification 44 §6.4).
    pub(in crate::native) fn has_class(&self, owner: Owner<'a>) -> bool {
        self.base(owner).is_some() || !owner.interfaces().is_empty() || self.is_extended(owner)
    }

    /// The method named `name` that a call on an instance of `owner` reaches: its own, else its
    /// base's, with the type that declares it.
    pub(in crate::native) fn method(
        &self,
        owner: Owner<'a>,
        name: &str,
    ) -> Option<(Owner<'a>, &'a MethodDef)> {
        let mut current = Some(owner);
        for _ in 0..=self.function_blocks.len() + self.classes.len() {
            let here = current?;
            if let Some(method) = here
                .methods()
                .iter()
                .find(|method| method.name.eq_ignore_ascii_case(name))
            {
                return Some((here, method));
            }
            current = self.base(here);
        }
        None
    }

    /// The method named `name` of interface `interface` or of an interface it extends.
    pub(in crate::native) fn interface_method(
        &self,
        interface: &str,
        name: &str,
    ) -> Option<&'a MethodDef> {
        let mut current = self.interface(interface);
        for _ in 0..=self.interfaces.len() {
            let here = current?;
            if let Some(method) = here
                .methods
                .iter()
                .find(|method| method.name.eq_ignore_ascii_case(name))
            {
                return Some(method);
            }
            current = here.base.as_deref().and_then(|base| self.interface(base));
        }
        None
    }

    /// Whether a type derived from `owner` declares a method named `name` of its own, which a call
    /// on a variable of type `owner` may then reach dynamically (IEC §6.6.8).
    pub(in crate::native) fn overridden_below(&self, owner: Owner<'a>, name: &str) -> bool {
        self.owners().any(|other| {
            !other.same(owner)
                && self.is_a(other, owner)
                && other
                    .methods()
                    .iter()
                    .any(|method| method.name.eq_ignore_ascii_case(name))
        })
    }

    /// Whether a function block derived from `owner` exists, whose own body a call of an instance
    /// of type `owner` may then reach dynamically (IEC §6.6.8.3, §6.6.8.4).
    pub(in crate::native) fn extended_below(&self, owner: Owner<'a>) -> bool {
        self.owners()
            .any(|other| !other.same(owner) && self.is_a(other, owner))
    }
}

/// A function block's or a class's definition: what its instances are made of.
#[derive(Clone, Copy)]
pub(in crate::native) enum Owner<'a> {
    Fb(&'a FunctionBlockDef),
    Class(&'a ClassDef),
}

impl<'a> Owner<'a> {
    pub(in crate::native) fn name(self) -> &'a SmolStr {
        match self {
            Self::Fb(def) => &def.name,
            Self::Class(def) => &def.name,
        }
    }

    /// The name of the base it extends.
    fn base_name(self) -> Option<&'a SmolStr> {
        match self {
            Self::Fb(def) => def.base.as_ref().map(|base| match base {
                FunctionBlockBase::FunctionBlock(name) | FunctionBlockBase::Class(name) => name,
            }),
            Self::Class(def) => def.base.as_ref(),
        }
    }

    fn interfaces(self) -> &'a [SmolStr] {
        match self {
            Self::Fb(def) => &def.interfaces,
            Self::Class(def) => &def.interfaces,
        }
    }

    pub(in crate::native) fn vars(self) -> &'a [VarDef] {
        match self {
            Self::Fb(def) => &def.vars,
            Self::Class(def) => &def.vars,
        }
    }

    /// The inputs, outputs and in-outs: a class has none.
    pub(in crate::native) fn params(self) -> &'a [Param] {
        match self {
            Self::Fb(def) => &def.params,
            Self::Class(_) => &[],
        }
    }

    pub(in crate::native) fn methods(self) -> &'a [MethodDef] {
        match self {
            Self::Fb(def) => &def.methods,
            Self::Class(def) => &def.methods,
        }
    }

    pub(in crate::native) fn is_class(self) -> bool {
        matches!(self, Self::Class(_))
    }

    /// Whether `self` and `other` are one definition.
    pub(in crate::native) fn same(self, other: Self) -> bool {
        match (self, other) {
            (Self::Fb(a), Self::Fb(b)) => std::ptr::eq(a, b),
            (Self::Class(a), Self::Class(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }

    /// The Rust type of its instances.
    pub(in crate::native) fn rust(self) -> String {
        format!("F_{}", mangle(self.name()))
    }
}

impl<'a> Types<'a> {
    /// The function block or class named `name`.
    pub(super) fn owner(&self, name: &str) -> Option<Owner<'a>> {
        self.defs.owner(name)
    }

    /// The type of the instances of `owner`: its base first, as `__BASE`, or its class id, as
    /// `__CLASS`, when its hierarchy needs one; then its own variables (specification 44 §6.2).
    pub(super) fn class_like(&mut self, owner: Owner<'a>) -> Result<Ty, NativeError> {
        let rust = owner.rust();
        if let Some(ty) = self.by_rust(&rust) {
            return Ok(ty);
        }
        let name = owner.name();
        let base = match owner.base_name() {
            Some(base_name) if super::types::StdFb::is_standard(base_name) => {
                return Err(NativeError::unsupported(format!(
                    "{name} EXTENDS the standard function block {base_name}"
                )));
            }
            Some(base_name) => {
                let Some(base) = self.defs.owner(base_name) else {
                    return Err(NativeError::Compile(format!(
                        "the base {base_name} of {name} is not declared"
                    )));
                };
                Some(self.class_like(base)?)
            }
            None => None,
        };
        let has_class = self.defs.has_class(owner);
        let first = match base {
            Some(base) => Some((SmolStr::new_static("__BASE"), base, 0)),
            None if has_class => Some((SmolStr::new_static("__CLASS"), self.elem(Elem::UDInt), 0)),
            None => None,
        };
        let fields = self.own_members(owner, first)?;
        let (mut members, size, align) = lay_out(fields, self);
        if let Some(base) = base {
            // The base's variables, which lie inside `__BASE` at offset 0.
            let inherited: Vec<Member> = self
                .info(base)
                .members()
                .iter()
                .map(|member| Member {
                    via: SmolStr::new(format!("__BASE.{}", member.via)),
                    ..member.clone()
                })
                .collect();
            members.splice(0..1, inherited);
        }
        // The id is the type's place among the types with an id, which its members' types, laid
        // out above, come before.
        let class = if has_class {
            u32::try_from(self.class_ids.len() + 1).unwrap_or(u32::MAX)
        } else {
            0
        };
        let ty = self.intern(TypeInfo {
            kind: Kind::Fb {
                def: name.clone(),
                members,
                base,
                class,
                is_class: owner.is_class(),
            },
            size,
            align,
            name: name.clone(),
            rust,
        });
        if has_class {
            self.class_ids.push(ty);
        }
        Ok(ty)
    }

    /// Every interface that the function block or class `def` implements, itself, through a base or
    /// through an interface that extends one (IEC §6.6.6.4), by their declared names: what its type
    /// record lists for the tools (specification 44 §5.8).
    pub(super) fn implemented_interfaces(&self, def: &str) -> Vec<SmolStr> {
        let Some(owner) = self.defs.owner(def) else {
            return Vec::new();
        };
        self.defs
            .interface_names()
            .filter(|name| self.defs.implements(owner, name))
            .cloned()
            .collect()
    }

    /// The type of a variable of interface `name`: a reference to an instance (specification 44
    /// §7.3), 16 bytes.
    pub(super) fn interface(&mut self, name: &SmolStr) -> Result<Ty, NativeError> {
        let Some(def) = self.defs.interface(name) else {
            return Err(NativeError::Compile(format!(
                "interface {name} is not declared"
            )));
        };
        Ok(self.intern(TypeInfo {
            kind: Kind::Interface,
            size: 16,
            align: 4,
            name: def.name.clone(),
            rust: "Cell<abi::RefValue>".into(),
        }))
    }

    /// The definition of the instances of type `ty`, a user function block or class.
    pub(super) fn owner_of(&self, ty: Ty) -> Option<Owner<'a>> {
        match &self.info(ty).kind {
            Kind::Fb { def, .. } => self.defs.owner(def),
            _ => None,
        }
    }

    /// The path from an instance of type `from` to its part of type `to`, a base of it: `.__BASE`
    /// once per level, empty for the type itself; `None` when `to` is no base of `from`.
    pub(super) fn upcast(&self, from: Ty, to: Ty) -> Option<String> {
        let mut path = String::new();
        let mut current = from;
        for _ in 0..=self.class_ids.len() + 1 {
            if current == to {
                return Some(path);
            }
            let Kind::Fb {
                base: Some(base), ..
            } = self.info(current).kind
            else {
                return None;
            };
            path.push_str(".__BASE");
            current = base;
        }
        None
    }

    /// The types with a class id and their ids, in the order of the ids.
    pub(super) fn classes(&self) -> Vec<(Ty, u32)> {
        self.class_ids
            .iter()
            .filter_map(|ty| match self.info(*ty).kind {
                Kind::Fb { class, .. } => Some((*ty, class)),
                _ => None,
            })
            .collect()
    }

    /// The path from an instance of type `ty` to the class id its hierarchy's root holds.
    pub(super) fn class_field(&self, ty: Ty) -> Option<String> {
        let mut path = String::new();
        let mut current = ty;
        for _ in 0..=self.class_ids.len() + 1 {
            match self.info(current).kind {
                Kind::Fb {
                    base: Some(base), ..
                } => {
                    path.push_str(".__BASE");
                    current = base;
                }
                Kind::Fb { class, .. } if class != 0 => {
                    path.push_str(".__CLASS");
                    return Some(path);
                }
                _ => return None,
            }
        }
        None
    }
}

impl Types<'_> {
    /// Whether a value of type `ty` holds an instance with a class id: itself, an element or a
    /// member.
    pub(super) fn holds_class(&self, ty: Ty) -> bool {
        match &self.info(ty).kind {
            Kind::Fb { class, members, .. } => {
                *class != 0 || members.iter().any(|member| self.holds_class(member.ty))
            }
            Kind::Array { elem, .. } => self.holds_class(*elem),
            Kind::Struct { members } => members.iter().any(|member| self.holds_class(member.ty)),
            _ => false,
        }
    }
}
