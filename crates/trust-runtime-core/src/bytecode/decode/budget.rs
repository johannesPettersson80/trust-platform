use super::*;

/// Cumulative requested decoder payload storage and work, excluding allocator overhead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecodeStats {
    /// Cumulative requested vector/map payload and owned string bytes.
    pub allocation_bytes: usize,
    /// Charged input bytes, records, payload copies and sort work.
    pub work: usize,
}
pub(super) struct DecodeBudget {
    bytes: usize,
    work: usize,
    stats: DecodeStats,
}
impl DecodeBudget {
    pub(super) fn new(bytes: usize, work: usize) -> Self {
        Self {
            bytes,
            work,
            stats: DecodeStats::default(),
        }
    }
    pub(super) fn charge(&mut self, bytes: usize, work: usize) -> Result<(), BytecodeError> {
        let allocation_bytes = self
            .stats
            .allocation_bytes
            .checked_add(bytes)
            .ok_or(BytecodeError::DecodeMemoryLimit)?;
        let work = self
            .stats
            .work
            .checked_add(work)
            .ok_or(BytecodeError::DecodeWorkLimit)?;
        if allocation_bytes > self.bytes {
            return Err(BytecodeError::DecodeMemoryLimit);
        }
        if work > self.work {
            return Err(BytecodeError::DecodeWorkLimit);
        }
        self.stats = DecodeStats {
            allocation_bytes,
            work,
        };
        Ok(())
    }
    pub(super) fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, BytecodeError> {
        self.charge(
            count
                .checked_mul(core::mem::size_of::<T>())
                .ok_or(BytecodeError::DecodeMemoryLimit)?,
            count,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| BytecodeError::DecodeMemoryLimit)?;
        Ok(values)
    }
    pub(super) fn copy<T: Clone>(&mut self, source: &[T]) -> Result<Vec<T>, BytecodeError> {
        let mut values = self.vector(source.len())?;
        values.extend_from_slice(source);
        Ok(values)
    }
    pub(super) fn stats(&self) -> DecodeStats {
        self.stats
    }
}
