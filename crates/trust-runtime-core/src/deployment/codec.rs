//! The bytes of a deployment record (specification 34 §5.1.1): a header, sections in ascending
//! order of their identifiers, and a CRC-32.
//!
//! Version 1 has exactly the sections 1 to 7. A later version adds sections with new
//! identifiers after them and decodes the sections of version 1 with the functions below.

mod configuration;

use super::{
    ApplicationIdentity, Capabilities, Contracts, DeploymentRecord, DeploymentRecordError,
    ImportKind, LimitKind, NumericContract, ProcessImageSummary, RequestedLimits, RequiredImport,
    SchedulingProfile, TaskSummary, TimingEvidence, RECORD_MAX_BYTES, RECORD_VERSION,
};
use crate::bytecode::BytecodeVersion;
use crate::crc32;
use alloc::vec::Vec;
use smol_str::SmolStr;

use super::DeploymentRecordError as Error;

/// The magic that starts a record.
pub(super) const MAGIC: [u8; 4] = *b"TRDR";
/// Magic, version, section count and record length.
const HEADER_BYTES: usize = 12;
/// The CRC-32 that ends a record.
const CRC_BYTES: usize = 4;
/// Identifier, reserved bytes and payload length.
const SECTION_HEADER_BYTES: usize = 8;
/// The sections of version 1, in their order.
const SECTION_COUNT: u16 = 7;

const APPLICATION: u16 = 1;
const CONTRACTS: u16 = 2;
const LIMITS: u16 = 3;
const PROCESS_IMAGE: u16 = 4;
const TASKS: u16 = 5;
const IMPORTS: u16 = 6;
const CONFIGURATION: u16 = 7;

const APPLICATION_BYTES: usize = 32 + 4 + 2 + 2 + 4 + 32 + 32;
const CONTRACTS_BYTES: usize = 2 + 2 + 1 + 1 + 2;
const LIMITS_BYTES: usize = 8 * LimitKind::ALL.len();
const PROCESS_IMAGE_BYTES: usize = 4 * 4;
const TASK_BYTES: usize = 8 + 4 + 4;
/// Kind, name length and contract version; the name follows.
const IMPORT_HEADER_BYTES: usize = 1 + 1 + 2;
/// The only task flag version 1 defines: an event task.
const TASK_EVENT: u32 = 1;

impl DeploymentRecord {
    /// The record as the bytes of `deployment-record.bin`.
    ///
    /// # Errors
    ///
    /// The record breaks a rule a reader checks: more than 65,535 tasks or imports, an empty or
    /// too long import name, imports out of order or repeated, a configuration field the section
    /// cannot hold (specification 34 §5.1.1), or more than `RECORD_MAX_BYTES`.
    pub fn encode(&self) -> Result<Vec<u8>, DeploymentRecordError> {
        let tasks = u16::try_from(self.tasks.len()).map_err(|_| Error::Field("the task count"))?;
        let imports =
            u16::try_from(self.imports.len()).map_err(|_| Error::Field("the import count"))?;
        for import in &self.imports {
            if import.name.is_empty() || import.name.len() > usize::from(u8::MAX) {
                return Err(Error::Field("an import name"));
            }
        }
        check_import_order(&self.imports)?;
        // Section 7's payload goes into its own buffer first: its length heads the section, and
        // its fields are the record's only fallible ones (specification 34 §5.1.1).
        let mut configuration_bytes = Vec::new();
        configuration_bytes
            .try_reserve_exact(self.configuration.encoded_len())
            .map_err(|_| Error::Memory)?;
        self.configuration.encode(&mut configuration_bytes)?;
        let length = self.encoded_len();
        if length > RECORD_MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let mut out = Vec::new();
        out.try_reserve_exact(length).map_err(|_| Error::Memory)?;
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&RECORD_VERSION.to_le_bytes());
        out.extend_from_slice(&SECTION_COUNT.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(length)
                .map_err(|_| Error::TooLarge)?
                .to_le_bytes(),
        );

        let application = &self.application;
        section(&mut out, APPLICATION, APPLICATION_BYTES, |out| {
            out.extend_from_slice(&application.program_sha256);
            out.extend_from_slice(&application.program_bytes.to_le_bytes());
            out.extend_from_slice(&application.bytecode_version.major.to_le_bytes());
            out.extend_from_slice(&application.bytecode_version.minor.to_le_bytes());
            out.extend_from_slice(&application.bytecode_flags.to_le_bytes());
            out.extend_from_slice(&application.configuration_sha256);
            out.extend_from_slice(&application.retain_schema_sha256);
        });
        let contracts = &self.contracts;
        section(&mut out, CONTRACTS, CONTRACTS_BYTES, |out| {
            out.extend_from_slice(&contracts.scheduling_profile.0.to_le_bytes());
            out.extend_from_slice(&contracts.numeric_contract.0.to_le_bytes());
            out.push(contracts.timing_evidence.0);
            out.push(0);
            out.extend_from_slice(&contracts.capabilities.0.to_le_bytes());
        });
        section(&mut out, LIMITS, LIMITS_BYTES, |out| {
            for (_, value) in self.limits.values() {
                out.extend_from_slice(&value.to_le_bytes());
            }
        });
        let image = &self.process_image;
        section(&mut out, PROCESS_IMAGE, PROCESS_IMAGE_BYTES, |out| {
            for value in [
                image.input_bytes,
                image.output_bytes,
                image.marker_bytes,
                image.bindings,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        });
        section(&mut out, TASKS, self.tasks_bytes(), |out| {
            out.extend_from_slice(&tasks.to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            for task in &self.tasks {
                out.extend_from_slice(&task.interval_nanos.to_le_bytes());
                out.extend_from_slice(&task.priority.to_le_bytes());
                out.extend_from_slice(&u32::from(task.event).to_le_bytes());
            }
        });
        section(&mut out, IMPORTS, self.imports_bytes(), |out| {
            out.extend_from_slice(&imports.to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            for import in &self.imports {
                out.push(import.kind as u8);
                // Checked above: every name is 1 to 255 bytes long.
                out.push(u8::try_from(import.name.len()).unwrap_or(u8::MAX));
                out.extend_from_slice(&import.contract_version.to_le_bytes());
                out.extend_from_slice(import.name.as_bytes());
            }
        });
        section(&mut out, CONFIGURATION, configuration_bytes.len(), |out| {
            out.extend_from_slice(&configuration_bytes);
        });
        let crc = crc32::checksum(&out);
        out.extend_from_slice(&crc.to_le_bytes());
        debug_assert_eq!(out.len(), length);
        Ok(out)
    }

    /// The length of the encoded record in bytes, its header and CRC included.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        HEADER_BYTES
            + usize::from(SECTION_COUNT) * SECTION_HEADER_BYTES
            + APPLICATION_BYTES
            + CONTRACTS_BYTES
            + LIMITS_BYTES
            + PROCESS_IMAGE_BYTES
            + self.tasks_bytes()
            + self.imports_bytes()
            + self.configuration.encoded_len()
            + CRC_BYTES
    }

    fn tasks_bytes(&self) -> usize {
        4 + self.tasks.len().saturating_mul(TASK_BYTES)
    }

    fn imports_bytes(&self) -> usize {
        self.imports.iter().fold(4usize, |bytes, import| {
            bytes.saturating_add(IMPORT_HEADER_BYTES + import.name.len())
        })
    }

    /// Read a record from the bytes of `deployment-record.bin`.
    ///
    /// # Errors
    ///
    /// The bytes are not a record this runtime reads; the error names the reason.
    pub fn decode(bytes: &[u8]) -> Result<Self, DeploymentRecordError> {
        if bytes.len() > RECORD_MAX_BYTES {
            return Err(Error::TooLarge);
        }
        let mut header = Reader::new(bytes);
        if header.take(4)? != MAGIC {
            return Err(Error::Magic);
        }
        let version = header.u16()?;
        if version != RECORD_VERSION {
            return Err(Error::Version(version));
        }
        let count = header.u16()?;
        let length = header.u32()? as usize;
        if length > RECORD_MAX_BYTES {
            return Err(Error::TooLarge);
        }
        if length != bytes.len() {
            return Err(Error::Length);
        }
        if length < HEADER_BYTES + CRC_BYTES {
            return Err(Error::Truncated);
        }
        let (body, crc) = bytes.split_at(length - CRC_BYTES);
        let crc = u32::from_le_bytes([crc[0], crc[1], crc[2], crc[3]]);
        if crc32::checksum(body) != crc {
            return Err(Error::Checksum);
        }
        let mut sections = Reader::new(&body[HEADER_BYTES..]);
        let mut payloads: [Option<&[u8]>; SECTION_COUNT as usize] = [None; SECTION_COUNT as usize];
        let mut previous = 0u16;
        for _ in 0..count {
            let id = sections.u16()?;
            if sections.u16()? != 0 {
                return Err(Error::Reserved);
            }
            let payload_length = sections.u32()? as usize;
            let payload = sections.take(payload_length)?;
            if !(1..=SECTION_COUNT).contains(&id) {
                return Err(Error::UnknownSection(id));
            }
            if id <= previous {
                return Err(Error::SectionOrder(id));
            }
            previous = id;
            payloads[usize::from(id - 1)] = Some(payload);
        }
        if !sections.is_empty() {
            return Err(Error::Field("the section count"));
        }
        let payload = |id: u16| payloads[usize::from(id - 1)].ok_or(Error::MissingSection(id));
        for id in 1..=SECTION_COUNT {
            payload(id)?;
        }
        Ok(Self {
            application: read_section(APPLICATION, payload(APPLICATION)?, read_application)?,
            contracts: read_section(CONTRACTS, payload(CONTRACTS)?, read_contracts)?,
            limits: read_section(LIMITS, payload(LIMITS)?, read_limits)?,
            process_image: read_section(
                PROCESS_IMAGE,
                payload(PROCESS_IMAGE)?,
                read_process_image,
            )?,
            tasks: read_section(TASKS, payload(TASKS)?, read_tasks)?,
            imports: read_section(IMPORTS, payload(IMPORTS)?, read_imports)?,
            configuration: read_section(
                CONFIGURATION,
                payload(CONFIGURATION)?,
                configuration::read,
            )?,
        })
    }
}

/// Write one section: its header, then the payload `write` produces, `length` bytes long.
fn section(out: &mut Vec<u8>, id: u16, length: usize, write: impl FnOnce(&mut Vec<u8>)) {
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    // `encode` checked that the whole record, and so every section, is below 4 GiB.
    out.extend_from_slice(&u32::try_from(length).unwrap_or(u32::MAX).to_le_bytes());
    let start = out.len();
    write(out);
    debug_assert_eq!(out.len() - start, length, "section {id}");
}

/// Decode one section's payload; it must be exactly as long as its content.
fn read_section<'a, T>(
    id: u16,
    payload: &'a [u8],
    read: impl FnOnce(&mut Reader<'a>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut reader = Reader::new(payload);
    let value = read(&mut reader).map_err(|error| match error {
        Error::Truncated => Error::SectionLength(id),
        other => other,
    })?;
    if reader.is_empty() {
        Ok(value)
    } else {
        Err(Error::SectionLength(id))
    }
}

fn read_application(reader: &mut Reader<'_>) -> Result<ApplicationIdentity, Error> {
    Ok(ApplicationIdentity {
        program_sha256: reader.digest()?,
        program_bytes: reader.u32()?,
        bytecode_version: BytecodeVersion::new(reader.u16()?, reader.u16()?),
        bytecode_flags: reader.u32()?,
        configuration_sha256: reader.digest()?,
        retain_schema_sha256: reader.digest()?,
    })
}

fn read_contracts(reader: &mut Reader<'_>) -> Result<Contracts, Error> {
    let scheduling_profile = SchedulingProfile(reader.u16()?);
    let numeric_contract = NumericContract(reader.u16()?);
    let timing_evidence = TimingEvidence(reader.u8()?);
    if reader.u8()? != 0 {
        return Err(Error::Reserved);
    }
    Ok(Contracts {
        scheduling_profile,
        numeric_contract,
        timing_evidence,
        capabilities: Capabilities(reader.u16()?),
    })
}

fn read_limits(reader: &mut Reader<'_>) -> Result<RequestedLimits, Error> {
    let mut values = [0u64; LimitKind::ALL.len()];
    for value in &mut values {
        *value = reader.u64()?;
    }
    Ok(RequestedLimits::from_values(values))
}

fn read_process_image(reader: &mut Reader<'_>) -> Result<ProcessImageSummary, Error> {
    Ok(ProcessImageSummary {
        input_bytes: reader.u32()?,
        output_bytes: reader.u32()?,
        marker_bytes: reader.u32()?,
        bindings: reader.u32()?,
    })
}

fn read_tasks(reader: &mut Reader<'_>) -> Result<Vec<TaskSummary>, Error> {
    let count = usize::from(reader.u16()?);
    if reader.u16()? != 0 {
        return Err(Error::Reserved);
    }
    // The count must match the payload before anything is reserved for it.
    if count.saturating_mul(TASK_BYTES) != reader.remaining() {
        return Err(Error::Truncated);
    }
    let mut tasks = Vec::new();
    tasks.try_reserve_exact(count).map_err(|_| Error::Memory)?;
    for _ in 0..count {
        let interval_nanos = reader.i64()?;
        let priority = reader.u32()?;
        let flags = reader.u32()?;
        if flags & !TASK_EVENT != 0 {
            return Err(Error::Field("a task flag"));
        }
        tasks.push(TaskSummary {
            interval_nanos,
            priority,
            event: flags & TASK_EVENT != 0,
        });
    }
    Ok(tasks)
}

fn read_imports(reader: &mut Reader<'_>) -> Result<Vec<RequiredImport>, Error> {
    let count = usize::from(reader.u16()?);
    if reader.u16()? != 0 {
        return Err(Error::Reserved);
    }
    // Every import takes at least its header and one byte of name.
    if count.saturating_mul(IMPORT_HEADER_BYTES + 1) > reader.remaining() {
        return Err(Error::Truncated);
    }
    let mut imports = Vec::new();
    imports
        .try_reserve_exact(count)
        .map_err(|_| Error::Memory)?;
    for _ in 0..count {
        let kind = match reader.u8()? {
            1 => ImportKind::Function,
            2 => ImportKind::FunctionBlock,
            _ => return Err(Error::Field("an import kind")),
        };
        let name_length = usize::from(reader.u8()?);
        let contract_version = reader.u16()?;
        let name = core::str::from_utf8(reader.take(name_length)?)
            .ok()
            .filter(|name| !name.is_empty())
            .ok_or(Error::Field("an import name"))?;
        imports.push(RequiredImport {
            kind,
            name: SmolStr::new(name),
            contract_version,
        });
    }
    check_import_order(&imports)?;
    Ok(imports)
}

/// Imports are ordered by kind and then by the bytes of their name, each once.
fn check_import_order(imports: &[RequiredImport]) -> Result<(), Error> {
    if imports.windows(2).all(|pair| {
        (pair[0].kind, pair[0].name.as_bytes()) < (pair[1].kind, pair[1].name.as_bytes())
    }) {
        Ok(())
    } else {
        Err(Error::Field("the import order"))
    }
}

/// Little-endian reads that fail with `Truncated` instead of reading past the end.
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len()
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        if count > self.bytes.len() {
            return Err(Error::Truncated);
        }
        let (head, tail) = self.bytes.split_at(count);
        self.bytes = tail;
        Ok(head)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    pub(super) fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, Error> {
        self.array().map(u16::from_le_bytes)
    }

    pub(super) fn u32(&mut self) -> Result<u32, Error> {
        self.array().map(u32::from_le_bytes)
    }

    pub(super) fn u64(&mut self) -> Result<u64, Error> {
        self.array().map(u64::from_le_bytes)
    }

    pub(super) fn i64(&mut self) -> Result<i64, Error> {
        self.array().map(i64::from_le_bytes)
    }

    pub(super) fn digest(&mut self) -> Result<[u8; 32], Error> {
        self.array()
    }
}
