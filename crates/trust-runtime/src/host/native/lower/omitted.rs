//! The inputs that a formal call of a standard function omits (IEC 61131-3 Ed.3 §6.6.1.4.2;
//! specification 07 §1): "Any variable to which no value is assigned in the list shall have the
//! initial value, if any, assigned in the declaration of the called entity, or the default value
//! for the associated data type". A standard function declares no initial value, so an omitted
//! input is the default value of the type its call gives it, as a variable of that type starts
//! (specification 02): the zero of an elementary type, the empty text of a string. An input is
//! `None` when the call omits it.

use trust_syntax::syntax::SyntaxNode;

use super::text::Text;
use super::Lowerer;
use crate::native::types::{Kind, Ty};
use crate::native::NativeError;

impl Lowerer<'_, '_> {
    /// The common type of the inputs among `operands` that the call gives, as `common_of` finds
    /// it: the type an omitted input takes.
    pub(super) fn common_of_given(
        &mut self,
        node: &SyntaxNode,
        operands: &[Option<SyntaxNode>],
    ) -> Result<Ty, NativeError> {
        let given: Vec<SyntaxNode> = operands.iter().flatten().cloned().collect();
        self.common_of(node, &given)
    }

    /// The default value of type `ty`, an input of that type that the call omits: the zero of an
    /// elementary type. An enumerated or a subrange type declares its own, which is not compiled
    /// here.
    pub(super) fn default_scalar(&self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        match &self.info(ty).kind {
            Kind::Elem(elem) => Ok(format!("<{}>::default()", elem.rust())),
            _ => Err(self.unsupported(
                node,
                format!(
                    "an omitted input of the type {}, whose default value it declares",
                    self.info(ty).name
                ),
            )),
        }
    }

    /// Each of `operands` as a scalar of type `ty`, in order, each in a temporary: an omitted one
    /// is the default value of `ty`.
    pub(super) fn operand_scalars(
        &mut self,
        node: &SyntaxNode,
        operands: &[Option<SyntaxNode>],
        ty: Ty,
    ) -> Result<Vec<String>, NativeError> {
        let mut scalars = Vec::with_capacity(operands.len());
        for operand in operands {
            let scalar = match operand {
                Some(value) => self.scalar_to(value, ty)?,
                None => self.default_scalar(node, ty)?,
            };
            scalars.push(self.bind(&scalar));
        }
        Ok(scalars)
    }

    /// Each of `operands` as a reference to a string and its capacity, in order: an omitted one is
    /// an empty text of the family, `WSTRING` when `wide`.
    pub(super) fn operand_texts(
        &mut self,
        operands: &[Option<SyntaxNode>],
        wide: bool,
    ) -> Result<Vec<(String, u32)>, NativeError> {
        let mut texts = Vec::with_capacity(operands.len());
        for operand in operands {
            texts.push(match operand {
                Some(value) => self.text_ref(value)?,
                None => {
                    let empty = if wide {
                        Text::Units(Vec::new())
                    } else {
                        Text::Bytes(Vec::new())
                    };
                    (self.text_temp(&[empty], 0), 0)
                }
            });
        }
        Ok(texts)
    }
}
