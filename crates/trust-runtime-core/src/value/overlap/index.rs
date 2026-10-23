//! Immutable path and interval indexes; a write never rescans every source for every alias.
use super::*;
use core::ops::Range;

pub(super) struct Selection {
    pub(super) leaves: Range<usize>,
    pub(super) children: Range<usize>,
    pub(super) bits: Range<u32>,
}

impl PhysicalLayout {
    pub(super) fn new(
        bits: u32,
        abi: crate::value::StorageAbi,
        leaves: Vec<Leaf>,
        children: Vec<Child>,
        charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let max_path_work = leaves
            .iter()
            .map(|leaf| path_work(&leaf.path))
            .chain(children.iter().map(|child| path_work(&child.path)))
            .max()
            .unwrap_or(0);
        let comparison_work = max_path_work
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?
            .max(1);
        let mut path_order = index_order(leaves.len(), charge)?;
        let mut bit_order = index_order(leaves.len(), charge)?;
        let mut child_path_order = index_order(children.len(), charge)?;
        let mut child_bit_order = index_order(children.len(), charge)?;
        sort_order(&mut path_order, charge, comparison_work, &mut |a, b| {
            compare_paths(&leaves[a].path, &leaves[b].path).is_lt()
        })?;
        sort_order(&mut bit_order, charge, 1, &mut |a, b| {
            (leaves[a].offset, a) < (leaves[b].offset, b)
        })?;
        sort_order(
            &mut child_path_order,
            charge,
            comparison_work,
            &mut |a, b| compare_paths(&children[a].path, &children[b].path).is_lt(),
        )?;
        sort_order(&mut child_bit_order, charge, 1, &mut |a, b| {
            (children[a].offset, a) < (children[b].offset, b)
        })?;
        for pair in path_order.windows(2) {
            charge(0, comparison_work)?;
            if compare_paths(&leaves[pair[0]].path, &leaves[pair[1]].path).is_eq() {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        for pair in child_path_order.windows(2) {
            charge(0, comparison_work)?;
            if compare_paths(&children[pair[0]].path, &children[pair[1]].path).is_eq() {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        let bit_max_end = ends(
            &bit_order,
            |i| leaves[i].offset.checked_add(u32::from(leaves[i].width)),
            charge,
        )?;
        let child_max_end = subtree_ends(&child_bit_order, &children, charge)?;
        Ok(Self {
            bits,
            abi,
            leaves,
            children,
            path_order,
            bit_order,
            bit_max_end,
            child_path_order,
            child_bit_order,
            child_max_end,
            max_path_work,
        })
    }

    pub(super) fn selection(&self, path: &[RefSegment]) -> Option<Selection> {
        let leaves = path_range(&self.path_order, path, |i| &self.leaves[i].path);
        let children = path_range(&self.child_path_order, path, |i| &self.children[i].path);
        let mut start = u32::MAX;
        let mut end = 0;
        for &i in &self.path_order[leaves.clone()] {
            let leaf = &self.leaves[i];
            start = start.min(leaf.offset);
            end = end.max(leaf.offset + u32::from(leaf.width));
        }
        for &i in &self.child_path_order[children.clone()] {
            let child = &self.children[i];
            start = start.min(child.offset);
            end = end.max(child.offset + child.bits);
        }
        if path.is_empty() {
            start = 0;
            end = self.bits;
        }
        (start < end).then_some(Selection {
            leaves,
            children,
            bits: start..end,
        })
    }

    pub(crate) fn canonical_path<'a>(&'a self, path: &[RefSegment]) -> Option<&'a [RefSegment]> {
        let range = path_range(&self.path_order, path, |i| &self.leaves[i].path);
        if let Some(&i) = self
            .path_order
            .get(range.start)
            .filter(|_| !range.is_empty())
        {
            return Some(&self.leaves[i].path[..path.len()]);
        }
        let range = path_range(&self.child_path_order, path, |i| &self.children[i].path);
        self.child_path_order
            .get(range.start)
            .filter(|_| !range.is_empty())
            .map(|&i| &self.children[i].path[..path.len()])
    }

    pub(crate) fn storage_compatible(&self, other: &Self) -> bool {
        let abi_matters = self
            .leaves
            .iter()
            .any(|leaf| matches!(leaf.primitive_id, 16 | 18 | 20 | 22));
        (!abi_matters || self.abi == other.abi)
            && self.bits == other.bits
            && self.leaves.len() == other.leaves.len()
            && self.children.len() == other.children.len()
            && self.leaves.iter().zip(&other.leaves).all(|(a, b)| {
                a.offset == b.offset
                    && a.width == b.width
                    && a.primitive_id == b.primitive_id
                    && a.enumeration == b.enumeration
                    && a.range == b.range
                    && compare_paths(&a.path, &b.path).is_eq()
            })
            && self.children.iter().zip(&other.children).all(|(a, b)| {
                a.offset == b.offset && a.bits == b.bits && compare_paths(&a.path, &b.path).is_eq()
            })
    }

    pub(super) fn leaf_at_path(&self, path: &[RefSegment]) -> Option<&Leaf> {
        let range = path_range(&self.path_order, path, |i| &self.leaves[i].path);
        let index = *self.path_order.get(range.start)?;
        compare_paths(&self.leaves[index].path, path)
            .is_eq()
            .then_some(&self.leaves[index])
    }

    pub(super) fn child_at_path(&self, path: &[RefSegment]) -> Option<&Child> {
        let range = path_range(&self.child_path_order, path, |i| &self.children[i].path);
        let index = *self.child_path_order.get(range.start)?;
        compare_paths(&self.children[index].path, path)
            .is_eq()
            .then_some(&self.children[index])
    }

    pub(super) fn leaves_below_path(&self, path: &[RefSegment]) -> Range<usize> {
        descendants(&self.path_order, path, |i| &self.leaves[i].path)
    }

    pub(super) fn children_below_path(&self, path: &[RefSegment]) -> Range<usize> {
        descendants(&self.child_path_order, path, |i| &self.children[i].path)
    }

    pub(super) fn affected_leaves(&self, bits: &Range<u32>) -> Range<usize> {
        let first = self.bit_max_end.partition_point(|&end| end <= bits.start);
        let last = self
            .bit_order
            .partition_point(|&i| self.leaves[i].offset < bits.end);
        first.min(last)..last
    }

    pub(super) fn affected_children<'a>(&'a self, bits: &Range<u32>) -> ChildIntervals<'a> {
        let mut stack = [(0, 0); usize::BITS as usize + 1];
        stack[0] = (0, self.child_bit_order.len());
        ChildIntervals {
            layout: self,
            bits: bits.clone(),
            stack,
            depth: 1,
        }
    }
}

fn descendants<'a>(
    order: &[usize],
    path: &[RefSegment],
    get: impl Fn(usize) -> &'a [RefSegment],
) -> Range<usize> {
    let mut range = path_range(order, path, &get);
    if let Some(&index) = order.get(range.start)
        && compare_paths(get(index), path).is_eq()
    {
        range.start += 1;
    }
    range
}

fn path_range<'a>(
    order: &[usize],
    path: &[RefSegment],
    get: impl Fn(usize) -> &'a [RefSegment],
) -> Range<usize> {
    let first = order.partition_point(|&i| compare_paths(get(i), path).is_lt());
    let length = order[first..].partition_point(|&i| path_suffix(get(i), path).is_some());
    first..first + length
}

fn ends(
    order: &[usize],
    get: impl Fn(usize) -> Option<u32>,
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<u32>, RuntimeError> {
    charge(
        order
            .len()
            .checked_mul(core::mem::size_of::<u32>())
            .ok_or(RuntimeError::Overflow)?,
        order.len(),
    )?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(order.len())
        .map_err(|_| RuntimeError::Overflow)?;
    let mut maximum = 0;
    for &i in order {
        maximum = maximum.max(get(i).ok_or(RuntimeError::Overflow)?);
        result.push(maximum);
    }
    Ok(result)
}

/// A balanced immutable interval index prunes by each subtree's own maximum,
/// rather than a prefix maximum dominated by a large enclosing structure.
pub(super) struct ChildIntervals<'a> {
    layout: &'a PhysicalLayout,
    bits: Range<u32>,
    stack: [(usize, usize); usize::BITS as usize + 1],
    depth: usize,
}
impl Iterator for ChildIntervals<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        while self.depth != 0 {
            self.depth -= 1;
            let (start, end) = self.stack[self.depth];
            if start == end {
                continue;
            }
            let middle = start + (end - start) / 2;
            if self.layout.children[self.layout.child_bit_order[start]].offset >= self.bits.end
                || self.layout.child_max_end[middle] <= self.bits.start
            {
                continue;
            }
            // Balanced subtree depth cannot exceed usize::BITS; the fixed stack
            // includes the two deferred siblings at each visited branch.
            if middle + 1 < end {
                self.stack[self.depth] = (middle + 1, end);
                self.depth += 1;
            }
            if start < middle {
                self.stack[self.depth] = (start, middle);
                self.depth += 1;
            }
            let index = self.layout.child_bit_order[middle];
            let child = &self.layout.children[index];
            if child.offset < self.bits.end && child.offset + child.bits > self.bits.start {
                return Some(index);
            }
        }
        None
    }
}

fn subtree_ends(
    order: &[usize],
    children: &[Child],
    charge: &mut dyn FnMut(usize, usize) -> Result<(), RuntimeError>,
) -> Result<Vec<u32>, RuntimeError> {
    charge(
        order
            .len()
            .checked_mul(core::mem::size_of::<u32>())
            .ok_or(RuntimeError::Overflow)?,
        order.len(),
    )?;
    let mut maxima = Vec::new();
    maxima
        .try_reserve_exact(order.len())
        .map_err(|_| RuntimeError::Overflow)?;
    maxima.resize(order.len(), 0);
    fn fill(
        order: &[usize],
        children: &[Child],
        maxima: &mut [u32],
        start: usize,
        end: usize,
    ) -> Result<u32, RuntimeError> {
        if start == end {
            return Ok(0);
        }
        let middle = start + (end - start) / 2;
        let child = &children[order[middle]];
        let own = child
            .offset
            .checked_add(child.bits)
            .ok_or(RuntimeError::Overflow)?;
        let left = fill(order, children, maxima, start, middle)?;
        let right = fill(order, children, maxima, middle + 1, end)?;
        let maximum = own.max(left).max(right);
        maxima[middle] = maximum;
        Ok(maximum)
    }
    fill(order, children, &mut maxima, 0, order.len())?;
    Ok(maxima)
}
