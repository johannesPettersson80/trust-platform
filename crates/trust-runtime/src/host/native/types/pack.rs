//! A packed structure (`{attribute 'pack_mode'}`, specification 02 §3.5.2): the relative addresses
//! its pack mode gives its members, at which it lies in a `STRUCT OVERLAP`'s backing and on the wire,
//! as a structure's declared relative addresses do. Its storage keeps the natural layout
//! (specification 44 §7.2), which no program observes.

use trust_hir::types::packed_layout;

use super::{Kind, Ty, Types};
use crate::native::NativeError;

impl Types<'_> {
    /// Gives the structure `ty` the relative addresses at which the pack mode `mode` lays out its
    /// members, and its size and alignment by that mode.
    ///
    /// # Errors
    ///
    /// [`NativeError::Unsupported`] for a structure whose layout passes 4 GiB.
    pub(super) fn pack(&mut self, ty: Ty, mode: u8) -> Result<(), NativeError> {
        let members: Vec<(u64, u64)> = self
            .info(ty)
            .members()
            .iter()
            .map(|member| self.backing(member.ty))
            .collect();
        let too_large = || {
            NativeError::unsupported(format!(
                "the packed structure {} past 4 GiB",
                self.info(ty).name
            ))
        };
        let layout = packed_layout(&members, mode).ok_or_else(too_large)?;
        let size = u32::try_from(layout.size).map_err(|_| too_large())?;
        let align = u32::try_from(layout.align).map_err(|_| too_large())?;
        let mut bits = Vec::with_capacity(layout.offsets.len());
        for offset in layout.offsets {
            bits.push(Some(offset.checked_mul(8).ok_or_else(too_large)?));
        }
        self.relative.insert(ty, bits);
        self.packed.insert(ty, (size, align));
        Ok(())
    }

    /// The bytes and the alignment a value of `ty` takes in a packed structure: the bytes it takes
    /// in a backing ([`Types::region`]), aligned as an elementary value's size, an array's element,
    /// a packed structure by its pack mode and any other value as it is stored.
    fn backing(&self, ty: Ty) -> (u64, u64) {
        let info = self.info(ty);
        match &info.kind {
            Kind::Elem(_) | Kind::Enum { .. } | Kind::Subrange { .. } => {
                (u64::from(info.size), u64::from(info.size))
            }
            Kind::Array { elem, dims } => {
                let (size, align) = self.backing(*elem);
                let count = dims.iter().fold(1u64, |count, (lower, upper)| {
                    let length = upper
                        .checked_sub(*lower)
                        .and_then(|span| span.checked_add(1))
                        .and_then(|length| u64::try_from(length).ok())
                        .unwrap_or(0);
                    count.saturating_mul(length)
                });
                (size.saturating_mul(count), align)
            }
            Kind::Struct { members } => match self.packed.get(&ty) {
                Some((size, align)) => (u64::from(*size), u64::from(*align)),
                None => {
                    // As a backing holds it: each member at its relative address, or at its
                    // offset, to the end of the last.
                    let relative = self.relative.get(&ty);
                    let end = members
                        .iter()
                        .enumerate()
                        .map(|(index, member)| {
                            let at = relative
                                .and_then(|bits| bits.get(index).copied().flatten())
                                .map_or(u64::from(member.offset), |bit| bit / 8);
                            at.saturating_add(self.backing(member.ty).0)
                        })
                        .max()
                        .unwrap_or(0);
                    (end, u64::from(info.align))
                }
            },
            _ => (u64::from(info.size), u64::from(info.align)),
        }
    }
}
