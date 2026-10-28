//! Bounded little-endian reads of the retain codec's byte stream (specification 11 §6.7).
//!
//! Every read is checked against the remaining bytes, so a truncated payload is refused instead
//! of a partly decoded snapshot.

use alloc::{string::String, vec::Vec};

use crate::error::RuntimeError;

/// Write a length-prefixed UTF-8 string: its `u32` little-endian byte length then its bytes.
pub fn encode_string(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// A cursor over retain bytes that never reads past the end.
pub(crate) struct RetainReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> RetainReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub(crate) fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], RuntimeError> {
        let end = self.offset.saturating_add(len);
        if end > self.data.len() {
            return Err(RuntimeError::RetainCorruption(
                "retain data truncated".into(),
            ));
        }
        let slice = &self.data[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.data.len()
    }

    pub(crate) fn expect_finished(&self, context: &str) -> Result<(), RuntimeError> {
        if self.is_finished() {
            Ok(())
        } else {
            Err(RuntimeError::RetainCorruption(
                alloc::format!("{context} has trailing retain data").into(),
            ))
        }
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, RuntimeError> {
        Ok(self.read_bytes(1)?[0])
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16, RuntimeError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, RuntimeError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64, RuntimeError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub(crate) fn read_i8(&mut self) -> Result<i8, RuntimeError> {
        Ok(self.read_u8()? as i8)
    }

    pub(crate) fn read_i16(&mut self) -> Result<i16, RuntimeError> {
        Ok(self.read_u16()? as i16)
    }

    pub(crate) fn read_i32(&mut self) -> Result<i32, RuntimeError> {
        Ok(self.read_u32()? as i32)
    }

    pub(crate) fn read_i64(&mut self) -> Result<i64, RuntimeError> {
        Ok(self.read_u64()? as i64)
    }

    pub(crate) fn read_f32(&mut self) -> Result<f32, RuntimeError> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn read_f64(&mut self) -> Result<f64, RuntimeError> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub(crate) fn read_string(&mut self) -> Result<String, RuntimeError> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| RuntimeError::RetainCorruption("invalid utf-8 in retain".into()))
    }
}
