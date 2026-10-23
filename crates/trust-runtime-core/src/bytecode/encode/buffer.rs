use super::{BytecodeError, Vec};

pub(super) fn encoded_extent() -> BytecodeError {
    BytecodeError::InvalidHeader("encoded container exceeds fixed resource limit".into())
}

pub(super) fn encoded_count(value: usize) -> Result<u32, BytecodeError> {
    u32::try_from(value).map_err(|_| encoded_extent())
}

/// Capacity-checked byte writer shared by every section serializer.
pub(super) struct Buffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl Buffer {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.bytes.len()
    }
    pub(super) fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
    fn reserve_to(&mut self, target: usize) -> Result<(), BytecodeError> {
        if target > self.limit {
            return Err(encoded_extent());
        }
        if target > self.bytes.capacity() {
            let capacity = target
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| encoded_extent())?;
        }
        Ok(())
    }
    pub(super) fn extend_from_slice(&mut self, data: &[u8]) -> Result<(), BytecodeError> {
        let end = self
            .bytes
            .len()
            .checked_add(data.len())
            .ok_or_else(encoded_extent)?;
        self.reserve_to(end)?;
        self.bytes.extend_from_slice(data);
        Ok(())
    }
    pub(super) fn push(&mut self, value: u8) -> Result<(), BytecodeError> {
        self.extend_from_slice(&[value])
    }
    pub(super) fn pad_to(&mut self, target: usize) -> Result<(), BytecodeError> {
        if target > self.bytes.len() {
            self.reserve_to(target)?;
            self.bytes.resize(target, 0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_writer_preserves_bytes_when_a_write_exceeds_its_limit() {
        let mut writer = Buffer::new(4);
        writer.extend_from_slice(&[1, 2, 3, 4]).unwrap();
        assert!(writer.push(5).is_err());
        assert_eq!(writer.into_vec(), [1, 2, 3, 4]);
    }

    #[test]
    fn padding_rejects_an_unrepresentable_extent_before_allocating() {
        let mut writer = Buffer::new(4);
        writer.push(1).unwrap();
        assert!(writer.pad_to(usize::MAX).is_err());
        assert_eq!(writer.into_vec(), [1]);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn counts_do_not_truncate_to_the_wire_width() {
        assert_eq!(encoded_count(u32::MAX as usize).unwrap(), u32::MAX);
        assert!(encoded_count(u32::MAX as usize + 1).is_err());
    }
}
