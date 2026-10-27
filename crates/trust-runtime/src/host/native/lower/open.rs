//! Arrays whose bounds a caller supplies, `ARRAY[*]` (IEC 61131-3 §6.5.3; specification 03 §6;
//! specification 10 §2.1; specification 44 §6.2, §7.3): a parameter of an open array is a view of
//! the caller's array (`abi::Open`), its elements and each dimension's bounds; a reference or a
//! pointer to one (`abi::OpenRef`) holds the bounds of the array it names beside the reference,
//! and indexing through either is checked against those bounds.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::types::{Elem, Kind, Ty};
use super::super::NativeError;
use super::{Lowerer, Place};

impl Lowerer<'_, '_> {
    /// The view of the array `node` names, for a parameter of the open array type `open`: an open
    /// array's own view, or a fixed array's elements and bounds.
    pub(super) fn open_view(&mut self, node: &SyntaxNode, open: Ty) -> Result<String, NativeError> {
        let place = self.place(node)?;
        self.view_of(node, &place, open)
    }

    /// The view of the array at `place` as the open array type `open`.
    fn view_of(
        &mut self,
        node: &SyntaxNode,
        place: &Place,
        open: Ty,
    ) -> Result<String, NativeError> {
        let Kind::Open { rank, .. } = self.info(open).kind else {
            return Err(self.unsupported(node, "a view of a value that is no open array"));
        };
        match self.info(place.ty).kind.clone() {
            Kind::Open { .. } => Ok(place.expr.clone()),
            Kind::Array { dims, .. } if dims.len() == rank => {
                let flatten = ".as_flattened()".repeat(rank - 1);
                Ok(format!(
                    "abi::Open::new(({}).as_slice(){flatten}, {})",
                    place.reference(),
                    bounds_literal(&dims)
                ))
            }
            _ => Err(self.unsupported(node, "an array of another rank for an open array")),
        }
    }

    /// The element of the open array at `view` that `indices` select, each index checked against
    /// its dimension's bounds.
    pub(super) fn open_element(
        &mut self,
        node: &SyntaxNode,
        view: &Place,
        indices: &[SyntaxNode],
    ) -> Result<Place, NativeError> {
        let Kind::Open { elem, rank } = self.info(view.ty).kind else {
            return Err(self.unsupported(node, "an index of a value that is no open array"));
        };
        if indices.len() != rank {
            return Err(self.unsupported(node, "an index list unlike the array's dimensions"));
        }
        let lint = self.model.types.elem(Elem::LInt);
        let mut values = Vec::with_capacity(rank);
        for index in indices {
            values.push(self.scalar_to(index, lint)?);
        }
        let expr = self.bind(&format!("{}.at(cx, [{}])?", view.expr, values.join(", ")));
        Ok(Place {
            expr,
            ty: elem,
            is_ref: true,
        })
    }

    /// The open array the reference at `reference` names, its type `target`, as a view.
    pub(super) fn open_target(&mut self, reference: &Place, target: Ty) -> Place {
        let (elem, rank) = match self.info(target).kind {
            Kind::Open { elem, rank } => (elem, rank),
            _ => (target, 1),
        };
        let storage = self.info(elem).rust.clone();
        let expr = self.bind(&format!(
            "w.areas.open::<{storage}, {rank}>(cx, {})?",
            reference.reference()
        ));
        Place {
            expr,
            ty: target,
            is_ref: false,
        }
    }

    /// `place := value` for a reference or a pointer to an open array: `NULL`, `REF` or `ADR` of
    /// an array or of an open array's view, or another reference to an open array, with the bounds
    /// of the array it names (specification 10 §2.1).
    pub(super) fn store_open(
        &mut self,
        place: &Place,
        value: &SyntaxNode,
        target: Ty,
    ) -> Result<(), NativeError> {
        let source = self.open_source(value, target)?;
        self.line(&format!("{}.copy_from({});", place.expr, source));
        Ok(())
    }

    /// A reference to an open array of type `target` for `value`, as an `abi::OpenRef` expression
    /// that the store copies.
    pub(super) fn open_source(
        &mut self,
        value: &SyntaxNode,
        target: Ty,
    ) -> Result<String, NativeError> {
        let Kind::Open { rank, .. } = self.info(target).kind else {
            return Err(self.unsupported(value, "a reference of an open array to another type"));
        };
        let temp = self.temp();
        self.line(&format!(
            "let {temp}: abi::OpenRef<{rank}> = <abi::OpenRef<{rank}> as Plain>::ZEROED;"
        ));
        if super::expr::is_null(value) {
            return Ok(format!("&{temp}"));
        }
        let taken = match value.kind() {
            SyntaxKind::ParenExpr => {
                return match value.children().next() {
                    Some(inner) => self.open_source(&inner, target),
                    None => Err(self.unsupported(value, "an empty parenthesis")),
                };
            }
            SyntaxKind::AddrExpr => Some((
                value
                    .children()
                    .find(|child| super::is_expression(child.kind())),
                "POINTER",
            )),
            SyntaxKind::CallExpr => super::expr::reference_call(value).map(|kind| {
                (
                    value
                        .children()
                        .find(|child| child.kind() == SyntaxKind::ArgList)
                        .and_then(|list| list.children().find(|arg| arg.kind() == SyntaxKind::Arg))
                        .and_then(|arg| arg.children().last()),
                    kind,
                )
            }),
            _ => None,
        };
        match taken {
            Some((Some(variable), kind)) => {
                let place = self.place(&variable)?;
                let view = self.view_of(&variable, &place, target)?;
                let view = self.bind(&view);
                self.line(&format!(
                    "{temp}.set(w.areas.refer_elements(cx, {view}.elements(), abi::RefValue::{kind})?, {view}.bounds());"
                ));
                Ok(format!("&{temp}"))
            }
            Some((None, _)) => Err(self.unsupported(value, "REF or ADR without its variable")),
            None => {
                // Another reference to an open array: its reference and its bounds.
                let place = self.place(value)?;
                if !matches!(self.info(place.ty).kind, Kind::Ref { target: held, .. } if self.same_open(held, target))
                {
                    return Err(
                        self.unsupported(value, "a value that is no reference to the open array")
                    );
                }
                Ok(place.reference())
            }
        }
    }

    /// Whether `a` and `b` are open arrays of one element type and rank.
    fn same_open(&self, a: Ty, b: Ty) -> bool {
        match (&self.info(a).kind, &self.info(b).kind) {
            (
                Kind::Open {
                    elem: a_elem,
                    rank: a_rank,
                },
                Kind::Open {
                    elem: b_elem,
                    rank: b_rank,
                },
            ) => a_elem == b_elem && a_rank == b_rank,
            _ => a == b,
        }
    }

    /// `LOWER_BOUND(array, dimension)` or `UPPER_BOUND`: the bound of a fixed array is a constant,
    /// an open array's its view's (IEC §6.5.3; specification 03 §6).
    pub(super) fn array_bound(
        &mut self,
        node: &SyntaxNode,
        upper: bool,
        args: &[SyntaxNode],
    ) -> Result<String, NativeError> {
        let [array, dimension] = args else {
            return Err(self.unsupported(node, "a bound without its array and dimension"));
        };
        let place = self.place(array)?;
        let lint = self.model.types.elem(Elem::LInt);
        match self.info(place.ty).kind.clone() {
            Kind::Array { dims, .. } => {
                let constant = self.constant_integer(dimension, lint).ok();
                let bounds = constant
                    .and_then(|dimension| usize::try_from(dimension).ok())
                    .and_then(|dimension| dimension.checked_sub(1))
                    .and_then(|index| dims.get(index));
                let Some((lower, high)) = bounds else {
                    return Err(self.unsupported(node, "a bound of a dimension the array lacks"));
                };
                Ok(format!("{}i64", if upper { high } else { lower }))
            }
            Kind::Open { .. } => {
                let dimension = self.scalar_to(dimension, lint)?;
                let function = if upper { "upper" } else { "lower" };
                Ok(self.bind(&format!("{}.{function}(cx, {dimension})?", place.expr)))
            }
            _ => Err(self.unsupported(node, "a bound of a value that is no array")),
        }
    }
}

/// The bounds `dims` as a Rust array of pairs.
fn bounds_literal(dims: &[(i64, i64)]) -> String {
    let pairs: Vec<String> = dims
        .iter()
        .map(|(lower, upper)| format!("({lower}i64, {upper}i64)"))
        .collect();
    format!("[{}]", pairs.join(", "))
}
