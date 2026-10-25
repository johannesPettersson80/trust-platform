//! The deployment record of a compiled program image (specification 44 §10.1, specification 34
//! §5.1.1): what the program store, admission and a device's loader read of an image, taken from the
//! image's own tables.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_plc_abi as abi;

use super::image::{
    Image, IoBindRecord, RetainRecord, TaskRecord, TypeRecord, BACKGROUND_PRIORITY, NONE,
};
use crate::deployment::{
    sha256, ApplicationIdentity, Capabilities, Configuration, Contracts, DeploymentRecord,
    NumericContract, ProcessImageSummary, ProgramVersion, RequestedLimits, SchedulingProfile,
    TaskSummary, TimingEvidence,
};
use crate::error::RuntimeError;

impl DeploymentRecord {
    /// The record of the compiled program image `image`, built with the configuration whose
    /// identity is `configuration_sha256` (`configuration_identity`).
    ///
    /// The image is checked as a device checks it before it is described (specification 44 §10.2,
    /// steps 2, 3 and 5), so a record exists only for an image whose tables read. The record names
    /// the image by its SHA-256 and length and by the version [`ProgramVersion::NATIVE_IMAGE`]; its
    /// retain-schema identity is SHA-256 over every retained variable in the retain map's order, as
    /// its canonical name, a zero byte, and its type's kind, size and count as little-endian words;
    /// its process image and tasks are the image's; it requires `cooperative-resource-v1`, numeric
    /// contract 1, no timing evidence and, for a program that reads the wall clock (the header's
    /// `FLAG_WALL_CLOCK`), the platform wall clock; it requests no preparation limit, since an
    /// image is not prepared; it calls no built-in import, since the program library is linked into
    /// the module; and its configuration section is empty for the caller to fill.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidBundle`] naming the first check the image fails.
    pub fn for_image(image: &[u8], configuration_sha256: [u8; 32]) -> Result<Self, RuntimeError> {
        let invalid = |error: super::image::ImageError| {
            RuntimeError::InvalidBundle(SmolStr::new(alloc::format!("{error}")))
        };
        let parsed = Image::parse(image).map_err(invalid)?;
        parsed.check_tables().map_err(invalid)?;
        let area = |id: u8| -> Result<u32, RuntimeError> {
            u32::try_from(parsed.area_length(id).unwrap_or(0)).map_err(|_| RuntimeError::Overflow)
        };
        let tasks = parsed
            .table::<TaskRecord>()
            .map_err(invalid)?
            .iter()
            .filter(|task| task.priority != BACKGROUND_PRIORITY)
            .map(|task| TaskSummary {
                interval_nanos: i64::try_from(task.interval_ns).unwrap_or(i64::MAX),
                priority: task.priority,
                event: task.single != NONE,
            })
            .collect();
        let process_image = ProcessImageSummary {
            input_bytes: area(abi::AREA_INPUTS)?,
            output_bytes: area(abi::AREA_OUTPUTS)?,
            marker_bytes: area(abi::AREA_MARKERS)?,
            bindings: parsed.table::<IoBindRecord>().map_err(invalid)?.len(),
        };
        // The record requests the process image the program binds, which a device bounds by its
        // `max_process_image_bytes` (specification 44 §10.2, steps 1 and 5); the other limits
        // named the bytecode engine's preparation.
        let mut limits = RequestedLimits::from_values([0; 11]);
        limits.max_process_image_bytes = u64::from(process_image.input_bytes)
            + u64::from(process_image.output_bytes)
            + u64::from(process_image.marker_bytes);
        Ok(Self {
            application: ApplicationIdentity {
                program_sha256: sha256(image),
                program_bytes: u32::try_from(image.len()).map_err(|_| RuntimeError::Overflow)?,
                program_version: ProgramVersion::NATIVE_IMAGE,
                program_flags: 0,
                configuration_sha256,
                retain_schema_sha256: retain_schema(&parsed).map_err(invalid)?,
            },
            contracts: Contracts {
                scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
                numeric_contract: NumericContract::V1,
                timing_evidence: TimingEvidence::UNQUALIFIED,
                // A program that reads the wall clock needs a device that has one (specification
                // 34 §5.2).
                capabilities: if parsed.header().reads_wall_clock() {
                    Capabilities::WALL_CLOCK
                } else {
                    Capabilities::NONE
                },
            },
            limits,
            process_image,
            tasks,
            imports: Vec::new(),
            configuration: Configuration::default(),
        })
    }
}

/// The retain-schema identity of `image`: SHA-256 over each retained variable in the retain map's
/// order, as its checkpoint key in capitals, a zero byte, and its type's kind, size and count.
fn retain_schema(image: &Image<'_>) -> Result<[u8; 32], super::image::ImageError> {
    let types = image.table::<TypeRecord>()?;
    let mut schema = Vec::new();
    for retained in image.table::<RetainRecord>()?.iter() {
        let key = image.string(retained.key).unwrap_or("");
        schema.extend_from_slice(key.to_ascii_uppercase().as_bytes());
        schema.push(0);
        let ty = types.get(retained.type_index);
        for word in [
            ty.map_or(0, |ty| ty.kind),
            ty.map_or(0, |ty| ty.size),
            ty.map_or(0, |ty| ty.count),
        ] {
            schema.extend_from_slice(&word.to_le_bytes());
        }
    }
    Ok(sha256(&schema))
}

#[cfg(test)]
mod tests;
