//! Online-change and hot-reload semantics.

use crate::error;

use super::core::Runtime;

impl Runtime {
    /// Apply online-change bytecode using the locked Phase E contract:
    /// - swap at scheduler cycle boundary,
    /// - invalidate in-flight instruction pointers by warm restart at entrypoint,
    /// - preserve retain state through warm-restart + retain-store reload.
    ///
    /// Host protocol surfaces such as the ADS server are refreshed by the
    /// control/runtime owner after this protocol-agnostic core swap returns.
    pub fn apply_online_change_bytes(
        &mut self,
        bytes: &[u8],
        resource_name: Option<&str>,
    ) -> Result<super::RuntimeMetadata, error::RuntimeError> {
        let retained = self.retain.load()?;
        self.apply_bytecode_bytes(bytes, resource_name)?;
        self.restart(super::types::RestartMode::Warm)?;
        self.apply_retain_snapshot(&retained)?;
        Ok(self.metadata_snapshot())
    }
}
