//! The crate table `CRATES` (specification 44 §5.12): the Rust crates whose code the module holds,
//! each with its name, version, source and checksum. The table lies before `SIGN`, so the signature
//! covers it with the module.

use super::records::u32_at;
use super::{record, Image, ImageError, Record, SectionId};

/// Where a crate of the scan crate graph comes from (specification 44 §5.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CrateSource {
    /// A registry: the checksum is the SHA-256 of the crate's archive that `Cargo.lock` pins.
    Registry = 1,
    /// A folder: `Cargo.lock` pins no checksum, and the checksum is zero.
    Path = 2,
    /// A git repository: the checksum's first 20 bytes are the commit, the others zero.
    Git = 3,
}

impl CrateSource {
    /// The source numbered `raw`.
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        [Self::Registry, Self::Path, Self::Git]
            .into_iter()
            .find(|source| *source as u32 == raw)
    }

    /// The leading bytes of the checksum that the source fills; the bytes after them are zero.
    #[must_use]
    pub fn checksum_bytes(self) -> usize {
        match self {
            Self::Registry => 32,
            Self::Path => 0,
            Self::Git => 20,
        }
    }

    /// The source's name as `build.json` writes it: `registry`, `path` or `git`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Path => "path",
            Self::Git => "git",
        }
    }
}

/// One crate: 44 bytes (specification 44 §5.12, `CRATES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrateRecord {
    /// The crate's name, a string offset.
    pub name: u32,
    /// The crate's version, a string offset.
    pub version: u32,
    /// The crate's source, a [`CrateSource`] by its number.
    pub source: u32,
    /// The crate's checksum, as its source fills it.
    pub checksum: [u8; 32],
}

impl Record for CrateRecord {
    const SECTION: SectionId = SectionId::Crates;
    const BYTES: usize = 44;
    fn read(bytes: &[u8]) -> Self {
        let mut checksum = [0; 32];
        checksum.copy_from_slice(&bytes[12..44]);
        Self {
            name: u32_at(bytes, 0),
            version: u32_at(bytes, 4),
            source: u32_at(bytes, 8),
            checksum,
        }
    }
    fn write(&self, out: &mut [u8]) {
        out[0..4].copy_from_slice(&self.name.to_le_bytes());
        out[4..8].copy_from_slice(&self.version.to_le_bytes());
        out[8..12].copy_from_slice(&self.source.to_le_bytes());
        out[12..44].copy_from_slice(&self.checksum);
    }
}

impl Image<'_> {
    /// Every crate's name and version a string of the pool that is not empty, its source 1, 2 or
    /// 3, the checksum's bytes that its source leaves zero zero, and the crates ordered by name,
    /// version and source, each once (specification 44 §10.2, step 5). An image without the table
    /// holds no crate.
    pub(super) fn check_crates(&self) -> Result<(), ImageError> {
        if self.section(SectionId::Crates).is_none() {
            return Ok(());
        }
        let crates = self.table::<CrateRecord>()?;
        let mut previous: Option<(&str, &str, u32)> = None;
        for (index, entry) in (0u32..).zip(crates.iter()) {
            let text = |offset| self.string(offset).filter(|text| !text.is_empty());
            let (Some(name), Some(version)) = (text(entry.name), text(entry.version)) else {
                return Err(record(
                    SectionId::Crates,
                    index,
                    "name or version not a string of STRINGS",
                ));
            };
            let Some(source) = CrateSource::from_raw(entry.source) else {
                return Err(record(SectionId::Crates, index, "unknown source"));
            };
            if entry.checksum[source.checksum_bytes()..]
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(record(
                    SectionId::Crates,
                    index,
                    "checksum bytes its source leaves zero are not zero",
                ));
            }
            let key = (name, version, entry.source);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(record(SectionId::Crates, index, "out of order or repeated"));
            }
            previous = Some(key);
        }
        Ok(())
    }
}
