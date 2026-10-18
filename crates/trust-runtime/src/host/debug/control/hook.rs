impl Default for DebugControl {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugHook for DebugControl {
    fn on_statement(&mut self, location: Option<&SourceLocation>, call_depth: u32) {
        let _ = self.on_statement_inner(location, call_depth, None);
    }

    fn on_statement_with_context(
        &mut self,
        ctx: &mut DebugRuntimeContext<'_>,
        location: Option<&SourceLocation>,
        call_depth: u32,
    ) {
        // A lent context is never projected, so this observation cannot fail.
        let _ =
            self.on_statement_inner(location, call_depth, Some(&mut ObservedStorage::Lent(ctx)));
    }
}

impl DebugControl {
    /// Observe a located statement of a hosted scan. The scan's storage is read where it runs, and
    /// a detached projection is built only when a stop or a breakpoint condition or logpoint needs
    /// one (specification 11 section 6.13.1).
    pub(crate) fn on_statement_projection(
        &self,
        projection: &mut StatementProjection<'_>,
        location: &SourceLocation,
        call_depth: u32,
    ) -> Result<(), RuntimeError> {
        let mut observed = ObservedStorage::Live(&mut *projection);
        let result = self.on_statement_inner(Some(location), call_depth, Some(&mut observed));
        let copies = projection.copies();
        if copies > 0 {
            self.add_storage_copies(copies);
        }
        result
    }

    /// Observe a statement with a detached context that the caller hands over. A stop publishes a
    /// copy of it, so the caller keeps the context.
    #[cfg(test)]
    pub(crate) fn on_statement_with_owned_context(
        &mut self,
        ctx: &mut DebugRuntimeContext<'_>,
        location: Option<&SourceLocation>,
        call_depth: u32,
    ) {
        let _ =
            self.on_statement_inner(location, call_depth, Some(&mut ObservedStorage::Lent(ctx)));
    }

    /// The statement hook. The shared run control decides every stop; this function holds the scan
    /// with the condition variable while the run control says the scan is held here.
    fn on_statement_inner(
        &self,
        location: Option<&SourceLocation>,
        call_depth: u32,
        mut observed: Option<&mut ObservedStorage<'_, '_>>,
    ) -> Result<(), RuntimeError> {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        // A resource stop released the hold and no new one may start (`release_for_stop`): the
        // scan that was suspended runs to its end, and the ordinary stop path follows it.
        if run_control(&mut state).stop_requested() {
            return Ok(());
        }
        trace_debug(&format!(
            "hook.entry location={} depth={} mode={:?} current_thread={:?} target_thread={:?} pending_stop={:?} steps={} breakpoints={}",
            format_location_ref(location),
            call_depth,
            state.mode,
            state.current_thread,
            state.target_thread,
            state.pending_stop,
            state.steps.len(),
            state.breakpoints.len()
        ));
        state.last_location = location.copied();
        run_control(&mut state).record_boundary(call_depth);
        if let Some(observed) = observed.as_deref() {
            observed.bind_frame_locations(&mut state.frame_locations, location.copied());
        }
        emit_pending_stop(&mut state, observed.as_deref_mut(), location)?;
        if let Some(location) = location
            && let Some(stop) =
                boundary_stop(&mut state, location, call_depth, observed.as_deref_mut())?
        {
            trace_debug(&format!(
                "hook.pause.enter reason={:?} generation={:?} location={} thread={:?}",
                stop.reason,
                stop.breakpoint_generation,
                format_location_ref(Some(location)),
                state.current_thread
            ));
            if let Some(observed) = observed.as_deref_mut() {
                publish_hold_stop(&mut state, observed)?;
            }
            emit_stop(
                &mut state,
                stop.reason,
                Some(*location),
                stop.breakpoint_generation,
            );
        }
        loop {
            emit_pending_stop(&mut state, observed.as_deref_mut(), location)?;
            if !run_control(&mut state).holds_here() {
                trace_debug(&format!(
                    "hook.exit location={} current_thread={:?} target_thread={:?} mode={:?}",
                    format_location_ref(location),
                    state.current_thread,
                    state.target_thread,
                    state.mode
                ));
                return Ok(());
            }
            trace_debug(&format!(
                "hook.wait location={} current_thread={:?} target_thread={:?}",
                format_location_ref(location),
                state.current_thread,
                state.target_thread
            ));
            let wait_started = std::time::Instant::now();
            state.watchdog_hold_started = Some(wait_started);
            state = cvar.wait(state).expect("debug state poisoned");
            self.record_watchdog_pause(wait_started.elapsed());
            state.watchdog_hold_started = None;
            trace_debug(&format!(
                "hook.wake mode={:?} location={} current_thread={:?} target_thread={:?}",
                state.mode,
                format_location_ref(location),
                state.current_thread,
                state.target_thread
            ));
        }
    }

    /// Record storage copies made for a statement, under the state lock.
    fn add_storage_copies(&self, copies: u64) {
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.storage_copies = state.storage_copies.saturating_add(copies);
    }

    /// Publish the state of a completed scan: one copy of its storage at the scan end, with no call
    /// stack, as specification 11 section 6.13.1 requires. Nothing is copied while debugging is
    /// disabled (specification 11 section 6.9).
    pub(crate) fn publish_completed_scan(
        &self,
        storage: &VariableStorage,
        now: Duration,
        storage_abi: StorageAbi,
    ) {
        if !self.is_enabled() {
            return;
        }
        let mut copy = storage.clone();
        copy.clear_frames();
        let snapshot = DebugSnapshot {
            storage_abi,
            storage: copy,
            now,
        };
        let (lock, _) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.storage_copies = state.storage_copies.saturating_add(1);
        state.completed = Some(Arc::new(snapshot));
    }
}

/// Where the statement hook reads the storage of the statement it observes.
enum ObservedStorage<'a, 'b> {
    /// Storage the caller lends for this call. A stop copies it.
    Lent(&'a mut DebugRuntimeContext<'b>),
    /// The live storage of a hosted scan. A detached projection is built only when it is needed.
    Live(&'a mut StatementProjection<'b>),
}

impl ObservedStorage<'_, '_> {
    /// Bind the statement's location to the innermost active frame, and drop the locations of
    /// frames that are no longer on the call stack.
    fn bind_frame_locations(
        &self,
        frame_locations: &mut HashMap<FrameId, SourceLocation>,
        location: Option<SourceLocation>,
    ) {
        match self {
            Self::Lent(ctx) => {
                let frames = ctx.storage.frames();
                frame_locations.retain(|id, _| frames.iter().any(|frame| frame.id == *id));
                if let (Some(location), Some(frame)) = (location, ctx.storage.current_frame()) {
                    frame_locations.insert(frame.id, location);
                }
            }
            Self::Live(projection) => {
                frame_locations
                    .retain(|id, _| projection.active_frames().any(|frame| frame == *id));
                if let (Some(location), Some(frame)) = (location, projection.active_frames().last())
                {
                    frame_locations.insert(frame, location);
                }
            }
        }
    }

    /// Whether a breakpoint at this statement evaluates a condition or a logpoint, which needs the
    /// storage. A suppressed hit is not counted. A hit whose condition is not met is counted, but
    /// not evaluated. The rule is `breakpoints::any_breakpoint_needs_context`.
    fn needs_breakpoint_context(
        &self,
        breakpoints: &[DebugBreakpoint],
        stopped_this_cycle: &HashSet<BreakpointCycleKey>,
        location: &SourceLocation,
    ) -> bool {
        match self {
            Self::Lent(_) => true,
            Self::Live(_) => super::breakpoints::any_breakpoint_needs_context(
                breakpoints,
                stopped_this_cycle,
                location,
            ),
        }
    }

    /// Run `f` with the evaluation context of this statement. A live statement builds its
    /// projection here, on first use.
    fn with_context<R>(
        &mut self,
        f: impl FnOnce(&mut DebugRuntimeContext<'_>) -> R,
    ) -> Result<R, RuntimeError> {
        match self {
            Self::Lent(ctx) => Ok(f(ctx)),
            Self::Live(projection) => projection.with_context(f),
        }
    }

    /// Publish the state of a stop: the storage and call stack at this boundary, with the watches
    /// recomputed on them, as the halted state (specification 11 section 6.13.1).
    fn publish_stop(&mut self, state: &mut DebugState) -> Result<(), RuntimeError> {
        match self {
            Self::Lent(ctx) => {
                update_watch_snapshot(state, ctx);
                update_snapshot(state, ctx);
                Ok(())
            }
            Self::Live(projection) => {
                if projection.is_published() {
                    return Ok(());
                }
                projection.with_context(|ctx| update_watch_snapshot(&mut *state, ctx))?;
                state.snapshot = Some(projection.take()?);
                Ok(())
            }
        }
    }
}

/// The stop of a located statement boundary. The shared run control decides it, and asks for the
/// breakpoint match only when no step stops here. The match needs the storage for a condition or a
/// logpoint, so it runs here (specification 13, "Breakpoint evaluation, source resolution, and hook
/// contract").
fn boundary_stop(
    state: &mut DebugState,
    location: &SourceLocation,
    call_depth: u32,
    observed: Option<&mut ObservedStorage<'_, '_>>,
) -> Result<Option<BoundaryStop>, RuntimeError> {
    let DebugState {
        mode,
        pending_stop,
        current_thread,
        target_thread,
        steps,
        last_call_depth,
        last_call_depths,
        stop_requested,
        breakpoints,
        breakpoint_stops_this_cycle,
        logs,
        log_tx,
        ..
    } = state;
    let mut run = RunControl {
        mode,
        pending_stop,
        current_thread,
        target_thread,
        steps,
        last_call_depth,
        last_call_depths,
        stop_requested,
    };
    run.boundary(call_depth, || {
        let needs_context = observed.as_deref().is_some_and(|observed| {
            observed.needs_breakpoint_context(
                breakpoints.as_slice(),
                breakpoint_stops_this_cycle,
                location,
            )
        });
        let matched = match observed {
            Some(observed) if needs_context => observed.with_context(|ctx| {
                matches_breakpoint(
                    breakpoints,
                    breakpoint_stops_this_cycle,
                    logs,
                    log_tx.as_ref(),
                    location,
                    &mut Some(ctx),
                )
            })?,
            _ => matches_breakpoint(
                breakpoints,
                breakpoint_stops_this_cycle,
                logs,
                log_tx.as_ref(),
                location,
                &mut None,
            ),
        };
        Ok(matched.map(|(generation, cycle_key)| {
            breakpoint_stops_this_cycle.insert(cycle_key);
            generation
        }))
    })
}

/// Emit the pending stop of the held target at this boundary, if there is one (specification 13,
/// "Pause"). The halted state is published first, as for every stop of a held scan.
fn emit_pending_stop(
    state: &mut DebugState,
    observed: Option<&mut ObservedStorage<'_, '_>>,
    location: Option<&SourceLocation>,
) -> Result<(), RuntimeError> {
    let Some(reason) = run_control(state).take_pending_stop() else {
        return Ok(());
    };
    trace_debug(&format!(
        "hook.pending_stop.consume reason={reason:?} location={} thread={:?}",
        format_location_ref(location),
        state.current_thread
    ));
    if let Some(observed) = observed {
        publish_hold_stop(state, observed)?;
    }
    emit_stop(state, reason, location.copied(), None);
    Ok(())
}

/// Publish the halted state of a stop that holds the scan. A stop that cannot be published ends the
/// hold it would have started, so the scan never waits for a resume that no stop asked for.
fn publish_hold_stop(
    state: &mut DebugState,
    observed: &mut ObservedStorage<'_, '_>,
) -> Result<(), RuntimeError> {
    let published = observed.publish_stop(state);
    if published.is_err() {
        run_control(state).abandon_stop();
    }
    published
}

fn format_location_ref(location: Option<&SourceLocation>) -> String {
    location
        .map(|loc| format!("{}:{}..{}", loc.file_id, loc.start, loc.end))
        .unwrap_or_else(|| "<none>".to_string())
}

fn emit_stop(
    state: &mut DebugState,
    reason: DebugStopReason,
    location: Option<SourceLocation>,
    breakpoint_generation: Option<u64>,
) {
    trace_debug(&format!(
        "stop reason={reason:?} location={:?} thread={:?}",
        location, state.current_thread
    ));
    let stop = DebugStop {
        reason,
        location,
        thread_id: state.current_thread,
        breakpoint_generation,
    };
    if let Some(sender) = &state.stop_tx
        && sender.send(stop.clone()).is_err()
    {
        trace_debug("stop sender closed; retaining stop in debug control buffer");
    }
    state.last_stop = Some(stop.clone());
    state.stops.push(stop);
}

fn update_watch_snapshot(state: &mut DebugState, ctx: &mut DebugRuntimeContext<'_>) {
    let mut changed = false;
    for watch in &mut state.watches {
        let next = crate::helper_eval::eval_storage_expr_with_abi(
            ctx.storage,
            watch.registry.as_deref().unwrap_or(ctx.registry),
            ctx.storage_abi,
            &ctx.profile,
            ctx.current_instance,
            ctx.stdlib,
            &watch.expr,
        )
        .ok();
        if watch.last != next {
            watch.last = next;
            changed = true;
        }
    }
    if changed {
        state.watch_changed = true;
    }
}

fn update_snapshot(state: &mut DebugState, ctx: &mut DebugRuntimeContext<'_>) {
    state.snapshot = Some(DebugSnapshot {
        storage_abi: ctx.storage_abi,
        storage: ctx.storage.clone(),
        now: ctx.now,
    });
}

#[cfg(test)]
#[path = "hook/contract_tests.rs"]
mod contract_tests;
