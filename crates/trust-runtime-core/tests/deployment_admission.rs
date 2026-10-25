//! Admission of a deployment record (spec 34 §5.1.1, DEPLOY-04, LOAD-07, LOAD-09; spec 44 §10.1
//! and §10.2, step 1): the checks run in the specified order before any state exists, each
//! refusal names its numbers, and admission reads the program only as the bytes the record
//! describes.

use trust_runtime_core::deployment::{
    admit, sha256, AdmissionRefusal, ApplicationIdentity, Capabilities, Configuration, Contracts,
    DeploymentRecord, DeviceDescriptor, ImportKind, LimitKind, NativeTarget, NumericContract,
    ProcessImageSummary, ProgramVersion, RequestedLimits, RequiredImport, SchedulingProfile,
    TimingEvidence,
};
use trust_runtime_core::error::{ImportRefusal, RuntimeError, StableErrorCode};
use trust_runtime_core::native::image::Target;

/// The bytes a record describes. Admission compares them with the record's length and SHA-256 and
/// never reads them as a program, so they need not be an image.
const PROGRAM: &[u8] = b"the bytes of one program image";
/// The bytes of another program, of another length.
const OTHER: &[u8] = b"the bytes of another program image";

/// The program versions of a device that runs compiled program images (spec 44 §10.1).
const IMAGES: &[ProgramVersion] = &[ProgramVersion::NATIVE_IMAGE];

/// A computer that runs program images, whatever CPU runs the test.
fn computer() -> DeviceDescriptor {
    DeviceDescriptor {
        program_versions: IMAGES,
        ..DeviceDescriptor::hosted()
    }
}

/// The record of `program` as spec 44 §10.1 describes an image's: its SHA-256 and length, the
/// version 44.1 with no flags, `cooperative-resource-v1`, numeric contract 1, no timing evidence, no
/// capability, the bytes of its process image (one input, one output and one marker byte) as its
/// only request, and no import.
fn record_of(program: &[u8]) -> DeploymentRecord {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: sha256(program),
            program_bytes: u32::try_from(program.len()).unwrap(),
            program_version: ProgramVersion::NATIVE_IMAGE,
            program_flags: 0,
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
            max_process_image_bytes: 3,
            ..limits_of(0)
        },
        process_image: ProcessImageSummary {
            input_bytes: 1,
            output_bytes: 1,
            marker_bytes: 1,
            bindings: 3,
        },
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration: Configuration::default(),
    }
}

/// Every limit at `value`.
fn limits_of(value: u64) -> RequestedLimits {
    RequestedLimits {
        max_artifact_bytes: value,
        max_preparation_bytes: value,
        max_preparation_work: value,
        max_validation_scratch_bytes: value,
        max_validation_work: value,
        max_construction_values: value,
        max_construction_bytes: value,
        max_process_image_bytes: value,
        max_work: value,
        max_construction_work: value,
        max_call_depth: value,
    }
}

fn refusal(result: Result<(), RuntimeError>) -> AdmissionRefusal {
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

/// Spec 34 HOST-02 and spec 44 §4.4 and NX-15: a computer has no maximum for any limit, keeps the
/// newest 1,000 events by default and admits at most 1,000,000, has no application slot and no
/// maximum of program memory, and runs program images of its own CPU with a task stack of 1 MiB and
/// the default budget of 1,000,000 statements per activation (NX-10); on a CPU no module targets
/// it runs no program.
#[test]
fn a_computer_runs_program_images_without_maxima() {
    let unbounded = u64::try_from(usize::MAX).unwrap();
    let native = Target::this_cpu().map(|target| NativeTarget {
        target,
        task_stack_bytes: 1_048_576,
        max_statements: 1_000_000,
    });
    assert_eq!(
        DeviceDescriptor::hosted(),
        DeviceDescriptor {
            program_versions: if native.is_some() { IMAGES } else { &[] },
            scheduling_profiles: &[SchedulingProfile::COOPERATIVE_RESOURCE_V1],
            numeric_contracts: &[NumericContract::V1],
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::WALL_CLOCK,
            maxima: limits_of(unbounded),
            event_store_default: 1_000,
            event_store_max: 1_000_000,
            application_slot_bytes: None,
            program_memory_bytes: None,
            trusted_keys: &[],
            native,
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
        ..computer()
    };
    let mut record = record_of(PROGRAM);
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
        Ok(()),
        "a capacity at the maximum is admitted"
    );
    record.configuration.event_capacity = 0;
    assert_eq!(
        admit(&record, PROGRAM, &device),
        Ok(()),
        "0 keeps the device's default"
    );
}

#[test]
fn a_record_admits_only_the_program_it_describes() {
    let record = record_of(PROGRAM);
    let device = computer();
    assert_eq!(admit(&record, PROGRAM, &device), Ok(()));
    assert_eq!(
        refusal(admit(&record, OTHER, &device)),
        AdmissionRefusal::ProgramMismatch
    );
    let mut same_length = PROGRAM.to_vec();
    let last = same_length.len() - 1;
    same_length[last] ^= 1;
    assert_eq!(
        refusal(admit(&record, &same_length, &device)),
        AdmissionRefusal::ProgramMismatch
    );
    let error = admit(&record, OTHER, &device).unwrap_err();
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
    let record = record_of(PROGRAM);
    let device = computer();

    // Spec 44 §10.1: a device whose descriptor does not name 44.1 refuses an image, and a device
    // that runs images refuses an STBC program with its version (§15: "rebuild the project").
    let no_images = DeviceDescriptor {
        program_versions: &[],
        ..computer()
    };
    let reason = refusal(admit(&record, PROGRAM, &no_images));
    assert_eq!(
        reason,
        AdmissionRefusal::ProgramVersion {
            major: 44,
            minor: 1
        }
    );
    assert_eq!(
        reason.to_string(),
        "the device does not run compiled program images"
    );
    let mut stbc = record.clone();
    stbc.application.program_version = ProgramVersion::new(3, 0);
    let reason = refusal(admit(&stbc, PROGRAM, &device));
    assert_eq!(
        reason,
        AdmissionRefusal::ProgramVersion { major: 3, minor: 0 }
    );
    assert_eq!(
        reason.to_string(),
        "the program is STBC 3.0, which this runtime does not run: rebuild the project"
    );

    let mut profile = record.clone();
    profile.contracts.scheduling_profile = SchedulingProfile(2);
    let reason = refusal(admit(&profile, PROGRAM, &device));
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
        refusal(admit(&numeric, PROGRAM, &device)),
        AdmissionRefusal::NumericContract(NumericContract(2))
    );

    let mut timing = record.clone();
    timing.contracts.timing_evidence = TimingEvidence::MEASURED_QUALIFIED;
    let reason = refusal(admit(&timing, PROGRAM, &device));
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

    // A program that reads the wall clock of CURRENT_DT needs it.
    let mut clock = record.clone();
    clock.contracts.capabilities = Capabilities::WALL_CLOCK;
    assert_eq!(admit(&clock, PROGRAM, &device), Ok(()));
    let without_clock = DeviceDescriptor {
        capabilities: Capabilities::NONE,
        ..computer()
    };
    let reason = refusal(admit(&clock, PROGRAM, &without_clock));
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
        refusal(admit(&unknown, PROGRAM, &device)).to_string(),
        "the device lacks capability bit 2"
    );
}

#[test]
fn every_import_needs_a_contract_of_the_same_version() {
    let record = record_of(PROGRAM);
    let device = computer();
    let with_import = |import: RequiredImport| {
        let mut changed = record.clone();
        changed.imports.push(import);
        admit(&changed, PROGRAM, &device)
    };
    assert_eq!(
        with_import(RequiredImport {
            kind: ImportKind::FunctionBlock,
            name: "TON".into(),
            contract_version: 1,
        }),
        Ok(())
    );
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
    let error = with_import(RequiredImport {
        kind: ImportKind::FunctionBlock,
        name: "TON".into(),
        contract_version: 2,
    })
    .unwrap_err();
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
    let mut record = record_of(PROGRAM);
    record.limits = limits_of(5);
    for (kind, requested) in record.limits.values() {
        assert_eq!(requested, 5, "{kind}");
        let mut device = computer();
        set(&mut device.maxima, kind, 4);
        assert_eq!(
            refusal(admit(&record, PROGRAM, &device)),
            AdmissionRefusal::Limit {
                limit: kind,
                requested: 5,
                maximum: 4,
            },
            "{kind}"
        );
        set(&mut device.maxima, kind, 5);
        assert_eq!(
            admit(&record, PROGRAM, &device),
            Ok(()),
            "{kind} at its maximum"
        );
    }
    let mut deep = record_of(PROGRAM);
    deep.limits.max_call_depth = 2048;
    let mut device = computer();
    device.maxima.max_call_depth = 1024;
    assert_eq!(
        admit(&deep, PROGRAM, &device).unwrap_err().to_string(),
        "deployment refused: the record requests max_call_depth 2048, the device allows at most \
         1024"
    );
}

#[test]
fn the_program_and_its_record_must_fit_the_application_slot() {
    let record = record_of(PROGRAM);
    let needed = u64::try_from(PROGRAM.len() + record.encoded_len()).unwrap();
    let device = DeviceDescriptor {
        application_slot_bytes: Some(needed - 1),
        ..computer()
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
        ..computer()
    };
    assert_eq!(admit(&record, PROGRAM, &exact), Ok(()));
}

#[test]
fn the_first_failed_check_in_the_specified_order_refuses() {
    let mut record = record_of(PROGRAM);
    record.contracts.capabilities = Capabilities::WALL_CLOCK;
    record.imports.push(RequiredImport {
        kind: ImportKind::FunctionBlock,
        name: "TON".into(),
        contract_version: 1,
    });
    // Break every check from 2 to 8 at once on a strict device, then repair them in order.
    let mut device = DeviceDescriptor {
        program_versions: &[],
        capabilities: Capabilities::NONE,
        application_slot_bytes: Some(1),
        ..computer()
    };
    device.maxima.max_call_depth = 1024;
    record.contracts.scheduling_profile = SchedulingProfile(9);
    record.contracts.numeric_contract = NumericContract(9);
    record.contracts.timing_evidence = TimingEvidence::ANALYTICALLY_SUPPORTED;
    record.imports[0].contract_version = 9;
    record.limits.max_call_depth = 4096;
    let check = |record: &DeploymentRecord| admit(record, PROGRAM, &device).unwrap_err();
    assert_eq!(
        check(&record),
        RuntimeError::DeploymentRefused(AdmissionRefusal::ProgramVersion {
            major: 44,
            minor: 1
        })
    );
    let device = DeviceDescriptor {
        program_versions: IMAGES,
        ..device
    };
    let check = |record: &DeploymentRecord| admit(record, PROGRAM, &device).unwrap_err();
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
    let check = |record: &DeploymentRecord| admit(record, PROGRAM, &device).unwrap_err();
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
    // The supplied bytes are no program image at all; the record describes exactly them.
    // Admission refuses on its own numbers, so it ran before the image was parsed, checked or
    // loaded.
    let program = b"not a program image";
    let mut record = record_of(program);
    record.limits.max_call_depth = 1_000_000;
    let mut device = computer();
    device.maxima.max_call_depth = 1024;
    assert!(matches!(
        admit(&record, program, &device),
        Err(RuntimeError::DeploymentRefused(AdmissionRefusal::Limit {
            limit: LimitKind::CallDepth,
            ..
        }))
    ));
}
