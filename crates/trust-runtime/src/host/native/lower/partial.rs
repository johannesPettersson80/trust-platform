//! Partial access of a bit string (IEC 61131-3 §6.5.2.2 and Table 13 note; specification 03 §6):
//! `.%Xn`, or `.n`, `.%Bn`, `.%Wn` and `.%Dn` read and write bit n, byte n, word n and double word
//! n of a `BYTE`, `WORD`, `DWORD` or `LWORD` variable, counted from its least significant end, as
//! trust-hir types them. A write changes only the selected bits of the variable.

use trust_runtime_core::value::{parse_partial_access, PartialAccess};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::types::Ty;
use super::super::NativeError;
use super::Lowerer;

impl Lowerer<'_, '_> {
    /// The base and the selected part of `node`, when `node` is a partial access `base.%Xn`.
    pub(super) fn partial_access(&self, node: &SyntaxNode) -> Option<(SyntaxNode, PartialAccess)> {
        if node.kind() != SyntaxKind::FieldExpr {
            return None;
        }
        let mut children = node.children();
        let (base, member) = (children.next()?, children.next()?);
        // An identifier never begins with `%` or a digit, so text that reads as a partial access
        // names no member.
        let access = parse_partial_access(member.text().to_string().trim())?;
        Some((base, access))
    }

    /// The value of the part `access` of `base`, as a value of `ty`, the part's type.
    pub(super) fn partial_read(
        &mut self,
        node: &SyntaxNode,
        base: &SyntaxNode,
        access: PartialAccess,
        ty: Ty,
    ) -> Result<String, NativeError> {
        let base_ty = self.type_of(base)?;
        let (shift, _) = self.partial_bits(node, base_ty, access)?;
        let value = self.scalar(base)?;
        let Some(part) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, "a partial access that is no scalar"));
        };
        Ok(match access {
            PartialAccess::Bit(_) => format!("(((({value}) >> {shift}) & 1) != 0)"),
            PartialAccess::Byte(_) | PartialAccess::Word(_) | PartialAccess::DWord(_) => {
                format!("((({value}) >> {shift}) as {})", part.rust())
            }
        })
    }

    /// `base.%Xn := value` and the other parts: the variable `base` names takes the part's new bits
    /// and keeps all others.
    pub(super) fn partial_write(
        &mut self,
        target: &SyntaxNode,
        base: &SyntaxNode,
        access: PartialAccess,
        value: &SyntaxNode,
    ) -> Result<(), NativeError> {
        let place = self.place(base)?;
        let (shift, width) = self.partial_bits(target, place.ty, access)?;
        let Some(base_elem) = self.info(place.ty).scalar() else {
            return Err(self.unsupported(target, "a partial access of a value that is no scalar"));
        };
        let rust = base_elem.rust();
        let part_ty = self.type_of(target)?;
        let part = self.scalar_to(value, part_ty)?;
        let mask: u64 = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let current = self.load(&place);
        let updated = format!(
            "((({current}) & !({mask}{rust} << {shift})) | ((({part}) as {rust}) << {shift}))"
        );
        self.store_scalar(&place, &updated);
        Ok(())
    }

    /// The shift and the width in bits of the part `access` of a value of `base_ty`, checked
    /// against the bits the base holds.
    fn partial_bits(
        &self,
        node: &SyntaxNode,
        base_ty: Ty,
        access: PartialAccess,
    ) -> Result<(u32, u32), NativeError> {
        let Some(base) = self.info(base_ty).scalar().filter(|elem| elem.is_bits()) else {
            return Err(self.unsupported(node, "a partial access of a value that is no bit string"));
        };
        let (index, width) = match access {
            PartialAccess::Bit(index) => (index, 1),
            PartialAccess::Byte(index) => (index, 8),
            PartialAccess::Word(index) => (index, 16),
            PartialAccess::DWord(index) => (index, 32),
        };
        let shift = u32::from(index) * width;
        if shift + width > base.size() * 8 || width >= base.size() * 8 {
            return Err(self.unsupported(node, "a partial access beyond its bit string"));
        }
        Ok((shift, width))
    }
}
