//! The copy of a value that holds a function block or class instance: an instance, or a structure
//! or an array that holds one (specification 44 §6.2; docs/IEC_DECISIONS.md, "Copy of a function
//! block instance").

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::{Lowerer, Place};
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// `place := value` for a place that holds an instance: every variable of `place`, the internal
    /// ones and a standard block's hidden state included, takes the value of the same variable of
    /// `value`, a variable of the place's type. The place stays an instance of its own.
    pub(super) fn store_instance(
        &mut self,
        place: &Place,
        value: &SyntaxNode,
    ) -> Result<(), NativeError> {
        let source = match value.kind() {
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                // A reference where its instance is wanted is that instance (DEV-031).
                let source = self.place(value)?;
                self.referenced_place(source)
            }
            _ => {
                return Err(self.unsupported(
                    value,
                    "an assignment of a value that holds a function block or class instance, \
                     from no variable",
                ));
            }
        };
        if source.ty != place.ty {
            return Err(self.unsupported(
                value,
                "an assignment of a value that holds a function block or class instance, from a \
                 variable of another type",
            ));
        }
        self.copy_instance(place, &source);
        Ok(())
    }

    /// Copies `source` into `destination`, two places of one type that holds an instance, cell by
    /// cell (`Plain::copy_cells`): an assignment's or an output's copy.
    pub(super) fn copy_instance(&mut self, destination: &Place, source: &Place) {
        self.line(&format!(
            "Plain::copy_cells({}, {});",
            destination.reference(),
            source.reference()
        ));
    }
}
