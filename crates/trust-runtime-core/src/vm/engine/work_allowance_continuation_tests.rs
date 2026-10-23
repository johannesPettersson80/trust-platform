//! TIME-08: every execution backend charges the same work total for the same nested-call execution.
use super::*;

#[cfg(feature = "register-ir")]
#[test]
fn nested_call_work_total_is_equal_on_the_stack_register_and_tier1_backends() {
    use crate::error::RuntimeError;
    use crate::vm::register_ir::RegisterExecutionConfiguration;

    struct Mode {
        debug: bool,
        tier1: bool,
    }
    impl services::ExecutionServices for Mode {
        fn register_configuration(&self) -> RegisterExecutionConfiguration {
            RegisterExecutionConfiguration {
                profile_enabled: true,
                tier1_enabled: self.tier1,
                tier1_hot_threshold: 1,
                ..RegisterExecutionConfiguration::default()
            }
        }

        fn debug_active(&self) -> bool {
            self.debug
        }

        fn deadline_exceeded(&self) -> bool {
            false
        }

        fn has_wall_clock(&self) -> bool {
            false
        }

        fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
            Err(RuntimeError::ProfileUnsupported("clock".into()))
        }
    }

    /// Runs the admitted nested-call fixture once under the default allowance and returns
    /// the work the run consumed.
    fn consumed(prepared: &PreparedModule, mode: &Mode) -> usize {
        let mut data = EngineState::construct_data(prepared, 0, mode).unwrap();
        let mut state = EngineState::borrow(&mut data, prepared, mode);
        state.data.storage.set_global("probe_depth", Value::Int(2));
        let full = prepared.limits.max_work;
        state.data.resources.work_budget.reset(full);
        let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
            panic!("program root")
        };
        let plant = *plant;
        let pou = *state
            .data
            .construction
            .instance_templates
            .get(&plant)
            .expect("Plant has an admitted program template");
        state
            .execute_pou(pou, Some(plant))
            .expect("the admitted nested-call path fits the default allowance");
        full - state.data.resources.work_budget.remaining()
    }

    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let stack = consumed(
        &prepared,
        &Mode {
            debug: true,
            tier1: false,
        },
    );
    let register = consumed(
        &prepared,
        &Mode {
            debug: false,
            tier1: false,
        },
    );
    let tier1 = consumed(
        &prepared,
        &Mode {
            debug: false,
            tier1: true,
        },
    );

    assert!(stack > 1, "the nested fixture must consume work");
    // TIME-08: "every execution backend charges the same total for the same execution".
    assert_eq!(
        register, stack,
        "the register backend charges the stack total"
    );
    assert_eq!(tier1, stack, "the tier-1 backend charges the stack total");
}
