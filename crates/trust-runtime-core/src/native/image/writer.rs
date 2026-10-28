//! Writing an image, as the build's image builder does (specification 44 §3, step 5).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use super::{
    Record, SectionId, Target, CODE_ALIGNMENT, FORMAT_VERSION, HEADER_BYTES, MAGIC,
    SECTION_ALIGNMENT, SECTION_ENTRY_BYTES, TABLE_PREFIX_BYTES,
};

/// A module's code and read-only data as the linker placed them: `RODATA` lies `rodata_distance`
/// bytes after the start of `CODE`, a multiple of 8 not below the code's length.
#[derive(Clone, Debug, Default)]
pub struct CodeImage {
    /// The executable code.
    pub code: Vec<u8>,
    /// The read-only data.
    pub rodata: Vec<u8>,
    /// The distance from the first byte of the code to the first byte of the read-only data.
    pub rodata_distance: u32,
}

/// The pool of an image's strings: each distinct string once, the empty string at offset 0.
#[derive(Clone, Debug)]
pub struct StringPool {
    bytes: Vec<u8>,
    offsets: BTreeMap<String, u32>,
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new()
    }
}

impl StringPool {
    /// A pool that holds the empty string.
    #[must_use]
    pub fn new() -> Self {
        let mut offsets = BTreeMap::new();
        offsets.insert(String::new(), 0);
        Self {
            bytes: alloc::vec![0],
            offsets,
        }
    }

    /// The offset of `text`, added when the pool does not hold it yet. A text with a NUL byte is
    /// cut at it.
    pub fn add(&mut self, text: &str) -> u32 {
        let text = text.split('\0').next().unwrap_or("");
        if let Some(offset) = self.offsets.get(text) {
            return *offset;
        }
        let offset = u32::try_from(self.bytes.len()).unwrap_or(u32::MAX);
        self.bytes.extend_from_slice(text.as_bytes());
        self.bytes.push(0);
        self.offsets.insert(String::from(text), offset);
        offset
    }

    /// The pool's bytes, the `STRINGS` section.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Why an image could not be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// The image would be longer than 4 GiB.
    TooLong,
    /// `RODATA` would overlap `CODE` or is not 8-byte aligned after it.
    RodataPlacement,
}

/// Collects the sections of an image and writes it.
#[derive(Clone, Debug)]
pub struct ImageWriter {
    target: Target,
    interface_version: u16,
    /// The build time, Unix seconds, informational (specification 44 NX-07).
    pub build_time: u64,
    /// The SHA-256 of the project's sources and settings.
    pub source_hash: [u8; 32],
    /// The Rust compiler's version.
    pub rustc: String,
    /// The program library's version and flags hash.
    pub library: [u8; 16],
    /// Whether the image is a release image, built without statement hooks (specification 44
    /// §13.2); its header flags say so.
    pub release: bool,
    /// Whether the program reads the platform's wall clock (`CURRENT_DT`); its header flags say so.
    pub wall_clock: bool,
    /// The conversion rules the program was built with (specification 07 §2.10); its header flags
    /// say when they are the CODESYS rules.
    pub conversion_profile: crate::stdlib::conversions::ConversionProfile,
    /// The offset plus one in `STRINGS` of the `RESOURCE` name the program's `CONFIGURATION`
    /// declares, or zero when it declares none (§5.2).
    pub resource: u32,
    module: CodeImage,
    sections: BTreeMap<u32, Vec<u8>>,
}

fn align_up(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

impl ImageWriter {
    /// A writer of an image for `target` built against kernel interface `interface_version`, which
    /// links this release's program library until [`Self::library`] says otherwise (§5.2).
    #[must_use]
    pub fn new(target: Target, interface_version: u16) -> Self {
        Self {
            target,
            interface_version,
            build_time: 0,
            source_hash: [0; 32],
            rustc: String::new(),
            library: crate::native::library::library_hash(
                crate::native::library::PROGRAM_LIBRARY,
                target,
            ),
            release: false,
            wall_clock: false,
            conversion_profile: crate::stdlib::conversions::ConversionProfile::Iec,
            resource: 0,
            module: CodeImage::default(),
            sections: BTreeMap::new(),
        }
    }

    /// Sets the module's code and read-only data.
    pub fn module(&mut self, module: CodeImage) {
        self.module = module;
    }

    /// Sets section `id` to `bytes`; `CODE` and `RODATA` come from [`Self::module`].
    pub fn section(&mut self, id: SectionId, bytes: Vec<u8>) {
        if !matches!(id, SectionId::Code | SectionId::Rodata) {
            self.sections.insert(id as u32, bytes);
        }
    }

    /// Sets the table of `R`: its count, its records and the `trailing` bytes its records name.
    pub fn table<R: Record>(&mut self, records: &[R], trailing: &[u8]) {
        let mut bytes =
            alloc::vec![0u8; TABLE_PREFIX_BYTES + records.len() * R::BYTES + trailing.len()];
        let count = u32::try_from(records.len()).unwrap_or(u32::MAX);
        bytes[..4].copy_from_slice(&count.to_le_bytes());
        for (index, record) in records.iter().enumerate() {
            let start = TABLE_PREFIX_BYTES + index * R::BYTES;
            record.write(&mut bytes[start..start + R::BYTES]);
        }
        let start = TABLE_PREFIX_BYTES + records.len() * R::BYTES;
        bytes[start..].copy_from_slice(trailing);
        self.section(R::SECTION, bytes);
    }

    /// Writes the image.
    ///
    /// # Errors
    ///
    /// [`WriteError`] for an image that cannot be laid out.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        let rodata_distance =
            usize::try_from(self.module.rodata_distance).map_err(|_| WriteError::TooLong)?;
        if rodata_distance < self.module.code.len() || rodata_distance % SECTION_ALIGNMENT != 0 {
            return Err(WriteError::RodataPlacement);
        }
        let count = 2 + self.sections.len();
        let table_end = HEADER_BYTES + count * SECTION_ENTRY_BYTES;
        let code_offset = align_up(table_end, CODE_ALIGNMENT);
        let rodata_offset = code_offset + rodata_distance;
        let mut placed: Vec<(u32, usize, &[u8])> = Vec::with_capacity(count);
        placed.push((SectionId::Code as u32, code_offset, &self.module.code));
        placed.push((SectionId::Rodata as u32, rodata_offset, &self.module.rodata));
        let mut end = rodata_offset + self.module.rodata.len();
        // The other sections in the order of their ids, which puts SIGN last.
        for (id, bytes) in &self.sections {
            let offset = align_up(end, SECTION_ALIGNMENT);
            placed.push((*id, offset, bytes));
            end = offset + bytes.len();
        }
        let length = u32::try_from(end).map_err(|_| WriteError::TooLong)?;
        let mut image = alloc::vec![0u8; end];
        image[..8].copy_from_slice(&MAGIC);
        image[12..16].copy_from_slice(&length.to_le_bytes());
        image[16..18].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        image[18..20].copy_from_slice(&self.interface_version.to_le_bytes());
        image[20..24].copy_from_slice(&(self.target as u32).to_le_bytes());
        image[24..32].copy_from_slice(&self.build_time.to_le_bytes());
        image[32..64].copy_from_slice(&self.source_hash);
        let rustc = self.rustc.as_bytes();
        let rustc_len = rustc.len().min(32);
        image[64..64 + rustc_len].copy_from_slice(&rustc[..rustc_len]);
        image[96..112].copy_from_slice(&self.library);
        let count = u32::try_from(count).map_err(|_| WriteError::TooLong)?;
        image[112..116].copy_from_slice(&count.to_le_bytes());
        let table = u32::try_from(HEADER_BYTES).map_err(|_| WriteError::TooLong)?;
        image[116..120].copy_from_slice(&table.to_le_bytes());
        let mut flags = if self.release { super::FLAG_RELEASE } else { 0 };
        if self.wall_clock {
            flags |= super::FLAG_WALL_CLOCK;
        }
        if self.conversion_profile == crate::stdlib::conversions::ConversionProfile::Codesys {
            flags |= super::FLAG_CODESYS_CONVERSIONS;
        }
        image[120..124].copy_from_slice(&flags.to_le_bytes());
        image[124..128].copy_from_slice(&self.resource.to_le_bytes());
        for (index, (id, offset, bytes)) in placed.iter().enumerate() {
            let entry = HEADER_BYTES + index * SECTION_ENTRY_BYTES;
            let offset32 = u32::try_from(*offset).map_err(|_| WriteError::TooLong)?;
            let length32 = u32::try_from(bytes.len()).map_err(|_| WriteError::TooLong)?;
            image[entry..entry + 4].copy_from_slice(&id.to_le_bytes());
            image[entry + 8..entry + 12].copy_from_slice(&offset32.to_le_bytes());
            image[entry + 12..entry + 16].copy_from_slice(&length32.to_le_bytes());
            image[*offset..*offset + bytes.len()].copy_from_slice(bytes);
        }
        let crc = crate::crc32::checksum(&image[12..table_end]);
        image[8..12].copy_from_slice(&crc.to_le_bytes());
        Ok(image)
    }
}
