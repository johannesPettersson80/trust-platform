//! Portable bytecode metadata records.

mod format;

use alloc::vec::Vec;
use smol_str::SmolStr;
use thiserror::Error;

use crate::task::TaskConfig;

pub use format::*;

/// Bytecode format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytecodeVersion {
    /// Major version. Incompatible changes increment this field.
    pub major: u16,
    /// Minor version. Compatible section extensions increment this field.
    pub minor: u16,
}

impl BytecodeVersion {
    /// Construct a bytecode version pair.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// Supported major bytecode version.
pub const SUPPORTED_MAJOR_VERSION: u16 = 1;
/// Supported minor bytecode version.
pub const SUPPORTED_MINOR_VERSION: u16 = 1;

/// Bytecode decoder, encoder, and validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BytecodeError {
    /// The byte stream does not start with the ST bytecode magic value.
    #[error("invalid bytecode magic")]
    InvalidMagic,
    /// The bytecode major version is not supported by this runtime.
    #[error("unsupported bytecode version {major}.{minor}")]
    UnsupportedVersion {
        /// Observed major version.
        major: u16,
        /// Observed minor version.
        minor: u16,
    },
    /// Header fields are internally inconsistent.
    #[error("invalid bytecode header: {0}")]
    InvalidHeader(SmolStr),
    /// CRC32 checksum validation failed.
    #[error("invalid bytecode checksum (expected {expected:#010x}, got {actual:#010x})")]
    InvalidChecksum {
        /// Expected checksum from the header.
        expected: u32,
        /// Actual checksum computed from the payload.
        actual: u32,
    },
    /// Section table fields are internally inconsistent.
    #[error("invalid section table: {0}")]
    InvalidSectionTable(SmolStr),
    /// A section points outside the byte stream.
    #[error("section out of bounds")]
    SectionOutOfBounds,
    /// Two sections overlap.
    #[error("section overlap")]
    SectionOverlap,
    /// A section offset is not aligned as required by the format.
    #[error("section alignment error")]
    SectionAlignment,
    /// The stream ended before a required field was fully decoded.
    #[error("unexpected end of input")]
    UnexpectedEof,
    /// A section payload is malformed.
    #[error("invalid section data: {0}")]
    InvalidSection(SmolStr),
    /// A required section is missing.
    #[error("missing required section: {0}")]
    MissingSection(SmolStr),
    /// Instruction stream contains an unknown opcode.
    #[error("invalid opcode 0x{0:02X}")]
    InvalidOpcode(u8),
    /// Instruction stream contains an invalid jump target.
    #[error("invalid jump target {0}")]
    InvalidJumpTarget(i32),
    /// POU id does not exist in the POU index.
    #[error("invalid POU id {0}")]
    InvalidPouId(u32),
    /// Table index does not exist.
    #[error("invalid index {index} for {kind}")]
    InvalidIndex {
        /// Table kind, for diagnostics.
        kind: SmolStr,
        /// Invalid index value.
        index: u32,
    },
}

/// Bytecode reader utility for little-endian byte streams.
#[derive(Debug, Clone)]
pub struct BytecodeReader<'a> {
    data: &'a [u8],
    cursor: usize,
}

impl<'a> BytecodeReader<'a> {
    /// Create a reader over a borrowed byte slice.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, cursor: 0 }
    }

    /// Return the current cursor position.
    #[must_use]
    pub const fn pos(&self) -> usize {
        self.cursor
    }

    /// Return remaining bytes after the current cursor.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.cursor)
    }

    /// Read a borrowed byte range of length `len`.
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], BytecodeError> {
        if self.cursor + len > self.data.len() {
            return Err(BytecodeError::UnexpectedEof);
        }
        let start = self.cursor;
        self.cursor += len;
        Ok(&self.data[start..start + len])
    }

    /// Read one unsigned byte.
    pub fn read_u8(&mut self) -> Result<u8, BytecodeError> {
        Ok(self.read_bytes(1)?[0])
    }

    /// Read a little-endian `u16`.
    pub fn read_u16(&mut self) -> Result<u16, BytecodeError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Read a little-endian `u32`.
    pub fn read_u32(&mut self) -> Result<u32, BytecodeError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a little-endian `u64`.
    pub fn read_u64(&mut self) -> Result<u64, BytecodeError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a little-endian `i32`.
    pub fn read_i32(&mut self) -> Result<i32, BytecodeError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a little-endian `i64`.
    pub fn read_i64(&mut self) -> Result<i64, BytecodeError> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
}

/// Return `value` rounded up to the next 4-byte boundary.
#[must_use]
pub const fn align4(value: usize) -> usize {
    (value + 3) & !3
}

/// Pad `bytes` with zeroes until it reaches `target` length.
pub fn pad_to(bytes: &mut Vec<u8>, target: usize) {
    if bytes.len() < target {
        bytes.resize(target, 0);
    }
}

/// Process image sizing derived from bytecode metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessImageConfig {
    /// Input image byte length.
    pub inputs: usize,
    /// Output image byte length.
    pub outputs: usize,
    /// Marker memory image byte length.
    pub memory: usize,
}

/// Resource metadata captured in a bytecode module.
#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    /// Resource name.
    pub name: SmolStr,
    /// Process image sizing for the resource.
    pub process_image: ProcessImageConfig,
    /// Task definitions associated with the resource.
    pub tasks: Vec<TaskConfig>,
}

/// Bytecode metadata for a configuration.
#[derive(Debug, Clone)]
pub struct BytecodeMetadata {
    /// Bytecode format version.
    pub version: BytecodeVersion,
    /// Resources encoded by the bytecode module.
    pub resources: Vec<ResourceMetadata>,
}

impl BytecodeMetadata {
    /// Lookup a resource by name.
    #[must_use]
    pub fn resource(&self, name: &str) -> Option<&ResourceMetadata> {
        self.resources
            .iter()
            .find(|resource| resource.name.eq_ignore_ascii_case(name))
    }

    /// Return the first resource, if any.
    #[must_use]
    pub fn primary_resource(&self) -> Option<&ResourceMetadata> {
        self.resources.first()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BytecodeMetadata, BytecodeVersion, ProcessImageConfig, ResourceMetadata,
        SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
    };
    use crate::task::TaskConfig;
    use crate::value::Duration;
    use alloc::{vec, vec::Vec};
    use smol_str::SmolStr;

    #[test]
    fn bytecode_metadata_resource_lookup_is_case_insensitive() {
        let metadata = BytecodeMetadata {
            version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION),
            resources: vec![ResourceMetadata {
                name: SmolStr::new("ResourceA"),
                process_image: ProcessImageConfig {
                    inputs: 1,
                    outputs: 2,
                    memory: 3,
                },
                tasks: vec![TaskConfig {
                    name: SmolStr::new("MainTask"),
                    interval: Duration::from_millis(20),
                    single: None,
                    priority: 1,
                    programs: vec![SmolStr::new("Main")],
                    fb_instances: Vec::new(),
                }],
            }],
        };

        let resource = metadata.resource("resourcea").expect("resource");
        assert_eq!(
            metadata.primary_resource().map(|entry| &entry.name),
            Some(&resource.name)
        );
        assert_eq!(resource.process_image.outputs, 2);
        assert_eq!(resource.tasks[0].interval, Duration::from_millis(20));
    }

    #[test]
    fn bytecode_reader_preserves_little_endian_contract_and_eof() {
        let mut reader = super::BytecodeReader::new(&[
            0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 0x88, 0x77, 0x66, 0x55,
        ]);

        assert_eq!(reader.read_u16().unwrap(), 0x1234);
        assert_eq!(reader.read_u32().unwrap(), 0x12345678);
        assert_eq!(reader.pos(), 6);
        assert_eq!(reader.read_i32().unwrap(), 0x55667788);
        assert_eq!(reader.remaining(), 0);
        assert!(matches!(
            reader.read_u8(),
            Err(super::BytecodeError::UnexpectedEof)
        ));
    }

    #[test]
    fn bytecode_alignment_helpers_preserve_zero_padding_contract() {
        assert_eq!(super::align4(0), 0);
        assert_eq!(super::align4(1), 4);
        assert_eq!(super::align4(4), 4);
        assert_eq!(super::align4(5), 8);

        let mut bytes = vec![1, 2, 3];
        super::pad_to(&mut bytes, 6);
        assert_eq!(bytes, vec![1, 2, 3, 0, 0, 0]);
    }

    #[test]
    fn bytecode_format_records_preserve_raw_discriminants() {
        assert_eq!(
            super::SectionId::from_raw(0x0001),
            Some(super::SectionId::StringTable)
        );
        assert_eq!(
            super::TypeKind::from_raw(8),
            Some(super::TypeKind::FunctionBlock)
        );
        assert_eq!(
            super::RefLocation::from_raw(3),
            Some(super::RefLocation::Io)
        );
        assert_eq!(super::PouKind::from_raw(3), Some(super::PouKind::Class));
        assert!(super::PouKind::FunctionBlock.is_class_like());
        assert!(super::PouKind::Class.is_class_like());
        assert!(!super::PouKind::Function.is_class_like());
    }
}
