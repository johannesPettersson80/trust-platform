//! Borrow one checked fixed wire record, then read its constant-offset fields.
//! The view borrows the input, so decoding never copies a complete row to the stack.
use super::*;

pub(super) struct Record<'a, const N: usize>(pub(super) &'a [u8; N]);

impl<'a, const N: usize> Record<'a, N> {
    pub(super) fn read(reader: &mut BytecodeReader<'a>) -> Result<Self, BytecodeError> {
        let bytes = reader.read_bytes(N)?;
        let record = bytes.try_into().map_err(|_| BytecodeError::UnexpectedEof)?;
        Ok(Self(record))
    }

    pub(super) fn word<const AT: usize>(&self) -> u32 {
        const { assert!(AT <= N && N - AT >= 4) };
        u32::from_le_bytes([self.0[AT], self.0[AT + 1], self.0[AT + 2], self.0[AT + 3]])
    }

    pub(super) fn half<const AT: usize>(&self) -> u16 {
        const { assert!(AT <= N && N - AT >= 2) };
        u16::from_le_bytes([self.0[AT], self.0[AT + 1]])
    }

    pub(super) fn optional<const AT: usize>(&self) -> Option<u32> {
        optional_u32(self.word::<AT>())
    }
}
