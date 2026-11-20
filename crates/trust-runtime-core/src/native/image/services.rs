//! The service table `SERVICES` (specification 44 §5.14; specification 48 RU-63): each service of
//! the project's `trust.toml` whose `writes` names a variable, in `trust.toml`'s order, with the
//! paths of the variables it may write. The table lies before `SIGN`, so the signature covers it,
//! and a device holds a service to it, a chip as a computer.

use alloc::string::String;
use alloc::vec::Vec;

use super::records::u32_at;
use super::{record, Image, ImageError, Record, SectionId, TABLE_PREFIX_BYTES};
use crate::native::reflect::resolve_writable;

/// One service: 16 bytes (specification 44 §5.14, `SERVICES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceRecord {
    /// The service's name as `trust.toml` names it, a string offset.
    pub name: u32,
    /// The index of its first path in the path list that follows the records.
    pub path_first: u32,
    /// The number of its paths.
    pub path_count: u32,
    /// Reserved, 0.
    pub reserved: u32,
}

impl Record for ServiceRecord {
    const SECTION: SectionId = SectionId::Services;
    const BYTES: usize = 16;
    fn read(bytes: &[u8]) -> Self {
        Self {
            name: u32_at(bytes, 0),
            path_first: u32_at(bytes, 4),
            path_count: u32_at(bytes, 8),
            reserved: u32_at(bytes, 12),
        }
    }
    fn write(&self, out: &mut [u8]) {
        out[0..4].copy_from_slice(&self.name.to_le_bytes());
        out[4..8].copy_from_slice(&self.path_first.to_le_bytes());
        out[8..12].copy_from_slice(&self.path_count.to_le_bytes());
        out[12..16].copy_from_slice(&self.reserved.to_le_bytes());
    }
}

/// A service of the table with the paths of the variables it may write, as the project spells them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageService<'a> {
    /// The service's name.
    pub name: &'a str,
    /// The paths, in the order `writes` gives them.
    pub paths: Vec<&'a str>,
}

impl<'a> Image<'a> {
    /// The string offsets of the paths of `service`, in order: the entries of the path list that
    /// follows the records, as the task table's program list follows its records.
    pub fn service_paths(&self, service: &ServiceRecord) -> impl Iterator<Item = u32> + 'a {
        let bytes = self.section_bytes(SectionId::Services);
        let records = usize::try_from(u32_at(bytes, 0))
            .unwrap_or(0)
            .saturating_mul(ServiceRecord::BYTES)
            .saturating_add(TABLE_PREFIX_BYTES);
        let first = usize::try_from(service.path_first).unwrap_or(usize::MAX);
        let count = usize::try_from(service.path_count).unwrap_or(0);
        let start = first.saturating_mul(4).saturating_add(records);
        bytes
            .get(start..start.saturating_add(count.saturating_mul(4)))
            .unwrap_or(&[])
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
    }

    /// The services of the table, each with its paths; none for an image without the table, which
    /// lets no service write a variable. Read after [`Image::check_tables`], a path that is no string
    /// is left out.
    ///
    /// # Errors
    ///
    /// [`ImageError::Table`] for a table whose records do not fit its section.
    pub fn services(&self) -> Result<Vec<ImageService<'a>>, ImageError> {
        if self.section(SectionId::Services).is_none() {
            return Ok(Vec::new());
        }
        let table = self.table::<ServiceRecord>()?;
        Ok(table
            .iter()
            .map(|entry| ImageService {
                name: self.string(entry.name).unwrap_or_default(),
                paths: self
                    .service_paths(&entry)
                    .filter_map(|path| self.string(path))
                    .collect(),
            })
            .collect())
    }

    /// Every service's name a string of the pool that is not empty and differs from every other's
    /// without case, its reserved word 0, its path list inside the section with at least one path,
    /// and each path a string of the pool that names one elementary value that is no constant
    /// (specification 44 §10.2, step 5). An image without the table has no service that writes.
    pub(super) fn check_services(&self) -> Result<(), ImageError> {
        if self.section(SectionId::Services).is_none() {
            return Ok(());
        }
        let services = self.table::<ServiceRecord>()?;
        let mut names: Vec<String> = Vec::new();
        for (index, entry) in (0u32..).zip(services.iter()) {
            let Some(name) = self.string(entry.name).filter(|name| !name.is_empty()) else {
                return Err(record(
                    SectionId::Services,
                    index,
                    "name not a string of STRINGS",
                ));
            };
            if entry.reserved != 0 {
                return Err(record(SectionId::Services, index, "reserved word not 0"));
            }
            let key = name.to_ascii_uppercase();
            if names.contains(&key) {
                return Err(record(SectionId::Services, index, "name repeated"));
            }
            names.push(key);
            let listed = self.service_paths(&entry).count();
            if entry.path_count == 0
                || listed != usize::try_from(entry.path_count).unwrap_or(usize::MAX)
            {
                return Err(record(
                    SectionId::Services,
                    index,
                    "path list empty or outside SERVICES",
                ));
            }
            for offset in self.service_paths(&entry) {
                let Some(path) = self.string(offset).filter(|path| !path.is_empty()) else {
                    return Err(record(
                        SectionId::Services,
                        index,
                        "path not a string of STRINGS",
                    ));
                };
                if resolve_writable(self, path).is_err() {
                    return Err(record(
                        SectionId::Services,
                        index,
                        "path names no elementary variable a write reaches",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;
