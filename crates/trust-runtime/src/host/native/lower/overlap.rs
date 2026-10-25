//! The structures and arrays that lie in a `STRUCT OVERLAP`'s backing (IEC 61131-3 §6.4.4.7.1, its
//! example `Com_data`; specification 02 §3.6): a region of the backing's bytes in the layout of its
//! type's region type (`Types::region`), whose members and elements are views of those bytes. A
//! whole structure or array is copied into a region, and out of it, member by member between its own
//! type and its region type; a region of the same layout has its bytes copied.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::{Lowerer, Place};
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// The element of the array region `array` that `indices` name: the view of an elementary
    /// element, or the region of a structure; an index outside the array faults
    /// (`IndexOutOfBounds`, specification 10 §2.1).
    pub(super) fn region_element(
        &mut self,
        node: &SyntaxNode,
        array: &Place,
        indices: &[SyntaxNode],
    ) -> Result<Place, NativeError> {
        let Kind::Array { elem, dims } = self.info(array.ty).kind.clone() else {
            return Err(self.unsupported(node, "an index of a value that is no array"));
        };
        if indices.len() != dims.len() {
            return Err(self.unsupported(node, "an index list unlike the array's dimensions"));
        }
        // The bytes of one step of each dimension; the last dimension's is the element's.
        let mut strides = vec![0usize; dims.len()];
        let mut stride = self.info(elem).size as usize;
        for (slot, (lower, upper)) in strides.iter_mut().zip(&dims).rev() {
            *slot = stride;
            stride = stride.saturating_mul(length(*lower, *upper));
        }
        let lint = self.model.types.elem(Elem::LInt);
        let mut region = array.expr.clone();
        for ((index, (lower, upper)), stride) in indices.iter().zip(&dims).zip(&strides) {
            if let Some(constant) = self
                .constant_index(index, lint)
                .filter(|constant| (*lower..=*upper).contains(constant))
            {
                let offset = usize::try_from(constant - lower)
                    .unwrap_or(0)
                    .saturating_mul(*stride);
                region = format!("ov::Region::of({region}.sub({offset}))");
                continue;
            }
            let value = self.scalar_to(index, lint)?;
            let count = length(*lower, *upper);
            let element = self.bind(&format!(
                "{region}.element(cx, {value}, {lower}, {count}, {stride})?"
            ));
            region = format!("ov::Region::of({element})");
        }
        Ok(Place {
            expr: self.view(&region, elem, 0, None).unwrap_or(region),
            ty: elem,
            is_ref: false,
        })
    }

    /// `place := value` for `place`, a region: a value of the same layout has its bytes copied, a
    /// value of the region type's own type is copied in member by member.
    pub(super) fn store_region(
        &mut self,
        place: &Place,
        value: &SyntaxNode,
    ) -> Result<(), NativeError> {
        let natural = self.model.types.natural(place.ty);
        let source = self.region_source(value, place.ty, natural)?;
        let same_bytes = source.ty == place.ty
            && (source.is_overlap_region()
                || matches!(self.info(place.ty).kind, Kind::Overlap { .. }));
        if same_bytes {
            let size = self.info(place.ty).size;
            let from = bytes(&source, size);
            self.line(&format!(
                "ov::copy({}.bytes(0, {size}), {from});",
                place.expr
            ));
        } else {
            self.copy_region(&place.expr, &source, place.ty, true);
        }
        Ok(())
    }

    /// The value of the region `place` as a value of `ty`, its region type's own type: a temporary
    /// that its members are copied into.
    pub(super) fn region_value(&mut self, place: &Place, ty: Ty) -> Place {
        let temp = self.temp();
        let storage = self.info(ty).rust.clone();
        self.line(&format!(
            "let {temp}: {storage} = <{storage} as Plain>::ZEROED;"
        ));
        let value = Place {
            expr: temp,
            ty,
            is_ref: false,
        };
        self.copy_region(&place.expr, &value, place.ty, false);
        value
    }

    /// The value that `value` assigns to a region of type `region`, whose own type is `natural`: a
    /// place of either type, or a function's result of the own type.
    fn region_source(
        &mut self,
        value: &SyntaxNode,
        region: Ty,
        natural: Option<Ty>,
    ) -> Result<Place, NativeError> {
        match value.kind() {
            SyntaxKind::ParenExpr => match value.children().next() {
                Some(inner) => self.region_source(&inner, region, natural),
                None => Err(self.unsupported(value, "an empty parenthesis")),
            },
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                let place = self.place(value)?;
                if place.ty == region || Some(place.ty) == natural {
                    return Ok(place);
                }
                Err(self.unsupported(value, "an assignment between different types"))
            }
            _ => self.aggregate(value, natural.unwrap_or(region)),
        }
    }

    /// Copies between the region `region` of the region type `region_ty` and `own`, a place of its
    /// own type, member by member and element by element: into the region (`into`) or out of it.
    fn copy_region(&mut self, region: &str, own: &Place, region_ty: Ty, into: bool) {
        let info = self.info(region_ty).clone();
        match info.kind {
            // A nested overlap is its own region type: its bytes.
            Kind::Overlap { .. } if own.ty == region_ty => {
                let size = info.size;
                let own_bytes = bytes(own, size);
                let region_bytes = format!("{region}.bytes(0, {size})");
                let (target, source) = if into {
                    (region_bytes, own_bytes)
                } else {
                    (own_bytes, region_bytes)
                };
                self.line(&format!("ov::copy({target}, {source});"));
            }
            Kind::Overlap { members, bits } => {
                let own_members = self.info(own.ty).members().to_vec();
                for ((member, bit), own_member) in members.iter().zip(&bits).zip(&own_members) {
                    let own_place = Place {
                        expr: format!("{}.{}", own.expr, own_member.field()),
                        ty: own_member.ty,
                        is_ref: false,
                    };
                    match self.view(region, member.ty, member.offset as usize, *bit) {
                        Some(view) => self.copy_leaf(&view, &own_place, into),
                        None => self.copy_region(
                            &format!("ov::Region::of({region}.sub({}))", member.offset),
                            &own_place,
                            member.ty,
                            into,
                        ),
                    }
                }
            }
            Kind::Array { elem, dims } => {
                let Kind::Array { elem: own_elem, .. } = self.info(own.ty).kind.clone() else {
                    return;
                };
                let stride = self.info(elem).size;
                let mut own_expr = own.expr.clone();
                let mut flat = String::from("0");
                for (lower, upper) in &dims {
                    let index = self.temp();
                    let count = length(*lower, *upper);
                    self.line(&format!("for {index} in 0..{count}usize {{"));
                    self.indent += 1;
                    own_expr = format!("{own_expr}[{index}]");
                    flat = format!("({flat}) * {count} + {index}");
                }
                let element_region = format!("ov::Region::of({region}.sub(({flat}) * {stride}))");
                let own_place = Place {
                    expr: own_expr,
                    ty: own_elem,
                    is_ref: false,
                };
                match self.view(&element_region, elem, 0, None) {
                    Some(view) => self.copy_leaf(&view, &own_place, into),
                    None => self.copy_region(&element_region, &own_place, elem, into),
                }
                for _ in &dims {
                    self.indent -= 1;
                    self.line("}");
                }
            }
            _ => {}
        }
    }

    /// The view of the elementary value of type `ty` `offset` bytes into `region`, at bit `bit` for
    /// a `BOOL` member of an overlap; `None` for a structure or an array, which is a region of its
    /// own. A `BOOL` without a bit takes its whole byte, 0 or 1, as a stored `BOOL` does.
    fn view(&self, region: &str, ty: Ty, offset: usize, bit: Option<u8>) -> Option<String> {
        if let Some(bit) = bit {
            return Some(format!("ov::Bit::in_region({region}, {offset}, {bit})"));
        }
        let storage = match self.info(ty).scalar()? {
            Elem::Bool => "u8",
            elem => elem.rust(),
        };
        Some(format!(
            "ov::At::<{storage}>::in_region({region}, {offset})"
        ))
    }

    /// Copies one elementary value between the view `view` and `own`: into the view (`into`) or
    /// out of it.
    fn copy_leaf(&mut self, view: &str, own: &Place, into: bool) {
        if into {
            self.line(&format!("{view}.set({}.get());", own.expr));
        } else {
            self.line(&format!("{}.set({view}.get());", own.expr));
        }
    }
}

/// The bytes of `place`, a value of an overlap type of `size` bytes: a region's, or the backing of
/// a variable of that type.
fn bytes(place: &Place, size: u32) -> String {
    if place.is_overlap_region() {
        format!("{}.bytes(0, {size})", place.expr)
    } else {
        format!("&({}).B[..]", place.expr)
    }
}

/// The elements of a dimension from `lower` to `upper`.
fn length(lower: i64, upper: i64) -> usize {
    usize::try_from(upper.saturating_sub(lower).saturating_add(1)).unwrap_or(0)
}
