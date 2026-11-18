//! The bytes of the record's crate section (specification 34 §5.1.1, section 9): the Rust crates of
//! the program image's crate table (specification 44 §5.12).

use super::Reader;
use crate::deployment::DeploymentRecordError as Error;
use crate::deployment::{CrateSource, RustCrate};
use alloc::vec::Vec;
use smol_str::SmolStr;

/// The longest name or version: the maximum of its 1-byte length.
const MAX_TEXT_BYTES: usize = 255;
/// One crate's bytes before its name and version: the source, the two lengths, a reserved byte
/// and the checksum.
const CRATE_HEADER_BYTES: usize = 1 + 1 + 1 + 1 + 32;

/// The length of section 9's payload in bytes, exactly what [`encode`] writes.
pub(super) fn encoded_len(crates: &[RustCrate]) -> usize {
    crates.iter().fold(4usize, |bytes, entry| {
        bytes
            .saturating_add(CRATE_HEADER_BYTES)
            .saturating_add(entry.name.len())
            .saturating_add(entry.version.len())
    })
}

/// Writes section 9's payload.
///
/// # Errors
///
/// More than 65,535 crates, a name or version that is empty or longer than 255 bytes, a checksum
/// with a byte its source leaves zero that is not 0, or crates out of their order or repeated.
pub(super) fn encode(crates: &[RustCrate], out: &mut Vec<u8>) -> Result<(), Error> {
    let count = u16::try_from(crates.len()).map_err(|_| Error::Field("the crate count"))?;
    for entry in crates {
        check(entry)?;
    }
    check_order(crates)?;
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    for entry in crates {
        out.push(entry.source as u8);
        // Checked above: every name and version is 1 to 255 bytes long.
        out.push(u8::try_from(entry.name.len()).unwrap_or(u8::MAX));
        out.push(u8::try_from(entry.version.len()).unwrap_or(u8::MAX));
        out.push(0);
        out.extend_from_slice(&entry.checksum);
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(entry.version.as_bytes());
    }
    Ok(())
}

/// Reads section 9's payload.
///
/// # Errors
///
/// A source other than 1, 2 or 3, a reserved byte other than 0, an empty or non-UTF-8 name or
/// version, a checksum with a byte its source leaves zero that is not 0, crates out of their order
/// or repeated, or a count the payload does not hold.
pub(super) fn read(reader: &mut Reader<'_>) -> Result<Vec<RustCrate>, Error> {
    let count = usize::from(reader.u16()?);
    if reader.u16()? != 0 {
        return Err(Error::Reserved);
    }
    // Every crate takes at least its header and one byte each of name and version.
    if count.saturating_mul(CRATE_HEADER_BYTES + 2) > reader.remaining() {
        return Err(Error::Truncated);
    }
    let mut crates = Vec::new();
    crates.try_reserve_exact(count).map_err(|_| Error::Memory)?;
    for _ in 0..count {
        let source = match reader.u8()? {
            1 => CrateSource::Registry,
            2 => CrateSource::Path,
            3 => CrateSource::Git,
            _ => return Err(Error::Field("a crate source")),
        };
        let name_length = usize::from(reader.u8()?);
        let version_length = usize::from(reader.u8()?);
        if reader.u8()? != 0 {
            return Err(Error::Reserved);
        }
        let checksum = reader.digest()?;
        let mut text = |length: usize| {
            reader.take(length).and_then(|bytes| {
                core::str::from_utf8(bytes)
                    .ok()
                    .filter(|text| !text.is_empty())
                    .map(SmolStr::new)
                    .ok_or(Error::Field("a crate name or version"))
            })
        };
        let name = text(name_length)?;
        let version = text(version_length)?;
        let entry = RustCrate {
            name,
            version,
            source,
            checksum,
        };
        check(&entry)?;
        crates.push(entry);
    }
    check_order(&crates)?;
    Ok(crates)
}

/// A name and a version of 1 to 255 bytes, and zero where the source fills no checksum.
fn check(entry: &RustCrate) -> Result<(), Error> {
    for text in [&entry.name, &entry.version] {
        if text.is_empty() || text.len() > MAX_TEXT_BYTES {
            return Err(Error::Field("a crate name or version"));
        }
    }
    if entry.checksum[entry.source.checksum_bytes()..]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::Field("a crate checksum"));
    }
    Ok(())
}

/// Crates are ordered by the bytes of their name, then of their version, then by their source,
/// each once.
fn check_order(crates: &[RustCrate]) -> Result<(), Error> {
    fn key(entry: &RustCrate) -> (&[u8], &[u8], CrateSource) {
        (
            entry.name.as_bytes(),
            entry.version.as_bytes(),
            entry.source,
        )
    }
    if crates.windows(2).all(|pair| key(&pair[0]) < key(&pair[1])) {
        Ok(())
    } else {
        Err(Error::Field("the crate order"))
    }
}
