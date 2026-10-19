//! A write of one scalar leaf of a backing, made where the backing lies (specification 34 MEM-12,
//! ARCH-05; C1-15b).
//!
//! The flat store holds a structure with a physical layout as its backing alone, so a write into one
//! of its scalar leaves is checked as [`StructValue::prepare_physical_write`] checks it (the written
//! value against the leaf, then each leaf that shares a bit with it, in bit order) and staged as the
//! bits it writes, with no value of the structure and no allocation. A leaf is at most 64 bits wide,
//! so the leaves that share its bits lie within 64 bits on either side of it: the staged write keeps
//! that window of the backing as it will read once the write is made, which is all its checks read.

use super::*;

/// The bytes of the window around a written leaf: 64 bits on either side of a leaf of at most 64
/// bits, from a byte boundary.
const WINDOW_BYTES: usize = 26;

/// One staged write of a scalar leaf of a backing: the bits it writes, and the backing around the
/// leaf as it will read once they are written.
#[derive(Clone)]
pub(crate) struct LeafWrite {
    layout: Arc<PhysicalLayout>,
    leaf: u32,
    bits: u64,
    /// The backing byte the window starts at.
    window_start: u32,
    window: [u8; WINDOW_BYTES],
}

impl PhysicalLayout {
    /// Whether scalar leaf `leaf` is an enumeration.
    pub(crate) fn leaf_is_enumeration(&self, leaf: u32) -> bool {
        self.leaves
            .get(leaf as usize)
            .is_some_and(|leaf| leaf.enumeration.is_some())
    }
}

/// Stage the write of `value` into scalar leaf `leaf` of the backing `bytes` of a structure whose
/// physical layout is `layout`, checked as a write of the leaf is checked: `value` of the leaf's
/// kind (an enumeration of the type named `enum_type`), within its subrange and a variant of its
/// enumeration, then each leaf that shares a bit with it, as it will read, in bit order. The first
/// refusal is the one the value store gives.
pub(crate) fn stage_leaf_write(
    layout: &Arc<PhysicalLayout>,
    leaf: u32,
    value: &Value,
    enum_type: &str,
    bytes: &[u8],
) -> Result<LeafWrite, RuntimeError> {
    let written = layout
        .leaves
        .get(leaf as usize)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let bits = scalar::checked_leaf_bits(value, written, layout.abi, enum_type)?;
    let window_start = written.offset.saturating_sub(64) / 8;
    let window_end = written
        .offset
        .checked_add(u32::from(written.width) + 64)
        .ok_or(RuntimeError::Overflow)?
        .div_ceil(8);
    let source = bytes
        .get(window_start as usize..(window_end as usize).min(bytes.len()))
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let mut window = [0u8; WINDOW_BYTES];
    window
        .get_mut(..source.len())
        .ok_or(RuntimeError::InvalidExecutionState)?
        .copy_from_slice(source);
    write_bits(
        &mut window,
        written.offset - window_start * 8,
        written.width,
        bits,
    );
    let staged = LeafWrite {
        layout: Arc::clone(layout),
        leaf,
        bits,
        window_start,
        window,
    };
    staged.aliases(&mut |alias, alias_bits| scalar::validate_leaf(alias_bits, alias))?;
    Ok(staged)
}

impl LeafWrite {
    /// Visit each leaf that shares a bit with the written one (itself included), in bit order, with
    /// its bits once the write is made.
    fn aliases(
        &self,
        visit: &mut dyn FnMut(&Leaf, u64) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let layout = &self.layout;
        let written = layout
            .leaves
            .get(self.leaf as usize)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let range = written.offset..written.offset + u32::from(written.width);
        for &index in &layout.bit_order[layout.affected_leaves(&range)] {
            let alias = &layout.leaves[index];
            if alias.offset + u32::from(alias.width) <= range.start {
                continue;
            }
            let start = alias
                .offset
                .checked_sub(self.window_start * 8)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            visit(alias, read_bits(&self.window, start, alias.width))?;
        }
        Ok(())
    }

    /// Run `check` over the value of each leaf that shares a bit with the written one, as
    /// [`PreparedPhysicalWrite::validate_aliases`] runs it over the leaves a write decodes again.
    pub(crate) fn validate_aliases(
        &self,
        check: &mut impl FnMut(&Value) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        self.aliases(&mut |alias, bits| match alias.enumeration {
            // An enumeration's bits are a variant of it, checked when the write was staged.
            Some(_) => Ok(()),
            None => {
                let template =
                    scalar::zero(alias.primitive_id).ok_or(RuntimeError::TypeMismatch)?;
                let value =
                    scalar::value(&template, bits, alias).ok_or(RuntimeError::TypeMismatch)?;
                check(&value)
            }
        })
    }

    /// Write the staged bits into `bytes`, the backing the write was staged on.
    pub(crate) fn commit(&self, bytes: &mut [u8]) -> bool {
        let Some(written) = self.layout.leaves.get(self.leaf as usize) else {
            return false;
        };
        let end = written.offset + u32::from(written.width);
        if end.div_ceil(8) as usize > bytes.len() {
            return false;
        }
        write_bits(bytes, written.offset, written.width, self.bits);
        true
    }
}

impl PhysicalLayout {
    /// The bits of the backing that the structure nested at `path` holds, or `None` when no
    /// nested structure lies at `path`. A nested structure starts and ends on a byte.
    pub(crate) fn node_bits(&self, path: &[RefSegment]) -> Option<core::ops::Range<u32>> {
        self.child_at_path(path)
            .map(|child| child.offset..child.offset + child.bits)
    }
}

/// Check the copy of `source`, the backing of a structure of the type of the structure nested at
/// `path`, into that nested structure of a backing of `layout` that holds `bytes`, as
/// [`StructValue::prepare_physical_write`] checks the write of the structure's value there: each
/// leaf below `path` with the bits it takes, in path order, then each leaf that shares a bit with
/// the nested structure, in bit order, with the bits it will hold (C1-15b).
pub(crate) fn check_node_write(
    layout: &PhysicalLayout,
    path: &[RefSegment],
    source: &[u8],
    bytes: &[u8],
) -> Result<(), RuntimeError> {
    let selection = layout.selection(path).ok_or(RuntimeError::TypeMismatch)?;
    let bits = selection.bits.clone();
    let leaf_bits = |leaf: &Leaf| {
        (0..u32::from(leaf.width)).fold(0u64, |value, bit| {
            let position = leaf.offset + bit;
            let read = if bits.contains(&position) {
                read_bits(source, position - bits.start, 1)
            } else {
                read_bits(bytes, position, 1)
            };
            value | (read << bit)
        })
    };
    for &index in &layout.path_order[selection.leaves.clone()] {
        let leaf = &layout.leaves[index];
        scalar::validate_leaf(leaf_bits(leaf), leaf)?;
    }
    for &index in &layout.bit_order[layout.affected_leaves(&bits)] {
        let leaf = &layout.leaves[index];
        if leaf.offset + u32::from(leaf.width) <= bits.start {
            continue;
        }
        scalar::validate_leaf(leaf_bits(leaf), leaf)?;
    }
    Ok(())
}
