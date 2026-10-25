//! The type a value takes in a `STRUCT OVERLAP`'s backing (IEC 61131-3 §6.4.4.7.1; specification 02
//! §3.6): the bytes of its own layout, which the members and elements of a structure or an array
//! that lies there read and write (`lower::overlap`).

use smol_str::SmolStr;

use super::{Elem, Kind, Member, Ty, TypeInfo, Types};
use crate::native::NativeError;

impl Types<'_> {
    /// The type a value of `ty` is in a `STRUCT OVERLAP`'s backing, the bytes of its own layout
    /// (IEC §6.4.4.7.1): an elementary value, an overlap and an array of elementary values are
    /// themselves; a structure is an overlap of its members at their relative addresses, or at
    /// their offsets when it has none, a `BOOL` at bit 0 of its byte; an array of structures an
    /// array of those. Its members and elements are views of the backing's bytes, which a
    /// whole structure is copied into and out of member by member (`lower::overlap`).
    ///
    /// # Errors
    ///
    /// [`NativeError::Unsupported`] for a string, an instance, a reference or an interface.
    pub(super) fn region(&mut self, ty: Ty) -> Result<Ty, NativeError> {
        if let Some(region) = self.regions.get(&ty) {
            return Ok(*region);
        }
        let info = self.info(ty).clone();
        let region = match &info.kind {
            Kind::Elem(_) | Kind::Enum { .. } | Kind::Subrange { .. } | Kind::Overlap { .. } => {
                return Ok(ty);
            }
            Kind::Struct { members } => {
                let relative = self.relative.get(&ty).cloned();
                let mut laid = Vec::with_capacity(members.len());
                let mut bits = Vec::with_capacity(members.len());
                let mut size = 0u32;
                for (index, member) in members.iter().enumerate() {
                    let at = relative
                        .as_ref()
                        .and_then(|bits| bits.get(index).copied().flatten())
                        .unwrap_or(u64::from(member.offset) * 8);
                    let unplaced = || {
                        NativeError::unsupported(format!(
                            "the member {} in an overlap",
                            member.name
                        ))
                    };
                    let byte = u32::try_from(at / 8).map_err(|_| unplaced())?;
                    #[allow(clippy::cast_possible_truncation)]
                    let bit = (at % 8) as u8;
                    let member_ty = self.region(member.ty)?;
                    let (bit, bytes) = if self.info(member_ty).scalar() == Some(Elem::Bool) {
                        self.bit(bit);
                        (Some(bit), 1)
                    } else if bit == 0 {
                        (None, self.info(member_ty).size)
                    } else {
                        return Err(unplaced());
                    };
                    size = size.max(byte.checked_add(bytes).ok_or_else(unplaced)?);
                    laid.push(Member {
                        name: member.name.clone(),
                        ty: member_ty,
                        offset: byte,
                        flags: member.flags,
                        via: SmolStr::default(),
                    });
                    bits.push(bit);
                }
                self.intern(TypeInfo {
                    kind: Kind::Overlap {
                        members: laid,
                        bits,
                    },
                    size,
                    align: 1,
                    name: info.name.clone(),
                    rust: format!("{}__REGION", info.rust),
                })
            }
            Kind::Array { elem, dims } => {
                let elem_region = self.region(*elem)?;
                if elem_region == *elem {
                    return Ok(ty);
                }
                let element = self.info(elem_region).clone();
                let mut count = 1u32;
                let mut rust = element.rust.clone();
                for (lower, upper) in dims.iter().rev() {
                    let length = u32::try_from(upper - lower + 1).unwrap_or(u32::MAX);
                    count = count.saturating_mul(length);
                    rust = format!("[{rust}; {length}]");
                }
                self.intern(TypeInfo {
                    kind: Kind::Array {
                        elem: elem_region,
                        dims: dims.clone(),
                    },
                    size: element.size.saturating_mul(count),
                    align: 1,
                    name: SmolStr::default(),
                    rust,
                })
            }
            _ => {
                return Err(NativeError::unsupported(format!(
                    "the type {} in an overlap",
                    info.name
                )));
            }
        };
        self.regions.insert(ty, region);
        self.naturals.insert(region, ty);
        Ok(region)
    }

    /// The type that the region type `region` is outside a backing, when it is another
    /// ([`Self::region`]).
    pub(in crate::native) fn natural(&self, region: Ty) -> Option<Ty> {
        self.naturals.get(&region).copied()
    }
}
