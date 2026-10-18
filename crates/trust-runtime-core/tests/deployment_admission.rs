//! Admission of a deployment record (spec 34 §5.1.1, DEPLOY-04, LOAD-07, LOAD-09): the checks
//! run in the specified order before any state exists, each refusal names its numbers, and an
//! admitted record prepares the program with exactly the limits it requests.

use trust_runtime_core::bytecode::{BytecodeVersion, BYTECODE_MAX_CONTAINER_BYTES};
use trust_runtime_core::deployment::{
    admit, AdmissionRefusal, ApplicationIdentity, Capabilities, Contracts, DeploymentRecord,
    DeviceDescriptor, ImportKind, LimitKind, NumericContract, ProcessImageSummary, RequestedLimits,
    RequiredImport, SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::error::{ImportRefusal, RuntimeError, StableErrorCode};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule, VM_MAX_CALL_DEPTH};

const PROGRAM: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const CLOCK: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/clock-v2.stbc");

fn record_of(program: &[u8], limits: PreparationLimits) -> DeploymentRecord {
    DeploymentRecord::for_program(program, limits, [0; 32])
        .expect("the saved fixture is admitted under these limits")
}

fn refusal(result: Result<PreparationLimits, RuntimeError>) -> AdmissionRefusal {
    match result {
        Err(RuntimeError::DeploymentRefused(reason)) => reason,
        other => panic!("expected a refused deployment, got {other:?}"),
    }
}

fn set(limits: &mut RequestedLimits, kind: LimitKind, value: u64) {
    let field = match kind {
        LimitKind::ArtifactBytes => &mut limits.max_artifact_bytes,
        LimitKind::PreparationBytes => &mut limits.max_preparation_bytes,
        LimitKind::PreparationWork => &mut limits.max_preparation_work,
        LimitKind::ValidationScratchBytes => &mut limits.max_validation_scratch_bytes,
        LimitKind::ValidationWork => &mut limits.max_validation_work,
        LimitKind::ConstructionValues => &mut limits.max_construction_values,
        LimitKind::ConstructionBytes => &mut limits.max_construction_bytes,
        LimitKind::ProcessImageBytes => &mut limits.max_process_image_bytes,
        LimitKind::Work => &mut limits.max_work,
        LimitKind::ConstructionWork => &mut limits.max_construction_work,
        LimitKind::CallDepth => &mut limits.max_call_depth,
    };
    *field = value;
}

#[test]
fn a_computer_runs_source_free_stbc_with_the_engine_maxima() {
    let unbounded = u64::try_from(usize::MAX).unwrap();
    assert_eq!(
        DeviceDescriptor::hosted(),
        DeviceDescriptor {
            bytecode_versions: &[
                BytecodeVersion::SOURCE_FREE_V2,
                BytecodeVersion::SOURCE_FREE
            ],
            scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
            numeric_contracts: &[NumericContract::V1],
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::WALL_CLOCK,
            maxima: RequestedLimits {
                max_artifact_bytes: u64::try_from(BYTECODE_MAX_CONTAINER_BYTES).unwrap(),
                max_preparation_bytes: unbounded,
                max_preparation_work: unbounded,
                max_validation_scratch_bytes: unbounded,
                max_validation_work: unbounded,
                max_construction_values: unbounded,
                max_construction_bytes: unbounded,
                max_process_image_bytes: unbounded,
                max_work: unbounded,
                max_construction_work: unbounded,
                max_call_depth: u64::try_from(VM_MAX_CALL_DEPTH).unwrap(),
            },
            event_store_default: 1_000,
            event_store_max: 1_000_000,
            application_slot_bytes: None,
        }
    );
}

#[test]
fn admission_refuses_an_event_capacity_above_the_device_maximum() {
    // Specification 34 §5.1.1, admission step 7: the record's section-7 event-store capacity,
    // when it is not 0, may not be above the device's maximum; 0 keeps the device's default.
    // Catches a check that is off by one, that measures the maximum in the wrong place, or that
    // treats 0 as a capacity above the maximum.
    let device = DeviceDescriptor {
        event_store_max: 200,
        ..DeviceDescriptor::hosted()
    };
    let mut record = record_of(PROGRAM, PreparationLimits::default());
    record.configuration.event_capacity = 201;
    let reason = refusal(admit(&record, PROGRAM, &device));
    assert_eq!(
        reason,
        AdmissionRefusal::EventStore {
            requested: 201,
            maximum: 200,
        }
    );
    assert_eq!(
        reason.to_string(),
        "the record requests an event store of 201 events, the device keeps at most 200"
    );
    record.configuration.event_capacity = 200;
    assert_eq!(
        admit(&record, PROGRAM, &device),
        Ok(PreparationLimits::default()),
        "a capacity at the maximum is admitted"
    );
    record.configuration.event_capacity = 0;
    assert_eq!(
        admit(&record, PROGRAM, &device),
        Ok(PreparationLimits::default()),
        "0 keeps the device's default"
    );
}

#[test]
fn an_admitted_record_prepares_the_program_with_exactly_its_limits() {
    let limits = PreparationLimits {
        max_call_depth: 8,
        max_work: 50_000,
        max_process_image_bytes: 64,
        ..PreparationLimits::default()
    };
    let record = record_of(PROGRAM, limits);
    let admitted = admit(&record, PROGRAM, &DeviceDescriptor::hosted()).unwrap();
    assert_eq!(admitted, limits);
    let prepared = PreparedModule::from_bytes(PROGRAM, admitted).unwrap();
    assert_eq!(prepared.limits(), limits);
    // Both saved fixtures, and the default request of `trust-runtime build`, fit a computer.
    for program in [PROGRAM, CLOCK] {
        let record = record_of(program, PreparationLimits::default());
        assert_eq!(
            admit(&record, program, &DeviceDescriptor::hosted()),
            Ok(PreparationLimits::default())
        );
    }
}

#[test]
fn a_record_admits_only_the_program_it_describes() {
    let record = record_of(PROGRAM, PreparationLimits::default());
    let device = DeviceDescriptor::hosted();
    assert_eq!(
        refusal(admit(&record, CLOCK, &device)),
        AdmissionRefusal::ProgramMismatch
    );
    let mut same_length = PROGRAM.to_vec();
    let last = same_length.len() - 1;
    same_length[last] ^= 1;
    assert_eq!(
        refusal(admit(&record, &same_length, &device)),
        AdmissionRefusal::ProgramMismatch
    );
    let error = admit(&record, CLOCK, &device).unwrap_err();
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeDeploymentRefused
    );
    assert_eq!(error.stable_code().as_str(), "runtime_deployment_refused");
    assert_eq!(
        error.to_string(),
        "deployment refused: the record describes another program"
    );
}

#[test]
fn each_contract_and_capability_is_checked_against_the_device() {
    let record = record_of(PROGRAM, PreparationLimits::default());
    let hosted = DeviceDescriptor::hosted();

    let three_only = DeviceDescriptor {
        bytecode_versions: &[BytecodeVersion::SOURCE_FREE],
        ..hosted.clone()
    };
    let reason = refusal(admit(&record, PROGRAM, &three_only));
    assert_eq!(
        reason,
        AdmissionRefusal::BytecodeVersion { major: 2, minor: 0 }
    );
    assert_eq!(reason.to_string(), "the device does not run STBC 2.0");

    let mut profile = record.clone();
    profile.contracts.scheduling_profile = SchedulingProfile(2);
    let reason = refusal(admit(&profile, PROGRAM, &hosted));
    assert_eq!(
        reason,
        AdmissionRefusal::SchedulingProfile(SchedulingProfile(2))
    );
    assert_eq!(
        reason.to_string(),
        "the device does not run the scheduling profile 2"
    );

    let mut numeric = record.clone();
    numeric.contracts.numeric_contract = NumericContract(2);
    assert_eq!(
        refusal(admit(&numeric, PROGRAM, &hosted)),
        AdmissionRefusal::NumericContract(NumericContract(2))
    );

    let mut timing = record.clone();
    timing.contracts.timing_evidence = TimingEvidence::MEASURED_QUALIFIED;
    let reason = refusal(admit(&timing, PROGRAM, &hosted));
    assert_eq!(
        reason,
        AdmissionRefusal::TimingEvidence {
            required: TimingEvidence::MEASURED_QUALIFIED,
            available: TimingEvidence::UNQUALIFIED,
        }
    );
    assert_eq!(
        reason.to_string(),
        "the record requires measured-qualified timing evidence, the device has unqualified"
    );

    // A clock program needs the wall clock that CURRENT_DT reads.
    let clock = record_of(CLOCK, PreparationLimits::default());
    let without_clock = DeviceDescriptor {
        capabilities: Capabilities::NONE,
        ..hosted.clone()
    };
    let reason = refusal(admit(&clock, CLOCK, &without_clock));
    assert_eq!(
        reason,
        AdmissionRefusal::Capability(Capabilities::WALL_CLOCK)
    );
    assert_eq!(
        reason.to_string(),
        "the device lacks the platform wall clock"
    );
    let mut unknown = record.clone();
    unknown.contracts.capabilities = Capabilities(0b100);
    assert_eq!(
        refusal(admit(&unknown, PROGRAM, &hosted)).to_string(),
        "the device lacks capability bit 2"
    );
}

#[test]
fn every_import_needs_a_contract_of_the_same_version() {
    let record = record_of(PROGRAM, PreparationLimits::default());
    let hosted = DeviceDescriptor::hosted();
    let with_import = |import: RequiredImport| {
        let mut changed = record.clone();
        changed.imports.push(import);
        admit(&changed, PROGRAM, &hosted)
    };
    assert_eq!(
        with_import(RequiredImport {
            kind: ImportKind::Function,
            name: "SQRTX".into(),
            contract_version: 1,
        }),
        Err(RuntimeError::ImportUnsupported {
            import: "SQRTX".into(),
            reason: ImportRefusal::NoContract,
        })
    );
    // A standard function block is not a function, and a function is not a block.
    assert_eq!(
        with_import(RequiredImport {
            kind: ImportKind::Function,
            name: "TON".into(),
            contract_version: 1,
        }),
        Err(RuntimeError::ImportUnsupported {
            import: "TON".into(),
            reason: ImportRefusal::NoContract,
        })
    );
    assert_eq!(
        with_import(RequiredImport {
            kind: ImportKind::FunctionBlock,
            name: "SQRT".into(),
            contract_version: 1,
        }),
        Err(RuntimeError::ImportUnsupported {
            import: "SQRT".into(),
            reason: ImportRefusal::NoContract,
        })
    );
    let mut version = record.clone();
    version.imports[1].contract_version = 2;
    let error = admit(&version, PROGRAM, &hosted).unwrap_err();
    assert_eq!(
        error,
        RuntimeError::ImportUnsupported {
            import: "TON".into(),
            reason: ImportRefusal::ContractVersion {
                record: 2,
                runtime: 1,
            },
        }
    );
    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeImportUnsupported
    );
    assert_eq!(
        error.to_string(),
        "unsupported import 'TON': the deployment record names contract version 2, the runtime \
         has version 1"
    );
}

#[test]
fn no_requested_limit_may_exceed_the_device_maximum() {
    let record = record_of(PROGRAM, PreparationLimits::default());
    for (kind, requested) in record.limits.values() {
        let mut device = DeviceDescriptor::hosted();
        set(&mut device.maxima, kind, requested - 1);
        assert_eq!(
            refusal(admit(&record, PROGRAM, &device)),
            AdmissionRefusal::Limit {
                limit: kind,
                requested,
                maximum: requested - 1,
            },
            "{kind}"
        );
        set(&mut device.maxima, kind, requested);
        assert_eq!(
            admit(&record, PROGRAM, &device),
            Ok(PreparationLimits::default()),
            "{kind} at its maximum"
        );
    }
    let mut deep = record.clone();
    deep.limits.max_call_depth = 2048;
    assert_eq!(
        admit(&deep, PROGRAM, &DeviceDescriptor::hosted())
            .unwrap_err()
            .to_string(),
        "deployment refused: the record requests max_call_depth 2048, the device allows at most \
         1024"
    );
}

#[test]
fn the_program_and_its_record_must_fit_the_application_slot() {
    let record = record_of(PROGRAM, PreparationLimits::default());
    let needed = u64::try_from(PROGRAM.len() + record.encoded_len()).unwrap();
    let device = DeviceDescriptor {
        application_slot_bytes: Some(needed - 1),
        ..DeviceDescriptor::hosted()
    };
    let reason = refusal(admit(&record, PROGRAM, &device));
    assert_eq!(
        reason,
        AdmissionRefusal::ApplicationSlot {
            needed,
            available: needed - 1,
        }
    );
    assert_eq!(
        reason.to_string(),
        format!(
            "the program and its record need {needed} bytes of the application slot, the device \
             has {}",
            needed - 1
        )
    );
    let exact = DeviceDescriptor {
        application_slot_bytes: Some(needed),
        ..DeviceDescriptor::hosted()
    };
    assert_eq!(
        admit(&record, PROGRAM, &exact),
        Ok(PreparationLimits::default())
    );
}

#[test]
fn the_first_failed_check_in_the_specified_order_refuses() {
    let mut record = record_of(CLOCK, PreparationLimits::default());
    // Break every check from 2 to 8 at once on a strict device, then repair them in order.
    let device = DeviceDescriptor {
        bytecode_versions: &[BytecodeVersion::SOURCE_FREE],
        capabilities: Capabilities::NONE,
        application_slot_bytes: Some(1),
        ..DeviceDescriptor::hosted()
    };
    record.contracts.scheduling_profile = SchedulingProfile(9);
    record.contracts.numeric_contract = NumericContract(9);
    record.contracts.timing_evidence = TimingEvidence::ANALYTICALLY_SUPPORTED;
    record.imports[0].contract_version = 9;
    record.limits.max_call_depth = 4096;
    let check = |record: &DeploymentRecord| admit(record, CLOCK, &device).unwrap_err();
    assert_eq!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::BytecodeVersion { major: 2, minor: 0 })
    );
    let device = DeviceDescriptor {
        bytecode_versions: &[BytecodeVersion::SOURCE_FREE_V2],
        ..device
    };
    let check = |record: &DeploymentRecord| admit(record, CLOCK, &device).unwrap_err();
    assert_eq!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::SchedulingProfile(SchedulingProfile(9)))
    );
    record.contracts.scheduling_profile = SchedulingProfile::COOPERATIVE_RESOURCE_V1;
    assert_eq!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::NumericContract(NumericContract(9)))
    );
    record.contracts.numeric_contract = NumericContract::V1;
    assert!(matches!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::TimingEvidence { .. })
    ));
    record.contracts.timing_evidence = TimingEvidence::UNQUALIFIED;
    assert_eq!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::Capability(Capabilities::WALL_CLOCK))
    );
    let device = DeviceDescriptor {
        capabilities: Capabilities::WALL_CLOCK,
        ..device
    };
    let check = |record: &DeploymentRecord| admit(record, CLOCK, &device).unwrap_err();
    assert!(matches!(
        check(&record),
        RuntimeError::ImportUnsupported {
            reason: ImportRefusal::ContractVersion { record: 9, .. },
            ..
        }
    ));
    record.imports[0].contract_version = 1;
    assert!(matches!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::Limit {
            limit: LimitKind::CallDepth,
            requested: 4096,
            ..
        })
    ));
    record.limits.max_call_depth = 1024;
    assert!(matches!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::ApplicationSlot { available: 1, .. })
    ));
}

#[test]
fn admission_refuses_before_the_program_is_read() {
    // The supplied bytes are no STBC at all; the record describes exactly them. Admission
    // refuses on its own numbers, so it ran before any decoding, preparation or construction.
    let program = b"not an STBC artifact";
    let record = DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: trust_runtime_core::deployment::sha256(program),
            program_bytes: u32::try_from(program.len()).unwrap(),
            bytecode_version: BytecodeVersion::SOURCE_FREE,
            bytecode_flags: 1,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits {
            max_call_depth: 1_000_000,
            ..RequestedLimits::from_preparation(&PreparationLimits::default())
        },
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration: trust_runtime_core::deployment::Configuration::default(),
    };
    assert!(matches!(
        admit(&record, program, &DeviceDescriptor::hosted()),
        Err(RuntimeError::DeploymentRefused(AdmissionRefusal::Limit {
            limit: LimitKind::CallDepth,
            ..
        }))
    ));
}
