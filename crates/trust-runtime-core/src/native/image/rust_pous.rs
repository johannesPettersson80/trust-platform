//! The Rust POU table `RUSTPOUS` (specification 44 §5.13): the Rust POUs of a debug image whose
//! module holds the project's Rust crate, in the order of the numbers by which the generated code
//! names their records (§8.2), each with its name and its budget for the image's target
//! (specification 48 RU-46, RU-49). The table lies before `SIGN`, so the signature covers it.

use super::records::{u32_at, u64_at};
use super::{record, Image, ImageError, Record, SectionId};

/// One Rust POU: 16 bytes (specification 44 §5.13, `RUSTPOUS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RustPouRecord {
    /// The POU's name as Structured Text declares it, a method's as its block's and its own joined
    /// by a dot, a string offset.
    pub name: u32,
    /// Reserved, 0.
    pub reserved: u32,
    /// The POU's budget for the image's target in nanoseconds.
    pub budget_ns: u64,
}

impl Record for RustPouRecord {
    const SECTION: SectionId = SectionId::RustPous;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            name: u32_at(bytes, 0),
            reserved: u32_at(bytes, 4),
            budget_ns: u64_at(bytes, 8),
        }
    }
    fn write(&self, out: &mut [u8]) {
        out[0..4].copy_from_slice(&self.name.to_le_bytes());
        out[4..8].copy_from_slice(&self.reserved.to_le_bytes());
        out[8..16].copy_from_slice(&self.budget_ns.to_le_bytes());
    }
}

impl Image<'_> {
    /// Every Rust POU's name a string of the pool that is not empty, its reserved word 0, its
    /// budget from 1 ns to `i64::MAX` ns, and the POUs in the order of their names in capitals,
    /// each once (specification 44 §10.2, step 5). An image without the table has no Rust POU.
    pub(super) fn check_rust_pous(&self) -> Result<(), ImageError> {
        if self.section(SectionId::RustPous).is_none() {
            return Ok(());
        }
        let pous = self.table::<RustPouRecord>()?;
        let mut previous: Option<alloc::string::String> = None;
        for (index, entry) in (0u32..).zip(pous.iter()) {
            let Some(name) = self.string(entry.name).filter(|name| !name.is_empty()) else {
                return Err(record(
                    SectionId::RustPous,
                    index,
                    "name not a string of STRINGS",
                ));
            };
            if entry.reserved != 0 {
                return Err(record(SectionId::RustPous, index, "reserved word not 0"));
            }
            if entry.budget_ns == 0 || i64::try_from(entry.budget_ns).is_err() {
                return Err(record(
                    SectionId::RustPous,
                    index,
                    "budget not from 1 ns to i64::MAX ns",
                ));
            }
            let key = name.to_ascii_uppercase();
            if previous.as_ref().is_some_and(|previous| *previous >= key) {
                return Err(record(
                    SectionId::RustPous,
                    index,
                    "out of order or repeated",
                ));
            }
            previous = Some(key);
        }
        Ok(())
    }
}
