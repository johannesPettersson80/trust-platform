use crate::config::OpenOtTelemetryConfig;
use crate::error::RuntimeError;

#[cfg(unix)]
mod imp {
    use super::*;
    use crate::config::OpenOtTelemetryFenceMode;
    use open_ot_carriage::registry::{EVENT_HEARTBEAT, SYSTEM_SOURCE_ID};
    use open_ot_carriage::wire::Record;
    use open_ot_shm::{FenceMode, SharedRecordPublisher};
    use smol_str::SmolStr;
    use std::path::Path;

    #[derive(Debug, Default)]
    pub(crate) struct OpenOtTelemetrySubsystem {
        publisher: Option<SharedRecordPublisher>,
    }

    impl OpenOtTelemetrySubsystem {
        pub(crate) fn configure(
            &mut self,
            config: &OpenOtTelemetryConfig,
            bundle_root: Option<&Path>,
        ) -> Result<(), RuntimeError> {
            if !config.enabled {
                self.publisher = None;
                return Ok(());
            }
            validate_config(config)?;
            let path = resolve_path(&config.path, bundle_root);
            let publisher =
                SharedRecordPublisher::create_with_mode(&path, config.capacity, fence_mode(config))
                    .map_err(|err| telemetry_error("create", err))?;
            self.publisher = Some(publisher);
            Ok(())
        }

        #[must_use]
        pub(crate) fn is_enabled(&self) -> bool {
            self.publisher.is_some()
        }

        pub(crate) fn publish_heartbeat(&mut self, cycle_counter: u64) -> Result<(), RuntimeError> {
            let Some(publisher) = self.publisher.as_mut() else {
                return Ok(());
            };
            let record = Record::new(
                cycle_counter,
                1,
                cycle_counter,
                SYSTEM_SOURCE_ID,
                EVENT_HEARTBEAT,
            );
            publisher
                .append_record(&record)
                .map_err(|err| telemetry_error("publish heartbeat", err))
        }
    }

    fn validate_config(config: &OpenOtTelemetryConfig) -> Result<(), RuntimeError> {
        if config.path.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "runtime.openot.path must not be empty when runtime.openot.enabled=true".into(),
            ));
        }
        if config.capacity == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.openot.capacity must be >= 1".into(),
            ));
        }
        if config.fence_mode == OpenOtTelemetryFenceMode::Unfenced
            && !config.allow_unfenced_for_proof
        {
            return Err(RuntimeError::InvalidConfig(
                "runtime.openot.fence_mode='unfenced' requires runtime.openot.allow_unfenced_for_proof=true"
                    .into(),
            ));
        }
        Ok(())
    }

    fn resolve_path(path: &Path, bundle_root: Option<&Path>) -> std::path::PathBuf {
        if path.is_relative() {
            bundle_root.unwrap_or_else(|| Path::new(".")).join(path)
        } else {
            path.to_path_buf()
        }
    }

    fn fence_mode(config: &OpenOtTelemetryConfig) -> FenceMode {
        match config.fence_mode {
            OpenOtTelemetryFenceMode::Fenced => FenceMode::Fenced,
            OpenOtTelemetryFenceMode::Unfenced => FenceMode::Unfenced,
        }
    }

    fn telemetry_error(action: &str, err: impl std::fmt::Debug) -> RuntimeError {
        RuntimeError::IoTransport(SmolStr::new(format!(
            "OpenOT telemetry {action} failed: {err:?}"
        )))
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;
    use std::path::Path;

    #[derive(Debug, Default)]
    pub(crate) struct OpenOtTelemetrySubsystem;

    impl OpenOtTelemetrySubsystem {
        pub(crate) fn configure(
            &mut self,
            config: &OpenOtTelemetryConfig,
            _bundle_root: Option<&Path>,
        ) -> Result<(), RuntimeError> {
            if config.enabled {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot shared-memory telemetry is only supported on Unix targets"
                        .into(),
                ));
            }
            Ok(())
        }

        #[must_use]
        pub(crate) fn is_enabled(&self) -> bool {
            false
        }

        pub(crate) fn publish_heartbeat(
            &mut self,
            _cycle_counter: u64,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }
    }
}

pub(super) use imp::OpenOtTelemetrySubsystem;
