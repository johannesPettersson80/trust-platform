//! Explicit hosted entries select admitted code; declaration ASTs never execute here.
use super::Runtime;
use crate::{error::RuntimeError, task::ProgramDef};

impl Runtime {
    /// Execute the installed program selected by this declaration's name.
    /// The argument does not replace the admitted body, types or storage layout.
    pub fn execute_program(&mut self, program: &ProgramDef) -> Result<(), RuntimeError> {
        self.state.execute_program(&program.name)
    }

    /// Construct, execute and retire one temporary admitted FB instance.
    pub fn execute_function_block_by_name(&mut self, name: &str) -> Result<(), RuntimeError> {
        self.state.execute_function_block_by_name(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{harness::CompileSession, value::Value};

    #[test]
    fn explicit_fb_entry_initializes_each_call_and_retires_nested_instances() {
        let source = "VAR_GLOBAL calls : DINT; END_VAR FUNCTION_BLOCK Child VAR value : INT := INT#3; END_VAR ASSERT_EQUAL(INT#3, value); value := INT#9; END_FUNCTION_BLOCK TEST_FUNCTION_BLOCK Probe VAR nested : Child; END_VAR nested(); calls := calls + DINT#1; END_TEST_FUNCTION_BLOCK PROGRAM Main END_PROGRAM";
        let mut runtime = CompileSession::from_source(source).build_runtime().unwrap();
        let instances = runtime.storage().instances().len();
        let globals = runtime.storage().globals().len();
        for expected in 1..=3 {
            runtime.execute_function_block_by_name("pRoBe").unwrap();
            assert_eq!(
                runtime.storage().get_global("calls"),
                Some(&Value::DInt(expected))
            );
            assert_eq!(runtime.storage().instances().len(), instances);
            assert_eq!(runtime.storage().globals().len(), globals);
            assert_eq!(runtime.storage().execution_frame_count(), 0);
        }
    }

    #[test]
    fn explicit_fb_fault_and_budget_exhaustion_do_not_leak_instances_or_fault_controller() {
        let source = "FUNCTION_BLOCK Child VAR value : INT := INT#3; END_VAR END_FUNCTION_BLOCK TEST_FUNCTION_BLOCK Fails VAR nested : Child; END_VAR ASSERT_TRUE(FALSE); END_TEST_FUNCTION_BLOCK TEST_FUNCTION_BLOCK Loops VAR nested : Child; END_VAR WHILE TRUE DO END_WHILE; END_TEST_FUNCTION_BLOCK PROGRAM Main END_PROGRAM";
        let mut runtime = CompileSession::from_source(source).build_runtime().unwrap();
        let instances = runtime.storage().instances().len();
        for name in ["Fails", "Loops", "Fails"] {
            let error = runtime.execute_function_block_by_name(name).unwrap_err();
            if name == "Loops" {
                assert!(matches!(
                    error,
                    RuntimeError::ExecutionLimit {
                        kind: trust_runtime_core::error::ExecutionLimitKind::Work,
                        used, requested, limit,
                    } if used <= limit && requested > limit - used
                ));
            } else {
                assert!(matches!(error, RuntimeError::AssertionFailed(_)));
            }
            assert_eq!(runtime.storage().instances().len(), instances);
            assert_eq!(runtime.storage().execution_frame_count(), 0);
            assert!(!runtime.faulted());
        }
    }

    #[test]
    fn program_argument_cannot_replace_admitted_body() {
        let mut runtime = CompileSession::from_source(
            "VAR_GLOBAL count : INT; END_VAR PROGRAM Main count := count + INT#1; END_PROGRAM",
        )
        .build_runtime()
        .unwrap();
        let mut descriptor = runtime.programs().values().next().unwrap().clone();
        descriptor.body.clear();
        runtime.execute_program(&descriptor).unwrap();
        assert_eq!(runtime.storage().get_global("count"), Some(&Value::Int(1)));
    }
}
