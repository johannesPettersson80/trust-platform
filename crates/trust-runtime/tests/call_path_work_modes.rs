//! Charged work is the same in every execution mode: canonical stack, register, tier-1 and in-place calls.
#![cfg(feature = "register-ir")]
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration};
use trust_runtime_core::vm::register::RegisterExecutionConfiguration;
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

const NESTED_CALLS_SOURCE: &str = r#"
VAR_GLOBAL in_divisor : INT := INT#1; fb_divisor : INT := INT#1; END_VAR
FUNCTION Scale : INT
VAR_IN_OUT value : INT; END_VAR
VAR_INPUT increment : INT := INT#1; END_VAR
value := value + increment;
Scale := value / in_divisor;
END_FUNCTION
FUNCTION_BLOCK Edge
VAR_INPUT pulse : BOOL R_EDGE; END_VAR
VAR_OUTPUT seen : INT; END_VAR
IF pulse THEN seen := seen + INT#1; END_IF;
seen := seen / fb_divisor;
END_FUNCTION_BLOCK
FUNCTION Outer : INT
VAR local : INT := INT#2; END_VAR
Outer := Scale(value := local) + local;
END_FUNCTION
PROGRAM Main
VAR count : INT := INT#5; scaled : INT; edge : Edge; seen : INT; answer : INT; END_VAR
scaled := Scale(value := count);
edge(pulse := TRUE, seen => seen);
answer := Outer();
END_PROGRAM
"#;

/// Execution mode of a comparison run, selected through the platform's register configuration.
struct Mode {
    /// A debugger is attached, so the canonical stack path runs.
    debug: bool,
    /// `Some(capacity)` enables tier-1 with that selection capacity and a hot threshold of one block hit.
    tier1_capacity: Option<usize>,
}

impl ExecutionServices for Mode {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        RegisterExecutionConfiguration {
            profile_enabled: true,
            tier1_enabled: self.tier1_capacity.is_some(),
            tier1_hot_threshold: 1,
            tier1_capacity: self.tier1_capacity.unwrap_or(1),
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
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Smallest per-cycle work allowance that admits `cycles` cycles of `bytes` run in `services`.
fn minimum_work(bytes: &[u8], services: &dyn ExecutionServices, cycles: i64) -> usize {
    let admitted = |max_work: usize| {
        let limits = PreparationLimits {
            max_work,
            ..PreparationLimits::default()
        };
        let Ok(prepared) = PreparedModule::from_bytes(bytes, limits) else {
            return false;
        };
        let Ok(mut state) = prepared.instantiate_with_services(0, services) else {
            return false;
        };
        (1..=cycles).all(|tick| {
            state
                .execute_cycle(Duration::from_millis(tick * 10))
                .is_ok()
        })
    };
    let (mut low, mut high) = (1usize, PreparationLimits::default().max_work);
    assert!(admitted(high), "the default allowance admits the program");
    while low < high {
        let middle = low + (high - low) / 2;
        if admitted(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn encoded(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap()
}

/// Spec 34 §4.3 HOST-04 "Calls" and bookkeeping under TIME-08: the canonical stack, register and tier-1 paths
/// consume the same work for the same cycles, so no optimization adds a parallel charge.
#[test]
fn call_path_charged_work_is_equal_in_debug_register_and_tier1_modes() {
    let bytes = encoded(NESTED_CALLS_SOURCE);
    let stack = minimum_work(
        &bytes,
        &Mode {
            debug: true,
            tier1_capacity: None,
        },
        24,
    );
    let register = minimum_work(
        &bytes,
        &Mode {
            debug: false,
            tier1_capacity: None,
        },
        24,
    );
    let tier1 = minimum_work(
        &bytes,
        &Mode {
            debug: false,
            tier1_capacity: Some(1024),
        },
        24,
    );
    assert_eq!(
        register, stack,
        "register path charges the stack path's work"
    );
    assert_eq!(tier1, stack, "tier-1 path charges the stack path's work");
}

/// Spec 34 §4.3 HOST-04 constant-time selection: selection bookkeeping that churns on every block execution
/// adds no charged work to the canonical stack path.
#[test]
fn call_path_tier1_selection_churn_adds_no_charged_work() {
    let bytes = encoded(NESTED_CALLS_SOURCE);
    let stack = minimum_work(
        &bytes,
        &Mode {
            debug: true,
            tier1_capacity: None,
        },
        24,
    );
    let churning = minimum_work(
        &bytes,
        &Mode {
            debug: false,
            tier1_capacity: Some(1),
        },
        24,
    );
    assert_eq!(
        churning, stack,
        "a one-entry tier-1 selection churns but charges the stack path's work"
    );
}

/// Spec 34 §4.3 HOST-04 constant-time selection: the charged work does not depend on the configured
/// tier-1 selection capacity.
#[test]
fn call_path_charged_work_does_not_depend_on_the_tier1_cache_capacity() {
    let bytes = encoded(NESTED_CALLS_SOURCE);
    let small = minimum_work(
        &bytes,
        &Mode {
            debug: false,
            tier1_capacity: Some(1),
        },
        24,
    );
    let large = minimum_work(
        &bytes,
        &Mode {
            debug: false,
            tier1_capacity: Some(1024),
        },
        24,
    );
    assert_eq!(
        small, large,
        "tier-1 capacity 1 and 1024 charge the same work"
    );
}
