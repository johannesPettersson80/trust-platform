impl Default for DebugControl {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugHook for DebugControl {
    fn on_statement(&mut self, location: Option<&SourceLocation>, call_depth: u32) {
        self.on_statement_inner(location, call_depth, None);
    }

    fn on_statement_with_context(
        &mut self,
        ctx: &mut DebugRuntimeContext<'_>,
        location: Option<&SourceLocation>,
        call_depth: u32,
    ) {
        self.on_statement_inner(location, call_depth, Some(ctx));
    }
}

impl DebugControl {
    fn on_statement_inner(
        &mut self,
        location: Option<&SourceLocation>,
        call_depth: u32,
        mut ctx: Option<&mut DebugRuntimeContext<'_>>,
    ) {
        let (lock, cvar) = &*self.state;
        let mut state = lock.lock().expect("debug state poisoned");
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
        state.last_call_depth = call_depth;
        if let Some(thread_id) = state.current_thread {
            state.last_call_depths.insert(thread_id, call_depth);
        }
        if let (Some(location), Some(eval_ctx)) = (location, ctx.as_deref()) {
            let frames = eval_ctx.storage.frames();
            state
                .frame_locations
                .retain(|id, _| frames.iter().any(|frame| frame.id == *id));
            if let Some(frame) = eval_ctx.storage.current_frame() {
                state.frame_locations.insert(frame.id, *location);
            }
        }
        let is_target_thread =
            state.target_thread.is_none() || state.target_thread == state.current_thread;
        if matches!(state.mode, DebugMode::Paused) && is_target_thread {
            if let Some(reason) = state.pending_stop.take() {
                trace_debug(&format!(
                    "hook.pending_stop.consume reason={reason:?} location={} thread={:?}",
                    format_location_ref(location),
                    state.current_thread
                ));
                if let Some(eval_ctx) = ctx.as_mut() {
                    update_watch_snapshot(&mut state, eval_ctx);
                    update_snapshot(&mut state, eval_ctx);
                }
                emit_stop(&mut state, reason, location.copied(), None);
            }
        }
        let effective_mode = if is_target_thread {
            state.mode
        } else {
            DebugMode::Running
        };
        trace_debug(&format!(
            "hook.decision effective_mode={effective_mode:?} is_target_thread={} location={}",
            is_target_thread,
            format_location_ref(location)
        ));
        if let (DebugMode::Running, Some(location)) = (effective_mode, location) {
            let mut should_pause = false;
            let mut stop_reason = None;
            let mut stop_generation = None;
            if is_target_thread {
                let step_key = state
                    .current_thread
                    .filter(|id| state.steps.contains_key(id))
                    .or_else(|| state.steps.contains_key(&0).then_some(0));
                if let Some(step_key) = step_key {
                    if let Some(step) = state.steps.get_mut(&step_key) {
                        if !step.started {
                            step.started = true;
                            trace_debug(&format!(
                                "hook.step.arm key={} kind={:?} target_depth={} current_depth={}",
                                step_key, step.kind, step.target_depth, call_depth
                            ));
                        } else {
                            should_pause = match step.kind {
                                StepKind::Into => true,
                                StepKind::Over => call_depth <= step.target_depth,
                                StepKind::Out => call_depth <= step.target_depth,
                            };
                            trace_debug(&format!(
                                "hook.step.check key={} kind={:?} target_depth={} current_depth={} should_pause={}",
                                step_key, step.kind, step.target_depth, call_depth, should_pause
                            ));
                            if should_pause {
                                state.steps.remove(&step_key);
                                stop_reason = Some(DebugStopReason::Step);
                            }
                        }
                    }
                }
            }
            if !should_pause {
                let breakpoint_generation = {
                    let DebugState {
                        breakpoints,
                        logs,
                        log_tx,
                        ..
                    } = &mut *state;
                    matches_breakpoint(breakpoints, logs, log_tx.as_ref(), location, &mut ctx)
                };
                trace_debug(&format!(
                    "hook.breakpoint.check location={} matched_generation={:?}",
                    format_location_ref(Some(location)),
                    breakpoint_generation
                ));
                if let Some(generation) = breakpoint_generation {
                    should_pause = true;
                    state.steps.clear();
                    stop_reason = Some(DebugStopReason::Breakpoint);
                    stop_generation = Some(generation);
                    state.target_thread = None;
                }
            }
            if should_pause {
                state.mode = DebugMode::Paused;
                if let Some(reason) = stop_reason {
                    state.pending_stop = None;
                    trace_debug(&format!(
                        "hook.pause.enter reason={reason:?} generation={:?} location={} thread={:?}",
                        stop_generation,
                        format_location_ref(Some(location)),
                        state.current_thread
                    ));
                    if let Some(eval_ctx) = ctx.as_mut() {
                        update_watch_snapshot(&mut state, eval_ctx);
                        update_snapshot(&mut state, eval_ctx);
                    }
                    emit_stop(&mut state, reason, Some(*location), stop_generation);
                }
            }
        }
        loop {
            let is_target_thread =
                state.target_thread.is_none() || state.target_thread == state.current_thread;
            if matches!(state.mode, DebugMode::Paused) && is_target_thread {
                if let Some(reason) = state.pending_stop.take() {
                    trace_debug(&format!(
                        "hook.pending_stop.consume reason={reason:?} location={} thread={:?}",
                        format_location_ref(location),
                        state.current_thread
                    ));
                    if let Some(eval_ctx) = ctx.as_mut() {
                        update_watch_snapshot(&mut state, eval_ctx);
                        update_snapshot(&mut state, eval_ctx);
                    }
                    emit_stop(&mut state, reason, location.copied(), None);
                }
            }
            match state.mode {
                DebugMode::Running => {
                    trace_debug(&format!(
                        "hook.exit reason=running location={} thread={:?}",
                        format_location_ref(location),
                        state.current_thread
                    ));
                    return;
                }
                DebugMode::Paused => {
                    if !is_target_thread {
                        trace_debug(&format!(
                            "hook.exit reason=paused_non_target location={} current_thread={:?} target_thread={:?}",
                            format_location_ref(location),
                            state.current_thread,
                            state.target_thread
                        ));
                        return;
                    }
                    trace_debug(&format!(
                        "hook.wait location={} current_thread={:?} target_thread={:?}",
                        format_location_ref(location),
                        state.current_thread,
                        state.target_thread
                    ));
                    state = cvar.wait(state).expect("debug state poisoned");
                    trace_debug(&format!(
                        "hook.wake mode={:?} location={} current_thread={:?} target_thread={:?}",
                        state.mode,
                        format_location_ref(location),
                        state.current_thread,
                        state.target_thread
                    ));
                }
            }
        }
    }
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
    if let Some(sender) = &state.stop_tx {
        if sender.send(stop.clone()).is_err() {
            trace_debug("stop sender closed; retaining stop in debug control buffer");
        }
    }
    state.last_stop = Some(stop.clone());
    state.stops.push(stop);
}

fn update_watch_snapshot(state: &mut DebugState, ctx: &mut DebugRuntimeContext<'_>) {
    let mut changed = false;
    for watch in &mut state.watches {
        let next = crate::helper_eval::eval_storage_expr_with_stdlib(
            ctx.storage,
            ctx.registry,
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
        storage: ctx.storage.clone(),
        now: ctx.now,
    });
}
