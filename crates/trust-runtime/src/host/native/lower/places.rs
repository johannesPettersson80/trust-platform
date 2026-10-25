//! Places (specification 44 §6.4, §7.3): what a name, a member, an element, a dereference, an
//! access path or `THIS` names in the frame of the POU being lowered, as a Rust expression of its
//! storage with its type.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::types::{rust_field, Elem, Kind, Member, Ty};
use super::super::NativeError;
use super::{call, Frame, Lowerer, Place};

impl Lowerer<'_, '_> {
    /// The place `node` names: a variable, a member, an element, `THIS`; a property's value read
    /// into a temporary.
    pub(in crate::native) fn place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        if let Some(place) = self.object_place(node) {
            return place;
        }
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.place(&inner),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::NameRef => {
                if let Some(place) = self.property_read(node)? {
                    return Ok(place);
                }
                let name = call::ident(node);
                self.name_place(node, &name)
            }
            SyntaxKind::FieldExpr => {
                if let Some(place) = self.property_read(node)? {
                    return Ok(place);
                }
                self.field_place(node)
            }
            SyntaxKind::IndexExpr => self.index_place(node),
            SyntaxKind::DerefExpr => self.deref_place(node),
            SyntaxKind::CallExpr => self.result_instance(node),
            other => Err(self.unsupported(node, format!("a {other:?} as a place"))),
        }
    }

    /// The instance `value` names when a reference to an instance of type `target` takes it:
    /// `THIS`, `SUPER` or a value of a function block's type; `None` for a reference value.
    pub(super) fn instance_source(
        &mut self,
        target: Ty,
        value: &SyntaxNode,
    ) -> Result<Option<Place>, NativeError> {
        if !self.info(target).kind.is_instance() {
            return Ok(None);
        }
        if let Some(place) = self.object_place(value) {
            return place.map(Some);
        }
        if self
            .type_of(value)
            .is_ok_and(|ty| self.info(ty).kind.is_instance())
        {
            return self.place(value).map(Some);
        }
        Ok(None)
    }

    /// The instance the result of call `node` names, a call of a function block's type: the
    /// receiver of a chained call `b.SetVolume(10).Build()` (§6.4).
    pub(super) fn result_instance(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let ty = self.type_of(node)?;
        if !self.info(ty).kind.is_instance() {
            return Err(self.unsupported(node, "a call as a place"));
        }
        let reference = self.model.types.reference(ty);
        let result = self.call_aggregate(node, reference)?;
        Ok(self.dereference(&format!("{}.get()", result.expr), ty))
    }

    /// `reference^`: the variable a reference names, checked as specification 44 §7.3 says.
    pub(super) fn deref_place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let Some(reference) = node.children().next() else {
            return Err(self.unsupported(node, "a dereference without its reference"));
        };
        let ty = self.type_of(&reference)?;
        let Kind::Ref { target, .. } = self.info(ty).kind else {
            return Err(self.unsupported(node, "a dereference of a value that is no reference"));
        };
        if matches!(self.info(target).kind, Kind::Open { .. }) {
            let place = self.place(&reference)?;
            return Ok(self.open_target(&place, target));
        }
        let value = self.reference(&reference)?;
        Ok(self.dereference(&value, target))
    }

    /// The variable of type `target` the reference value `reference` names, bound to a temporary.
    pub(in crate::native) fn dereference(&mut self, reference: &str, target: Ty) -> Place {
        let storage = self.info(target).rust.clone();
        let expr = self.bind(&format!("w.areas.deref::<{storage}>(cx, &{reference})?"));
        Place {
            expr,
            ty: target,
            is_ref: true,
        }
    }

    /// The variable `name` as `node` names it: the frame's own, the instance's, a global; a
    /// directly represented variable is the global of its address's name (IEC §6.5.5.1).
    pub(in crate::native) fn name_place(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> Result<Place, NativeError> {
        let direct = name
            .starts_with('%')
            .then(|| crate::io::IoAddress::parse(name).ok())
            .flatten()
            .map(|address| super::super::located::address_text(&address));
        let name = direct.as_deref().unwrap_or(name);
        if let Some(local) = self
            .locals
            .iter()
            .rev()
            .find(|local| local.name.eq_ignore_ascii_case(name))
        {
            return Ok(Place {
                expr: local.rust.clone(),
                ty: local.ty,
                is_ref: local.is_ref,
            });
        }
        match self.frame {
            Frame::Program(index) => {
                let program = &self.model.program_types[index];
                if let Some((member, retained)) = program.member(name) {
                    let owner = if retained { "mr" } else { "me" };
                    return Ok(Place {
                        expr: format!("{owner}.{}", member.field()),
                        ty: member.ty,
                        is_ref: false,
                    });
                }
            }
            Frame::FunctionBlock(ty) | Frame::Method(ty) => {
                if let Some(member) = self
                    .info(ty)
                    .members()
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(name))
                    .cloned()
                {
                    // A `VAR_IN_OUT` is the variable its stored reference names (§6.4).
                    if member.flags & trust_runtime_core::native::image::SYMBOL_IN_OUT != 0
                        && let Kind::Ref { target, .. } = self.info(member.ty).kind
                    {
                        if matches!(self.info(target).kind, Kind::Open { .. }) {
                            let reference = Place {
                                expr: format!("me.{}", member.field()),
                                ty: member.ty,
                                is_ref: false,
                            };
                            return Ok(self.open_target(&reference, target));
                        }
                        let reference = format!("me.{}.get()", member.field());
                        return Ok(self.dereference(&reference, target));
                    }
                    return Ok(Place {
                        expr: format!("me.{}", member.field()),
                        ty: member.ty,
                        is_ref: false,
                    });
                }
            }
            Frame::Function => {}
        }
        // A global by its name, or by the qualified name of the global of a namespace that
        // trust-hir resolved the name to (IEC §6.9).
        let qualified = self.qualified_name(node);
        let global = self.model.global(name).or_else(|| {
            qualified
                .as_deref()
                .and_then(|name| self.model.global(name))
        });
        if let Some(global) = global {
            let area = if global.retained { "w.r" } else { "w.g" };
            return Ok(Place {
                expr: format!("{area}.{}", rust_field(&global.name)),
                ty: global.ty,
                is_ref: false,
            });
        }
        // An access path of the body's own program names its variable in the instance that runs
        // the body: each instance has its own (IEC §6.5.2.2, Table 47).
        if let Frame::Program(index) = self.frame
            && let Some(access) = self
                .model
                .program_access
                .iter()
                .find(|access| {
                    access.name.eq_ignore_ascii_case(name)
                        && declared_by(access, &self.model.program_types[index].def.name)
                })
                .cloned()
        {
            return self.own_access_place(node, index, &access);
        }
        if let Some(access) = self
            .model
            .access
            .iter()
            .find(|access| access.name.eq_ignore_ascii_case(name))
            .cloned()
        {
            return self.access_place(node, &access);
        }
        Err(self.unsupported(node, format!("the name {name}")))
    }

    /// The variable that the access path `access` of the program type `index`, the body's own,
    /// names in the instance that runs the body: its path starts with the type.
    fn own_access_place(
        &mut self,
        node: &SyntaxNode,
        index: usize,
        access: &crate::harness::AccessDecl,
    ) -> Result<Place, NativeError> {
        use crate::harness::{AccessPart, AccessPath};
        let AccessPath::Parts(parts) = &access.path else {
            return Err(self.unsupported(node, "an access path to a direct address"));
        };
        let Some((AccessPart::Name(variable), rest)) =
            parts.get(1..).and_then(<[AccessPart]>::split_first)
        else {
            return Err(self.unsupported(node, format!("the access path {}", access.name)));
        };
        let Some((member, retained)) = self.model.program_types[index].member(variable) else {
            return Err(self.unsupported(node, format!("the access path {}", access.name)));
        };
        let owner = if retained { "mr" } else { "me" };
        let place = Place {
            expr: format!("{owner}.{}", member.field()),
            ty: member.ty,
            is_ref: false,
        };
        self.access_rest(node, &access.name, place, rest)
    }

    /// The variable the access path `access` names (IEC §6.5.2.2; specification 03 §2): a program
    /// instance's variable or a global, then its members and its elements at constant indices.
    pub(super) fn access_place(
        &mut self,
        node: &SyntaxNode,
        access: &crate::harness::AccessDecl,
    ) -> Result<Place, NativeError> {
        use crate::harness::{AccessPart, AccessPath};
        let names_none =
            |lowerer: &Self| lowerer.unsupported(node, format!("the access path {}", access.name));
        let AccessPath::Parts(parts) = &access.path else {
            return Err(self.unsupported(node, "an access path to a direct address"));
        };
        let Some((AccessPart::Name(first), mut rest)) = parts.split_first() else {
            return Err(names_none(self));
        };
        let instance = self
            .model
            .instances
            .iter()
            .find(|instance| instance.name.eq_ignore_ascii_case(first))
            .map(|instance| (instance.name.clone(), instance.program));
        let place = if let Some((instance, program)) = instance {
            let Some((AccessPart::Name(variable), tail)) = rest.split_first() else {
                return Err(names_none(self));
            };
            rest = tail;
            let Some((member, retained)) = self.model.program_types[program].member(variable)
            else {
                return Err(names_none(self));
            };
            let area = if retained { "w.r" } else { "w.p" };
            Place {
                expr: format!("{area}.{}.{}", rust_field(&instance), member.field()),
                ty: member.ty,
                is_ref: false,
            }
        } else if let Some(global) = self.model.global(first) {
            let area = if global.retained { "w.r" } else { "w.g" };
            Place {
                expr: format!("{area}.{}", rust_field(&global.name)),
                ty: global.ty,
                is_ref: false,
            }
        } else {
            return Err(names_none(self));
        };
        self.access_rest(node, &access.name, place, rest)
    }

    /// The global variable, or the member or element of one, that `parts` name: a global's name,
    /// then its members and constant indices (IEC §6.8.2, `Global_Var_Access`); `None` when the
    /// first part names no global. `label` names the path in a refusal.
    pub(in crate::native) fn global_place(
        &mut self,
        node: &SyntaxNode,
        label: &str,
        parts: &[crate::harness::AccessPart],
    ) -> Result<Option<Place>, NativeError> {
        let Some((crate::harness::AccessPart::Name(first), rest)) = parts.split_first() else {
            return Ok(None);
        };
        let Some(global) = self.model.global(first) else {
            return Ok(None);
        };
        let area = if global.retained { "w.r" } else { "w.g" };
        let place = Place {
            expr: format!("{area}.{}", rust_field(&global.name)),
            ty: global.ty,
            is_ref: false,
        };
        self.access_rest(node, label, place, rest).map(Some)
    }

    /// The member or element of `place` that the rest `rest` of the access path `access` names:
    /// its members and its elements at constant indices.
    fn access_rest(
        &mut self,
        node: &SyntaxNode,
        access: &str,
        mut place: Place,
        rest: &[crate::harness::AccessPart],
    ) -> Result<Place, NativeError> {
        use crate::harness::AccessPart;
        let names_none =
            |lowerer: &Self| lowerer.unsupported(node, format!("the access path {access}"));
        for part in rest {
            let info = self.info(place.ty).clone();
            place = match (part, &info.kind) {
                (AccessPart::Name(name), Kind::Struct { members } | Kind::Fb { members, .. }) => {
                    let Some(member) = members
                        .iter()
                        .find(|member| member.name.eq_ignore_ascii_case(name))
                    else {
                        return Err(names_none(self));
                    };
                    Place {
                        expr: format!("{}.{}", place.expr, member.field()),
                        ty: member.ty,
                        is_ref: false,
                    }
                }
                (AccessPart::Index(indices), Kind::Array { elem, dims })
                    if indices.len() == dims.len() =>
                {
                    let mut expr = format!("({})", place.expr);
                    for (index, (lower, upper)) in indices.iter().zip(dims) {
                        if !(lower..=upper).contains(&index) {
                            return Err(names_none(self));
                        }
                        expr = format!("{expr}[{}]", index - lower);
                    }
                    Place {
                        expr,
                        ty: *elem,
                        is_ref: false,
                    }
                }
                _ => return Err(names_none(self)),
            };
        }
        Ok(place)
    }

    /// The name of the symbol trust-hir resolved `node` to, qualified by the namespaces that hold
    /// it (`GVL.shared`); `None` when it resolved none or no namespace holds it.
    pub(super) fn qualified_name(&self, node: &SyntaxNode) -> Option<String> {
        let symbols = &self.src.analysis.symbols;
        let symbol = symbols.get(self.src.symbol_of(node)?)?;
        let mut parts = vec![symbol.name.to_string()];
        let mut parent = symbol.parent;
        while let Some(id) = parent {
            let holder = symbols.get(id)?;
            if !matches!(holder.kind, trust_hir::SymbolKind::Namespace) {
                break;
            }
            parts.push(holder.name.to_string());
            parent = holder.parent;
        }
        if parts.len() == 1 {
            return None;
        }
        parts.reverse();
        Some(parts.join("."))
    }

    pub(super) fn field_place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let mut children = node.children();
        let (Some(base), Some(member)) = (children.next(), children.next()) else {
            return Err(self.unsupported(node, "a member access without a member"));
        };
        let name = call::ident(&member);
        if self.namespace_qualified(node) {
            return self.name_place(node, &name);
        }
        let base = self.place(&base)?;
        let info = self.info(base.ty).clone();
        if let Kind::Overlap { members, bits } = &info.kind {
            return self.overlap_member(node, &base, members, bits, &name);
        }
        let members = match &info.kind {
            Kind::Struct { members } | Kind::Fb { members, .. } => members.clone(),
            Kind::StdFb(fb) => self.model.types.std_fb_members(*fb),
            _ => return Err(self.unsupported(node, "a member of a value without members")),
        };
        let Some(member) = members
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(&name))
        else {
            return Err(self.unsupported(node, format!("the member {name}")));
        };
        let expr = format!("{}.{}", base.expr, member.field());
        Ok(Place {
            expr,
            ty: member.ty,
            is_ref: false,
        })
    }

    /// The place of member `name` of `base`, a `STRUCT OVERLAP` or a region of a backing: the view
    /// of the bytes it takes of the backing, of its bit, or the region of a structure or an array
    /// that lies there (specification 02 §3.6; IEC §6.4.4.7.1).
    pub(super) fn overlap_member(
        &self,
        node: &SyntaxNode,
        base: &Place,
        members: &[Member],
        bits: &[Option<u8>],
        name: &str,
    ) -> Result<Place, NativeError> {
        let Some(index) = members
            .iter()
            .position(|member| member.name.eq_ignore_ascii_case(name))
        else {
            return Err(self.unsupported(node, format!("the member {name}")));
        };
        let member = &members[index];
        let start = member.offset;
        let region = if base.is_overlap_region() {
            base.expr.clone()
        } else {
            format!("ov::Region::new(&{}.B, 0)", base.expr)
        };
        let expr = match bits.get(index).copied().flatten() {
            Some(bit) => format!("ov::Bit::in_region({region}, {start}, {bit})"),
            None => match self.info(member.ty).scalar() {
                Some(elem) => format!("ov::At::<{}>::in_region({region}, {start})", elem.rust()),
                None => format!("ov::Region::of({region}.sub({start}))"),
            },
        };
        Ok(Place {
            expr,
            ty: member.ty,
            is_ref: false,
        })
    }

    /// Whether `node` is a name qualified by a namespace (`Control.LoopMode`), which trust-hir
    /// resolved to a symbol of its own.
    pub(super) fn namespace_qualified(&self, node: &SyntaxNode) -> bool {
        let Some(symbol) = self.src.symbol_of(node) else {
            return false;
        };
        let symbols = &self.src.analysis.symbols;
        symbols
            .get(symbol)
            .and_then(|symbol| symbol.parent)
            .and_then(|parent| symbols.get(parent))
            .is_some_and(|parent| matches!(parent.kind, trust_hir::SymbolKind::Namespace))
    }

    /// The value of the index `index` when it is an integer literal, which the build can compare
    /// with the bounds (§6.5).
    pub(super) fn constant_index(&mut self, index: &SyntaxNode, ty: Ty) -> Option<i64> {
        if index.kind() != SyntaxKind::Literal {
            return None;
        }
        self.constant_integer(index, ty)
            .ok()
            .and_then(|value| i64::try_from(value).ok())
    }

    /// `array[i, j]`: each index checked against its dimension's bounds (§6.5), one nesting of
    /// Rust arrays per dimension.
    pub(super) fn index_place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let mut children = node.children();
        let Some(base) = children.next() else {
            return Err(self.unsupported(node, "an index without an array"));
        };
        let array = self.place(&base)?;
        let indices: Vec<SyntaxNode> = children.collect();
        if matches!(self.info(array.ty).kind, Kind::Open { .. }) {
            return self.open_element(node, &array, &indices);
        }
        if array.is_overlap_region() {
            return self.region_element(node, &array, &indices);
        }
        let info = self.info(array.ty).clone();
        let Kind::Array { elem, dims } = &info.kind else {
            return Err(self.unsupported(node, "an index of a value that is no array"));
        };
        if indices.len() != dims.len() {
            return Err(self.unsupported(node, "an index list unlike the array's dimensions"));
        }
        let lint = self.model.types.elem(Elem::LInt);
        let mut current = array.reference();
        for (index, (lower, upper)) in indices.iter().zip(dims) {
            // A constant inside the bounds is not checked (§6.5): it indexes the Rust array, whose
            // bound the compiler checks when it builds the module.
            if let Some(constant) = self
                .constant_index(index, lint)
                .filter(|constant| (*lower..=*upper).contains(constant))
            {
                current = self.bind(&format!("&({current})[{}]", constant - lower));
                continue;
            }
            let value = self.scalar_to(index, lint)?;
            current = self.bind(&format!("lib::at(cx, {current}, {value}, {lower})?"));
        }
        Ok(Place {
            expr: current,
            ty: *elem,
            is_ref: true,
        })
    }
}

/// Whether the program type `program` declares the access path `access`: the path of a program's
/// own access path starts with its type.
fn declared_by(access: &crate::harness::AccessDecl, program: &str) -> bool {
    use crate::harness::{AccessPart, AccessPath};
    let AccessPath::Parts(parts) = &access.path else {
        return false;
    };
    matches!(parts.first(), Some(AccessPart::Name(owner)) if owner.eq_ignore_ascii_case(program))
}
