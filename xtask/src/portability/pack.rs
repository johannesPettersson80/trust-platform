//! `xtask portability pack-project` (specification 34 §5.1): the bench packer that builds the
//! `TRSTA001` container from `program.stbc` and `deployment-record.bin`.
//!
//! Each wire format has one owner: the chip's own `trust_runtime_services::installation` writes
//! the container, and the core's `DeploymentRecord` reads the record. Before packing, the packer
//! checks that the record names exactly the program it packs, so a container never pairs a record
//! with another program.

use anyhow::{ensure, Context, Result};
use trust_runtime_core::deployment::{sha256, DeploymentRecord};
use trust_runtime_services::installation;

/// Build the `TRSTA001` container of `program` and `record`.
///
/// The record is read by the core's own decoder, which refuses a record it cannot read
/// (specification 34 §5.1.1). The container is built by the chip's own writer,
/// `installation::encode`, so the bench writes exactly the bytes a chip checks.
///
/// # Errors
///
/// The record does not read, or it describes a program other than the one being packed, by
/// length or by SHA-256.
pub(super) fn pack_project(program: &[u8], record: &[u8]) -> Result<Vec<u8>> {
    let named = DeploymentRecord::decode(record).context("read the deployment record")?;
    ensure!(
        usize::try_from(named.application.program_bytes)? == program.len(),
        "the record describes a program of {} bytes, this program has {}",
        named.application.program_bytes,
        program.len()
    );
    ensure!(
        named.application.program_sha256 == sha256(program),
        "the record describes another program, not this program.stbc"
    );
    let container = installation::encode(program, record)?;
    Ok(container)
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime_core::bytecode::BytecodeVersion;
    use trust_runtime_core::deployment::{
        ApplicationIdentity, Capabilities, Configuration, Contracts, NumericContract,
        ProcessImageSummary, RequestedLimits, SchedulingProfile, TimingEvidence,
    };
    use trust_runtime_core::vm::PreparationLimits;

    /// SHA-256 of `abc` (FIPS 180-4 test vector).
    const ABC_SHA256: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];

    /// A valid version-1 record that names the program of `sha256` and `bytes`, written by the
    /// core's own record writer (specification 34 §5.1.1).
    fn record_of(program_sha256: [u8; 32], program_bytes: u32) -> Vec<u8> {
        DeploymentRecord {
            application: ApplicationIdentity {
                program_sha256,
                program_bytes,
                bytecode_version: BytecodeVersion::SOURCE_FREE,
                bytecode_flags: 0,
                configuration_sha256: [0; 32],
                retain_schema_sha256: [0; 32],
            },
            contracts: Contracts {
                scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
                numeric_contract: NumericContract::V1,
                timing_evidence: TimingEvidence::UNQUALIFIED,
                capabilities: Capabilities::NONE,
            },
            limits: RequestedLimits::from_preparation(&PreparationLimits::default()),
            process_image: ProcessImageSummary::default(),
            tasks: Vec::new(),
            imports: Vec::new(),
            configuration: Configuration::default(),
        }
        .encode()
        .expect("a small record encodes")
    }

    #[test]
    fn pack_project_refuses_a_record_of_another_program() {
        let program = b"abc";
        let matching = record_of(ABC_SHA256, 3);
        let container =
            pack_project(program, &matching).expect("the record describes this program");
        let installed = installation::decode(&container).expect("the packed container decodes");
        assert_eq!(installed.program, program);
        assert_eq!(installed.record, matching.as_slice());

        // The record of another program, by SHA-256 or by length, is refused.
        for other in [record_of([0x22; 32], 3), record_of(ABC_SHA256, 4)] {
            assert!(
                pack_project(program, &other).is_err(),
                "a record of another program must be refused"
            );
        }
        // A damaged record is refused before it is packed.
        let mut damaged = matching;
        let middle = damaged.len() / 2;
        damaged[middle] ^= 1;
        assert!(pack_project(program, &damaged).is_err());
    }
}
