use super::*;
use crate::value::DateTimeValue;
use crate::vm::{
    register_ir::RegisterExecutionConfiguration, ExecutionServices, PreparationLimits,
    PreparedModule,
};

struct Configured(RegisterExecutionConfiguration);

impl ExecutionServices for Configured {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        self.0
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

const FIXTURE: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
);

/// Spec 34 §4.3 HOST-04 "Construction-time scratch": the scratch reserved for eligible POUs counts against the
/// construction allocation limit. Without the scratch demand the construction total would fit the limit.
#[test]
fn call_path_scratch_demand_counts_against_the_construction_limit() {
    let prepared = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
    let services = Configured(Default::default());
    let total = EngineState::construct_data(&prepared, 0, &services)
        .unwrap()
        .resources
        .construction_bytes
        .get();
    let mut scratch = 0usize;
    prepared
        .register_plans
        .reserve_execution_buffers(&prepared.vm, |bytes, _| {
            scratch += bytes;
            Ok(())
        })
        .unwrap();
    assert!(scratch > 0, "the fixture reserves scratch");
    let without_scratch = PreparationLimits {
        max_construction_bytes: total - scratch,
        ..PreparationLimits::default()
    };
    let constrained = PreparedModule::from_bytes(FIXTURE, without_scratch).unwrap();
    assert!(matches!(
        EngineState::construct_data(&constrained, 0, &services),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    let exact = PreparationLimits {
        max_construction_bytes: total,
        ..PreparationLimits::default()
    };
    let admitted = PreparedModule::from_bytes(FIXTURE, exact).unwrap();
    assert!(EngineState::construct_data(&admitted, 0, &services).is_ok());
}

/// Spec 34 §4.3 HOST-04 "Construction-time scratch": scratch is reserved only for eligible (lowered, fallback-free)
/// POUs.
#[test]
fn call_path_only_eligible_pous_hold_scratch_buffers() {
    let prepared = PreparedModule::from_bytes(FIXTURE, PreparationLimits::default()).unwrap();
    let services = Configured(Default::default());
    let data = EngineState::construct_data(&prepared, 0, &services).unwrap();
    assert!(!data.resources.register.buffers.is_empty());
    for (id, buffers) in &data.resources.register.buffers {
        assert!(buffers.is_some(), "POU {id} entry holds its scratch");
        let plan = prepared
            .register_plans
            .get(*id)
            .expect("scratch belongs to a prepared plan");
        assert_eq!(
            plan.fallback_reason(),
            None,
            "POU {id} holds scratch although its plan has a fallback"
        );
    }
}
