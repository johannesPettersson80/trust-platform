//! Shared IEC direct-address syntax; driver access remains in the host/platform adapter.

use crate::{error::RuntimeError, memory::IoArea};
use alloc::{string::String, vec::Vec};

/// Width selected by an IEC direct address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoSize {
    /// One bit.
    Bit,
    /// One byte.
    Byte,
    /// Two bytes.
    Word,
    /// Four bytes.
    DWord,
    /// Eight bytes.
    LWord,
    /// Explicit byte sequence length.
    Bytes(u32),
}

/// Parsed direct address, including hierarchical paths and unresolved wildcards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoAddress {
    /// Process-image area.
    pub area: IoArea,
    /// Selected access width.
    pub size: IoSize,
    /// First path component or flat byte offset.
    pub byte: u32,
    /// Bit within a selected byte.
    pub bit: u8,
    /// Hierarchical address components.
    pub path: Vec<u32>,
    /// Whether configuration must supply the final address.
    pub wildcard: bool,
}

/// Existing hosted container ceiling for each flat process-image area.
pub const PROCESS_IMAGE_AREA_LIMIT: usize = 16 * 1024 * 1024;

impl IoAddress {
    /// Exclusive flat byte extent; hierarchical addresses occupy a separate space.
    pub fn flat_byte_end(&self) -> Result<Option<u32>, RuntimeError> {
        if self.wildcard {
            return Err(RuntimeError::InvalidIoAddress(
                "unresolved wildcard address".into(),
            ));
        }
        if self.path.len() > 1 {
            return Ok(None);
        }
        let width = match self.size {
            IoSize::Bit | IoSize::Byte => 1,
            IoSize::Word => 2,
            IoSize::DWord => 4,
            IoSize::LWord => 8,
            IoSize::Bytes(width) if width != 0 => width,
            IoSize::Bytes(_) => {
                return Err(RuntimeError::InvalidIoAddress(
                    "zero-length byte window".into(),
                ))
            }
        };
        self.byte
            .checked_add(width)
            .map(Some)
            .ok_or_else(|| RuntimeError::InvalidIoAddress("process image address overflow".into()))
    }

    /// Parse the existing IEC direct-address syntax without accessing hardware.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        let trimmed = text.trim();
        if !trimmed.starts_with('%') {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }
        let mut chars = trimmed[1..].chars();
        let area = match chars.next() {
            Some('I') => IoArea::Input,
            Some('Q') => IoArea::Output,
            Some('M') => IoArea::Memory,
            _ => return Err(RuntimeError::InvalidIoAddress(trimmed.into())),
        };
        let rest: String = chars.collect();
        if rest.is_empty() {
            return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
        }
        if rest == "*" {
            return Ok(Self {
                area,
                size: IoSize::Bit,
                byte: 0,
                bit: 0,
                path: Vec::new(),
                wildcard: true,
            });
        }

        let mut rest_chars = rest.chars();
        let first = rest_chars
            .next()
            .ok_or_else(|| RuntimeError::InvalidIoAddress(trimmed.into()))?;
        let (size, rest) = match first {
            'X' => (IoSize::Bit, rest_chars.as_str()),
            'B' => (IoSize::Byte, rest_chars.as_str()),
            'W' => (IoSize::Word, rest_chars.as_str()),
            'D' => (IoSize::DWord, rest_chars.as_str()),
            'L' => (IoSize::LWord, rest_chars.as_str()),
            ch if ch.is_ascii_digit() => (IoSize::Bit, rest.as_str()),
            _ => return Err(RuntimeError::InvalidIoAddress(trimmed.into())),
        };

        let mut path: Vec<u32> = Vec::new();
        let mut bit = 0u8;
        let parts: Vec<&str> = rest.split('.').collect();
        if matches!(size, IoSize::Bit) && parts.len() >= 2 {
            for part in &parts[..parts.len() - 1] {
                path.push(parse_u32(part, trimmed)?);
            }
            let bit_part = parts[parts.len() - 1];
            bit = parse_u8(bit_part, trimmed)?;
            if bit > 7 {
                return Err(RuntimeError::InvalidIoAddress(trimmed.into()));
            }
        } else {
            for part in &parts {
                path.push(parse_u32(part, trimmed)?);
            }
        }
        let byte = path
            .first()
            .copied()
            .ok_or_else(|| RuntimeError::InvalidIoAddress(trimmed.into()))?;
        Ok(Self {
            area,
            size,
            byte,
            bit,
            path,
            wildcard: false,
        })
    }
}

fn parse_u32(value: &str, full: &str) -> Result<u32, RuntimeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RuntimeError::InvalidIoAddress(full.into()));
    }
    value
        .parse::<u32>()
        .map_err(|_| RuntimeError::InvalidIoAddress(full.into()))
}

fn parse_u8(value: &str, full: &str) -> Result<u8, RuntimeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RuntimeError::InvalidIoAddress(full.into()));
    }
    value
        .parse::<u8>()
        .map_err(|_| RuntimeError::InvalidIoAddress(full.into()))
}
