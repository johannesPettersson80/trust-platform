//! Private historical stack/register fixture, never a product execution composition.
use super::LegacyRuntimeFixture;
use crate::{
    harness::{CompileError, CompileSession, CycleResult},
    value::Value,
};

pub(crate) struct LegacyTestHarness {
    runtime: LegacyRuntimeFixture,
    cycles: u64,
}
impl LegacyTestHarness {
    pub(crate) fn from_source(source: &str) -> Result<Self, CompileError> {
        Ok(Self {
            runtime: CompileSession::from_source(source).build_legacy_fixture()?,
            cycles: 0,
        })
    }
    pub(crate) fn runtime(&self) -> &LegacyRuntimeFixture {
        &self.runtime
    }
    pub(crate) fn runtime_mut(&mut self) -> &mut LegacyRuntimeFixture {
        &mut self.runtime
    }
    pub(crate) fn cycle(&mut self) -> CycleResult {
        let result = self.runtime.execute_cycle();
        self.cycles += 1;
        CycleResult {
            cycle_number: self.cycles,
            elapsed_time: self.runtime.current_time(),
            errors: result.err().into_iter().collect(),
        }
    }
    // This historical corpus reads globals or direct program outputs. Preserve
    // the public harness's global-first and unique unqualified-member resolution;
    // full boundary grammar is tested through the installed public harness.
    pub(crate) fn get_output(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.runtime.storage().get_global(name) {
            return Some(value.clone());
        }
        let mut found = None;
        for program in self.runtime.programs().values() {
            let Some(Value::Instance(instance)) = self.runtime.storage().get_global(&program.name)
            else {
                continue;
            };
            let prefix = format!("{}.", program.name);
            let member = if name
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(&prefix))
            {
                &name[prefix.len()..]
            } else {
                name
            };
            if let Some(value) = self
                .runtime
                .storage()
                .get_instance_var_recursive(*instance, member)
            {
                if found.is_some() {
                    return None;
                }
                found = Some(value.clone());
            }
        }
        found
    }
}

pub(crate) fn legacy_module_from_source(
    source: &str,
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(crate::bytecode::BytecodeVersion::LEGACY)
}
pub(crate) fn legacy_bytes_from_source(source: &str) -> Result<Vec<u8>, CompileError> {
    legacy_module_from_source(source)?
        .encode()
        .map_err(|error| CompileError::new(error.to_string()))
}
