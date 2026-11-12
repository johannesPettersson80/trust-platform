//! A string's bytes through a pointer under the CODESYS profile (docs/IEC_DEVIATIONS.md DEV-039;
//! specification 44 §7.3). `ADR(s)` where a pointer to a byte or to a one-dimensional array of bytes
//! is wanted names the string's bytes by a reference of kind `TEXT_BYTES`, which names the whole
//! string. Every write through a pointer of such a type then sets the length that the written bytes
//! give the text, its first 0 byte, as CODESYS reads a string (`Areas::bytes_written`); a pointer to
//! a variable of its own sets nothing.

use trust_syntax::syntax::SyntaxNode;

use super::{Lowerer, Place};
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// Whether a pointer to `ty` may name a string's bytes: a `BYTE`, a `CHAR` or a one-dimensional
    /// array of them, the types of DEV-039's bindings.
    pub(super) fn names_text_bytes(&self, ty: Ty) -> bool {
        if self.is_byte(ty) {
            return true;
        }
        matches!(&self.info(ty).kind, Kind::Array { elem, dims } if dims.len() == 1 && self.is_byte(*elem))
    }

    /// The reference to the bytes of `place`, a `STRING`, where a pointer to `target`, a byte or a
    /// one-dimensional array of bytes, is wanted: the whole string, named from its first byte
    /// (`Areas::refer_text_bytes`). `None` for any other pair. An array names the string's bytes as
    /// far as they go: an element past them is refused at its index (`lib::element_reference`).
    pub(super) fn string_bytes(&mut self, place: &Place, target: Ty) -> Option<String> {
        let Kind::Str(capacity) = self.info(place.ty).kind else {
            return None;
        };
        if capacity == 0 || !self.names_text_bytes(target) {
            return None;
        }
        Some(self.bind(&format!(
            "w.areas.refer_text_bytes(cx, ({}).view(), 0)?",
            place.reference()
        )))
    }

    /// Whether `s[i]` of a value of `ty` is the `BYTE` at offset `i`, counted from 0: `ty` is a
    /// `STRING`, and the build follows the CODESYS profile (docs/IEC_DEVIATIONS.md DEV-046).
    pub(super) fn bytes_by_offset(&self, ty: Ty) -> bool {
        matches!(self.info(ty).kind, Kind::Str(_))
            && crate::harness::dialect_of(self.model.conversions).is_codesys()
    }

    /// `text[index]` of a `STRING` under the CODESYS profile as a place: the `BYTE` at offset
    /// `index`, counted from 0, named by a reference to the string's bytes from it
    /// (`Areas::refer_text_bytes`), so that a write into it, `ADR` of it and an in-out given it set
    /// the string's length as a write through a pointer does (DEV-046, DEV-039).
    pub(super) fn text_byte_place(
        &mut self,
        node: &SyntaxNode,
        text: &Place,
        index: &SyntaxNode,
    ) -> Result<Place, NativeError> {
        if text.is_overlap_member() || text.is_overlap_region() {
            return Err(self.unsupported(
                node,
                "a byte of a string in a STRUCT OVERLAP, which no reference can name apart from \
                 the members that share its bytes",
            ));
        }
        let lint = self.model.types.elem(Elem::LInt);
        let index = self.scalar_to(index, lint)?;
        let reference = self.bind(&format!(
            "w.areas.refer_text_bytes(cx, ({}).view(), {index})?",
            text.reference()
        ));
        let byte = self.model.types.elem(Elem::Byte);
        Ok(self.dereference_bytes(&reference, byte))
    }

    /// The variable of type `target`, a byte or an array of bytes, that the reference value
    /// `reference` names, which may be bytes of a string: the reference is read once and kept for
    /// the writes into the variable (`after_write`).
    pub(super) fn dereference_bytes(&mut self, reference: &str, target: Ty) -> Place {
        let reference = self.bind(reference);
        let storage = self.info(target).rust.clone();
        let expr = self.bind(&format!("w.areas.deref::<{storage}>(cx, &{reference})?"));
        self.text_bytes.push((expr.clone(), reference));
        Place {
            expr,
            ty: target,
            is_ref: true,
        }
    }

    /// The reference that gave `place`, when it is the dereference of a pointer that may name a
    /// string's bytes.
    pub(super) fn bytes_reference(&self, place: &Place) -> Option<String> {
        self.text_bytes
            .iter()
            .find(|(expr, _)| *expr == place.expr)
            .map(|(_, reference)| reference.clone())
    }

    /// After a write into `place`: when a pointer that may name a string's bytes gave the place, the
    /// string's length follows the bytes written (`Areas::bytes_written`), which sets nothing when
    /// the pointer names a variable of its own.
    pub(super) fn after_write(&mut self, place: &Place) {
        if let Some(reference) = self.bytes_reference(place) {
            let size = self.info(place.ty).size;
            self.line(&format!(
                "w.areas.bytes_written(cx, &{reference}, {size})?;"
            ));
        }
    }

    /// `fresh`, the reference that `REF`, `ADR` or a function block's in-out takes to `place`; when
    /// a pointer that may name a string's bytes gave the place, the pointer's reference if it does,
    /// so that writes through the new reference set the string's length too.
    pub(super) fn keep_text_bytes(&self, place: &Place, fresh: String) -> String {
        match self.bytes_reference(place) {
            Some(reference) => format!("text_bytes_or({reference}, {fresh})"),
            None => fresh,
        }
    }
}
