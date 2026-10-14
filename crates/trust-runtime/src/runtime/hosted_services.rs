//! Host clocks and detached debugger observations for the shared execution owner.

use std::{
    cell::Cell,
    sync::Arc,
    time::{Duration as PhysicalDuration, Instant},
};

use crate::{
    debug::{DebugControl, DebugRuntimeContext, DebugSnapshot},
    error::RuntimeError,
    memory::VariableStorage,
    value::{DateTimeValue, Duration, Value},
};
use trust_runtime_core::vm::{ExecutionServices, PreparedModule, StatementFrame};

use super::RuntimeMetadata;

/// Services belong to the controller owner; observers never reacquire that owner.
pub(super) struct HostedExecutionServices {
    prepared: Arc<PreparedModule>,
    register_configuration: trust_runtime_core::vm::register::RegisterExecutionConfiguration,
    metadata: Option<Arc<RuntimeMetadata>>,
    debug: Option<DebugControl>,
    deadline: Cell<Option<Instant>>,
    pause_baseline: Cell<PhysicalDuration>,
}

impl HostedExecutionServices {
    pub(super) fn new(
        prepared: Arc<PreparedModule>,
        metadata: Option<Arc<RuntimeMetadata>>,
        debug: Option<DebugControl>,
    ) -> Self {
        Self {
            prepared,
            register_configuration: crate::register_environment::capture(),
            metadata,
            debug,
            deadline: Cell::new(None),
            pause_baseline: Cell::new(PhysicalDuration::ZERO),
        }
    }

    pub(super) fn set_register_configuration(
        &mut self,
        configuration: trust_runtime_core::vm::register::RegisterExecutionConfiguration,
    ) {
        self.register_configuration = configuration;
    }

    pub(super) fn set_execution_deadline(&self, deadline: Option<Instant>) {
        self.deadline.set(deadline);
        self.pause_baseline.set(self.pause_elapsed());
    }

    pub(super) fn execution_deadline(&self) -> Option<Instant> {
        self.deadline.get()
    }

    pub(super) fn deadline_state(&self) -> (Option<Instant>, PhysicalDuration) {
        (self.deadline.get(), self.pause_baseline.get())
    }

    pub(super) fn suspend_execution_deadline(&self) -> (Option<Instant>, PhysicalDuration) {
        (self.deadline.replace(None), self.pause_baseline.get())
    }

    pub(super) fn restore_execution_deadline(
        &self,
        suspended: (Option<Instant>, PhysicalDuration),
    ) {
        self.deadline.set(suspended.0);
        self.pause_baseline.set(suspended.1);
    }

    pub(super) fn effective_execution_deadline(&self) -> Option<Instant> {
        let deadline = self.deadline.get()?;
        deadline_with_pause(
            Some(deadline),
            self.pause_baseline.get(),
            self.pause_elapsed(),
        )
    }

    pub(super) fn debug_control(&self) -> Option<&DebugControl> {
        self.debug.as_ref()
    }

    pub(super) fn set_debug_control(&mut self, debug: Option<DebugControl>) {
        self.debug = debug;
    }

    pub(super) fn set_prepared(&mut self, prepared: Arc<PreparedModule>) {
        self.prepared = prepared;
    }

    pub(super) fn set_metadata(&mut self, metadata: Arc<RuntimeMetadata>) {
        self.metadata = Some(metadata);
    }

    fn pause_elapsed(&self) -> PhysicalDuration {
        self.debug
            .as_ref()
            .map(DebugControl::watchdog_pause_elapsed)
            .unwrap_or_default()
    }
}

// Preserve the hosted watchdog contract, including reset/overflow behavior.
fn deadline_with_pause(
    deadline: Option<Instant>,
    baseline: PhysicalDuration,
    elapsed: PhysicalDuration,
) -> Option<Instant> {
    let delta = elapsed.saturating_sub(baseline);
    deadline.map(|value| value.checked_add(delta).unwrap_or(value))
}

impl ExecutionServices for HostedExecutionServices {
    fn register_configuration(
        &self,
    ) -> trust_runtime_core::vm::register::RegisterExecutionConfiguration {
        self.register_configuration
    }

    fn debug_active(&self) -> bool {
        self.debug.is_some() && self.metadata.is_some()
    }

    fn deadline_exceeded(&self) -> bool {
        self.effective_execution_deadline()
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn has_wall_clock(&self) -> bool {
        true
    }

    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        match crate::stdlib::time::runtime_clock_value("CURRENT_DT", Duration::ZERO)? {
            Value::Dt(value) => Ok(value),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    fn on_statement_with_frame_checked(
        &self,
        pou_id: u32,
        code_offset: usize,
        depth: u32,
        storage: &VariableStorage,
        logical_time: Duration,
        frame: StatementFrame<'_>,
    ) -> Result<(), RuntimeError> {
        let (Some(metadata), Some(debug)) = (&self.metadata, &self.debug) else {
            return Ok(());
        };
        let pc = u32::try_from(code_offset).map_err(|_| RuntimeError::InvalidExecutionState)?;
        let Some(source) = self.prepared.debug_map().source_by_pc.get(&(pou_id, pc)) else {
            return Ok(());
        };
        let Some(location) =
            metadata.resolve_vm_debug_location(&source.file, source.line, source.column)
        else {
            return Ok(());
        };
        let mut snapshot =
            DebugSnapshot::from_execution(storage, logical_time, &self.prepared, frame)?;
        let current_instance = snapshot
            .storage
            .current_frame()
            .and_then(|frame| frame.instance_id);
        let mut context = DebugRuntimeContext {
            storage_abi: self.prepared.storage_abi(),
            storage: &mut snapshot.storage,
            registry: metadata.registry(),
            stdlib: Some(metadata.stdlib()),
            profile: metadata.profile(),
            current_instance,
            now: logical_time,
        };
        // The previous VM supplied only a raw snapshot and no evaluation context.
        // Contextual hooks evaluate conditions/watches on this detached projection;
        // any side effects of a debug expression stay outside installed PLC state.
        debug
            .clone()
            .on_statement_with_owned_context(&mut context, Some(&location), depth);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        harness::{CompileSession, SourceFile},
        program_model::Expr,
    };
    use trust_runtime_core::vm::PreparationLimits;

    const GPIO: &[u8] = include_bytes!("../../tests/fixtures/portability/f401/gpio.stbc");
    const GPIO_SOURCE: &str = include_str!("../../tests/fixtures/portability/f401/gpio.st");

    #[test]
    fn execution_deadline_counts_only_new_debug_pause_and_keeps_overflow_fallback() {
        let deadline = Instant::now();
        let baseline = PhysicalDuration::from_secs(7);
        assert_eq!(deadline_with_pause(None, baseline, baseline), None);
        assert_eq!(
            deadline_with_pause(Some(deadline), baseline, PhysicalDuration::from_secs(10)),
            deadline.checked_add(PhysicalDuration::from_secs(3))
        );
        assert_eq!(
            deadline_with_pause(Some(deadline), baseline, PhysicalDuration::from_secs(1)),
            Some(deadline)
        );
        assert_eq!(
            deadline_with_pause(
                Some(deadline),
                PhysicalDuration::ZERO,
                PhysicalDuration::MAX
            ),
            Some(deadline)
        );
        let prepared =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        let services = HostedExecutionServices::new(prepared, None, None);
        assert!(!services.debug_active());
        assert!(services.has_wall_clock());
        assert!(services.current_dt().is_ok());
        services.set_execution_deadline(Some(deadline));
        assert_eq!(services.execution_deadline(), Some(deadline));
        assert!(services.deadline_exceeded());
        services.set_execution_deadline(None);
        assert!(!services.deadline_exceeded());
        services.pause_baseline.set(PhysicalDuration::from_secs(7));
        services.deadline.set(Some(deadline));
        let suspended = services.suspend_execution_deadline();
        assert_eq!(suspended, (Some(deadline), PhysicalDuration::from_secs(7)));
        assert!(
            !services.deadline_exceeded(),
            "safe output delivery is a separate owner operation"
        );
        services.pause_baseline.set(PhysicalDuration::from_secs(99));
        services.restore_execution_deadline(suspended);
        assert_eq!(services.deadline.get(), Some(deadline));
        assert_eq!(
            services.pause_baseline.get(),
            PhysicalDuration::from_secs(7),
            "safety delivery cannot rebase the original pause allowance"
        );
    }

    #[test]
    fn absent_deadline_keeps_pause_baseline_and_resumes_the_original_deadline() {
        let prepared =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        for debug in [None, Some(DebugControl::new())] {
            let services = HostedExecutionServices::new(Arc::clone(&prepared), None, debug);
            // An absent deadline needs no pause adjustment. Even extreme stale
            // baselines are retained until a real deadline is set or restored.
            services.pause_baseline.set(PhysicalDuration::MAX);
            for _ in 0..3 {
                assert_eq!(services.effective_execution_deadline(), None);
                assert!(!services.deadline_exceeded());
                assert_eq!(services.deadline_state(), (None, PhysicalDuration::MAX));
            }
            let deadline = Instant::now();
            services.set_execution_deadline(Some(deadline));
            let original = services.deadline_state();
            let suspended = services.suspend_execution_deadline();
            assert_eq!(services.effective_execution_deadline(), None);
            assert!(!services.deadline_exceeded());
            services.restore_execution_deadline(suspended);
            assert_eq!(services.deadline_state(), original);
            assert_eq!(services.effective_execution_deadline(), Some(deadline));
            assert!(services.deadline_exceeded());
        }
    }

    #[test]
    fn saved_v2_execution_uses_detached_named_snapshot_watches_and_pause_control() {
        let prepared =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        let label = prepared
            .debug_map()
            .source_by_pc
            .values()
            .next()
            .unwrap()
            .file
            .to_string();
        let authoring =
            CompileSession::from_sources(vec![SourceFile::with_path(label, GPIO_SOURCE)])
                .build_runtime()
                .unwrap();
        let metadata = Arc::new(authoring.metadata_snapshot());
        let debug = DebugControl::new();
        debug.register_watch_expression(Expr::Name("probe_depth".into()));
        let (sender, receiver) = std::sync::mpsc::channel();
        debug.set_stop_sender(sender);
        debug.pause_entry();
        let services = HostedExecutionServices::new(
            Arc::clone(&prepared),
            Some(metadata),
            Some(debug.clone()),
        );
        assert!(services.debug_control().is_some());
        let mut state = prepared
            .instantiate_owned_with_services(0, services)
            .unwrap();
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        let handle = debug.clone();
        let observer = std::thread::spawn(move || {
            let stop = receiver.recv_timeout(PhysicalDuration::from_secs(10));
            let snapshot = handle.snapshot();
            let watch_changed = handle.take_watch_changed();
            handle.with_snapshot(|snapshot| {
                snapshot.storage.set_global("probe_depth", Value::Int(99));
            });
            // Always release a pause before asserting, including a failed timeout.
            handle.continue_run();
            (stop, snapshot, watch_changed)
        });
        let result = state.execute_cycle(Duration::from_millis(10));
        let (stop, snapshot, watch_changed) = observer.join().unwrap();
        result.unwrap();
        assert!(stop.unwrap().location.is_some());
        let snapshot = snapshot.expect("pause publishes the actual named frame");
        assert_eq!(snapshot.now, Duration::from_millis(10));
        assert_eq!(snapshot.storage.current_frame().unwrap().owner, "Gpio");
        assert_eq!(
            snapshot.storage.get_global("probe_depth"),
            Some(&Value::Int(4))
        );
        assert!(watch_changed);
        assert_eq!(
            state.storage().get_global("probe_depth"),
            Some(&Value::Int(4))
        );
        assert!(state.storage().frames().is_empty());
    }

    #[test]
    fn saved_v2_breakpoint_condition_uses_the_installed_value_in_detached_context() {
        use crate::{debug::DebugBreakpoint, program_model::BinaryOp};
        let prepared =
            Arc::new(PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap());
        let label = prepared
            .debug_map()
            .source_by_pc
            .values()
            .next()
            .unwrap()
            .file
            .to_string();
        let authoring =
            CompileSession::from_sources(vec![SourceFile::with_path(label, GPIO_SOURCE)])
                .build_runtime()
                .unwrap();
        let metadata = Arc::new(authoring.metadata_snapshot());
        let (_, source) = prepared
            .debug_map()
            .source_by_pc
            .iter()
            .filter(|((pou, _), _)| prepared.pou_name(*pou) == Some("Gpio"))
            .min_by_key(|((_, pc), _)| *pc)
            .expect("real program source map");
        let location = metadata
            .resolve_vm_debug_location(&source.file, source.line, source.column)
            .expect("program statement in matching immutable metadata");
        let debug = DebugControl::new();
        let mut breakpoint = DebugBreakpoint::new(location);
        breakpoint.condition = Some(Expr::Binary {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Name("probe_depth".into())),
            right: Box::new(Expr::Literal(Value::Int(4))),
        });
        debug.set_breakpoints_for_file(location.file_id, vec![breakpoint]);
        let (sender, receiver) = std::sync::mpsc::channel();
        debug.set_stop_sender(sender);
        let services = HostedExecutionServices::new(
            Arc::clone(&prepared),
            Some(metadata),
            Some(debug.clone()),
        );
        let mut state = prepared
            .instantiate_owned_with_services(0, services)
            .unwrap();
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        assert!(
            matches!(
                receiver.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ),
            "false expression does not stop the real PLC cycle"
        );
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        let handle = debug.clone();
        let observer = std::thread::spawn(move || {
            let stop = receiver.recv_timeout(PhysicalDuration::from_secs(10));
            let snapshot = handle.snapshot();
            handle.continue_run();
            (stop, snapshot)
        });
        let result = state.execute_cycle(Duration::from_millis(20));
        let (stop, snapshot) = observer.join().unwrap();
        result.unwrap();
        let stop = stop.unwrap();
        assert_eq!(stop.reason, crate::debug::DebugStopReason::Breakpoint);
        assert_eq!(stop.location, Some(location));
        let snapshot = snapshot.expect("matching condition publishes detached owner state");
        assert_eq!(snapshot.now, Duration::from_millis(20));
        assert_eq!(
            snapshot.storage.get_global("probe_depth"),
            Some(&Value::Int(4))
        );
        assert_eq!(
            state.storage().get_global("probe_depth"),
            Some(&Value::Int(4))
        );
        assert_eq!(state.storage().execution_frame_count(), 0);
    }
}
