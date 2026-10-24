//! The debug operations of the control protocol, answered from a [`Debugger`]: `debug.state`,
//! `debug.stops`, `debug.stack`, `debug.scopes`, `debug.variables`, `debug.breakpoint_locations`,
//! `breakpoints.set`, `breakpoints.clear`, `breakpoints.list`, `breakpoints.clear_all`,
//! `breakpoints.clear_id`, `step_in`, `step_over` and `step_out`, and `eval` and `debug.evaluate`,
//! which a compiled program answers and any other program refuses (specification 11 §6.9 and
//! §6.13; specification 13).
//!
//! The dispatcher has checked the request's role and the debug surface before it calls
//! [`answer`], so a request that gets here is permitted. A request reads the published debug state
//! (specification 11 §6.13.1) and changes nothing but the debugger's own state: its breakpoints,
//! its run control and its variable handles. Positions in a request and in a reply are the
//! zero-based line and column that trust-runtime has always carried; the program's `LINE_TABLE`
//! holds them one-based, and this module converts at its boundary.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::memory::FrameId;

use super::breakpoint::BreakpointOptions;
use super::logpoint::LogRecord;
use super::options::{OptionsError, WireOptions};
use super::paging::{ChildKind, Page};
use super::projection::{file_name_of, DebugSource, DebugVariableHandles};
use super::reads::{self, Located, ReadState};
use super::replies::{self, StopPlace};
use super::run::{StepKind, StopReason};
use super::sources::LineSources;
use super::statement::Span;
use crate::live_values::io_requests::IoSnapshotRead;
use crate::live_values::io_snapshot::IoSnapshot;
use crate::protocol::{ControlRequest, ControlResponse};

/// The parameters of `breakpoints.set` and `breakpoints.clear`.
#[derive(Debug, Deserialize)]
pub struct BreakpointsParams {
    /// The file, named by its path.
    pub source: String,
    /// The zero-based lines to place a breakpoint on.
    pub lines: Vec<u32>,
    /// Each line's options, as many as the lines (specification 11 §6.13.2).
    #[serde(default)]
    pub options: Option<Vec<Option<WireOptions>>>,
}

/// The parameters of `breakpoints.clear_id`.
#[derive(Debug, Deserialize)]
pub struct BreakpointsClearIdParams {
    /// The identifier of the file whose breakpoints are cleared.
    pub file_id: u32,
}

/// The parameters of `debug.scopes`.
#[derive(Debug, Deserialize)]
pub struct DebugScopesParams {
    /// The frame whose scopes are listed.
    pub frame_id: u32,
}

/// The parameters of `debug.variables`: the handle, and the optional arguments of the Debug Adapter
/// Protocol's `variables` request that page its children (specification 11 §6.13.3, "Variable
/// pages").
#[derive(Debug, Deserialize)]
pub struct DebugVariablesParams {
    /// The handle of the scope or variable whose members are listed.
    pub variables_reference: u32,
    /// The kind of children to list, or both kinds when absent.
    pub filter: Option<ChildKind>,
    /// The index of the first child to list, among the children that `filter` keeps.
    pub start: Option<u32>,
    /// The most children to list; absent or `0`, an array lists a page of its elements.
    pub count: Option<u32>,
}

impl DebugVariablesParams {
    /// The window of the children that these parameters ask for.
    #[must_use]
    pub fn page(&self) -> Page {
        let size = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);
        Page {
            filter: self.filter,
            start: self.start.map_or(0, size),
            count: self.count.map(size),
        }
    }
}

/// The parameters of `debug.breakpoint_locations`: a range of a file, zero-based.
#[derive(Debug, Deserialize)]
pub struct DebugBreakpointLocationsParams {
    /// The file, named by its path.
    pub source: String,
    /// The first line of the range.
    pub line: u32,
    /// The last line of the range; the first line when absent.
    pub end_line: Option<u32>,
    /// The first column, on the first line.
    pub column: Option<u32>,
    /// The last column, on the last line.
    pub end_column: Option<u32>,
}

/// A stop the debugger recorded: why the scan stopped, before which statement, on which thread,
/// and the generation of the breakpoint set that named a breakpoint stop (specification 13,
/// "Stopped Events").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopRecord {
    /// Why the scan stopped.
    pub reason: StopReason,
    /// The statement that the scan stopped before.
    pub statement: StatementSpan,
    /// The thread that stopped.
    pub thread: Option<u32>,
    /// The generation of the breakpoint set that named a breakpoint stop, and none for any other.
    pub breakpoint_generation: Option<u64>,
}

/// A breakpoint as `breakpoints.list` reports it: its file and the one-based span of the statement
/// it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakpointView {
    /// The file's identifier.
    pub file_id: u32,
    /// The statement's first line and column.
    pub start: (u32, u32),
    /// The position just after the statement.
    pub end: (u32, u32),
}

/// What the debug operations read and change on a runtime that holds no source text (specification
/// 34, ARCH-11 and SERVICE-06). The runtime's resource loop implements it over its own debugger.
pub trait Debugger {
    /// The program's `LINE_TABLE` and file labels, or `None` for a program that has no
    /// `LINE_TABLE`, which cannot be debugged (specification 11 §6.13.3).
    fn sources(&self) -> Option<&LineSources>;

    /// The state a read answers from: the halted state while the debugger holds the scan, and
    /// otherwise the state of the last scan that ended (specification 11 §6.13.1).
    ///
    /// # Errors
    ///
    /// The error text of a read that has no state to answer from, `no snapshot available`
    /// before the first scan ends.
    fn read_state(&self) -> Result<ReadState<'_>, String>;

    /// The published I/O snapshot, which the I/O scope lists.
    fn io_snapshot(&self) -> IoSnapshotRead;

    /// Whether the debugger holds the scan, or has a hold pending.
    fn is_held(&self) -> bool;

    /// Whether a step is accepted now: only while the debugger holds the scan.
    fn accepts_step(&self) -> bool;

    /// Releases the hold until the boundary of a step of `kind`.
    fn step(&self, kind: StepKind);

    /// The latest stop.
    fn last_stop(&self) -> Option<StopRecord>;

    /// The stops since the last call, oldest first. The buffer is bounded; a stop beyond its
    /// bound replaces the oldest (specification 11 §6.13.3).
    fn drain_stops(&self) -> Vec<StopRecord>;

    /// The logpoint messages since the last call, oldest first, bounded as the stops are
    /// (specification 11 §6.13.2); none from a debugger that evaluates no logpoint.
    fn drain_logs(&self) -> Vec<LogRecord> {
        Vec::new()
    }

    /// The statement that a frame last stood at.
    fn frame_span(&self, frame: FrameId) -> Option<StatementSpan>;

    /// The breakpoints, in registration order.
    fn breakpoints(&self) -> Vec<BreakpointView>;

    /// Replaces all breakpoints of file `file_id` with `breakpoints`, each on its span with its
    /// options, and returns the file's new generation (specification 13, "Runtime debug state,
    /// mutation, and projection invariants").
    fn replace_breakpoints(
        &self,
        file_id: u32,
        breakpoints: Vec<(Span<(u32, u32)>, BreakpointOptions)>,
    ) -> u64;

    /// Clears every breakpoint and every generation.
    fn clear_breakpoints(&self);

    /// Runs `operation` on the variable handles of the current stop.
    fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles));

    /// A compiled program's memory, while the debugger holds its scan and between scans
    /// (specification 44 §13.2); `None` for the interpreter's program, which
    /// [`Self::read_state`] answers for. A compiled program's stack, scopes and variables are read
    /// through it.
    #[cfg(feature = "native")]
    fn program_memory(&self) -> Option<&dyn trust_runtime_core::native::memory::ProgramMemory> {
        None
    }

    /// Why the program cannot be debugged at all, when it cannot: a compiled release image, which
    /// has no statement hooks (specification 44 §13.2). `None` for a debuggable program.
    fn undebuggable(&self) -> Option<&'static str> {
        None
    }
}

/// Whether `kind` is a debug operation that [`answer`] serves.
#[must_use]
pub fn is_debug_operation(kind: &str) -> bool {
    matches!(
        kind,
        "debug.state"
            | "debug.stops"
            | "debug.stack"
            | "debug.scopes"
            | "debug.variables"
            | "debug.breakpoint_locations"
            | "debug.evaluate"
            | "eval"
            | "breakpoints.set"
            | "breakpoints.clear"
            | "breakpoints.list"
            | "breakpoints.clear_all"
            | "breakpoints.clear_id"
            | "step_in"
            | "step_over"
            | "step_out"
    )
}

/// Answers the debug operation `request` from `debugger`.
///
/// A compiled program evaluates `debug.evaluate` in the compiled form of version 1 and answers
/// `eval` as an exact name read; a program without its memory, which evaluates no compiled
/// expression, refuses both (specification 11 §6.13.2). Every other operation on a program without
/// a `LINE_TABLE` is refused with the stable `no_line_table` error (specification 11 §6.13.3).
#[must_use]
pub fn answer(request: &ControlRequest, debugger: &dyn Debugger) -> ControlResponse {
    let id = request.id;
    let kind = request.r#type.as_str();
    if let Some(reason) = debugger.undebuggable() {
        return failed(id, reason);
    }
    if matches!(kind, "eval" | "debug.evaluate") {
        #[cfg(feature = "native")]
        if let Some(memory) = debugger.program_memory() {
            return super::compiled::evaluate(id, kind, request.params.as_ref(), memory);
        }
        return replies::expressions_unavailable(id);
    }
    let Some(sources) = debugger.sources() else {
        return replies::no_line_table(id);
    };
    let params = request.params.as_ref();
    match kind {
        "debug.state" => state(id, debugger, sources),
        "debug.stops" => stops(id, debugger, sources),
        "debug.stack" => {
            #[cfg(feature = "native")]
            if let Some(memory) = debugger.program_memory() {
                return super::compiled::stack(id, memory, debugger, sources);
            }
            stack(id, debugger, sources)
        }
        "debug.scopes" => {
            #[cfg(feature = "native")]
            if let Some(memory) = debugger.program_memory() {
                return super::compiled::scopes(id, params, memory, debugger, sources);
            }
            scopes(id, params, debugger, sources)
        }
        "debug.variables" => {
            #[cfg(feature = "native")]
            if let Some(memory) = debugger.program_memory() {
                return super::compiled::variables(id, params, memory, debugger);
            }
            variables(id, params, debugger)
        }
        "debug.breakpoint_locations" => breakpoint_locations(id, params, sources),
        "breakpoints.set" => breakpoints_set(id, params, debugger, sources),
        "breakpoints.clear" => breakpoints_clear(id, params, debugger, sources),
        "breakpoints.list" => breakpoints_list(id, debugger),
        "breakpoints.clear_all" => {
            debugger.clear_breakpoints();
            replies::breakpoints_cleared(id)
        }
        "breakpoints.clear_id" => breakpoints_clear_id(id, params, debugger, sources),
        "step_in" => step(id, debugger, StepKind::Into),
        "step_over" => step(id, debugger, StepKind::Over),
        "step_out" => step(id, debugger, StepKind::Out),
        _ => ControlResponse::error(id, String::from("unsupported request")),
    }
}

/// The parameters of a request, or the error text of missing or malformed ones.
pub(super) fn parse<T: DeserializeOwned>(params: Option<&serde_json::Value>) -> Result<T, String> {
    match params {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|err| alloc::format!("invalid params: {err}")),
        None => Err(String::from("missing params")),
    }
}

/// The failed response with the error text `error`.
pub(super) fn failed(id: u64, error: impl Into<String>) -> ControlResponse {
    ControlResponse::error(id, error.into())
}

/// The position of a statement as a frame or a scope reply carries it, zero-based, with the file
/// named by its label.
pub(super) fn located(sources: &LineSources, statement: StatementSpan) -> Option<Located> {
    let label = sources.label(statement.file_index)?;
    Some(Located {
        source: DebugSource {
            name: Some(String::from(file_name_of(label))),
            path: Some(String::from(label)),
        },
        line: statement.start.0.saturating_sub(1),
        column: statement.start.1.saturating_sub(1),
    })
}

/// The reply entry of a stop: the stop's place is named when its file has a label.
fn stop_json(stop: &StopRecord, sources: &LineSources) -> serde_json::Value {
    let place = sources
        .label(stop.statement.file_index)
        .map(|path| StopPlace {
            file_id: stop.statement.file_index,
            line: stop.statement.start.0.saturating_sub(1),
            column: stop.statement.start.1.saturating_sub(1),
            path,
        });
    replies::stop(stop.reason, stop.thread, stop.breakpoint_generation, place)
}

fn state(id: u64, debugger: &dyn Debugger, sources: &LineSources) -> ControlResponse {
    let last_stop = debugger.last_stop().map(|stop| stop_json(&stop, sources));
    replies::state(id, debugger.is_held(), last_stop.as_ref())
}

fn stops(id: u64, debugger: &dyn Debugger, sources: &LineSources) -> ControlResponse {
    let stops = debugger
        .drain_stops()
        .iter()
        .map(|stop| stop_json(stop, sources))
        .collect::<Vec<_>>();
    let logs = debugger
        .drain_logs()
        .iter()
        .map(|log| {
            replies::log(
                &log.text,
                log.statement.file_index,
                sources.label(log.statement.file_index),
                log.statement.start.0.saturating_sub(1),
                log.statement.start.1.saturating_sub(1),
                log.thread,
            )
        })
        .collect::<Vec<_>>();
    replies::stops(id, &stops, &logs)
}

fn stack(id: u64, debugger: &dyn Debugger, sources: &LineSources) -> ControlResponse {
    let state = match debugger.read_state() {
        Ok(state) => state,
        Err(error) => return failed(id, error),
    };
    let fallback = debugger
        .last_stop()
        .and_then(|stop| located(sources, stop.statement));
    let frames = reads::stack_frames(
        &state.frames,
        |frame| {
            debugger
                .frame_span(frame)
                .and_then(|statement| located(sources, statement))
        },
        fallback.as_ref(),
    );
    replies::stack(id, &frames)
}

fn scopes(
    id: u64,
    params: Option<&serde_json::Value>,
    debugger: &dyn Debugger,
    sources: &LineSources,
) -> ControlResponse {
    let params: DebugScopesParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let state = match debugger.read_state() {
        Ok(state) => state,
        Err(error) => return failed(id, error),
    };
    let frame = FrameId(params.frame_id);
    if !state.frames.iter().any(|candidate| candidate.id == frame) {
        return failed(id, replies::unknown_frame(params.frame_id));
    }
    let place = debugger
        .frame_span(frame)
        .or_else(|| debugger.last_stop().map(|stop| stop.statement))
        .and_then(|statement| located(sources, statement));
    let has_io = match debugger.io_snapshot() {
        IoSnapshotRead::Published(io) => {
            !(io.inputs.is_empty() && io.outputs.is_empty() && io.memory.is_empty())
        }
        IoSnapshotRead::Missing => false,
        IoSnapshotRead::Unavailable => return failed(id, "I/O snapshot unavailable"),
    };
    let mut reply = None;
    debugger.with_handles(&mut |handles| {
        let scopes = reads::scopes(handles, state.storage, frame, place.as_ref(), has_io);
        reply = Some(if handles.take_overflow() {
            handles.clear();
            replies::handles_full(id)
        } else {
            replies::scopes(id, &scopes)
        });
    });
    reply.unwrap_or_else(|| failed(id, "debug variables unavailable"))
}

fn variables(
    id: u64,
    params: Option<&serde_json::Value>,
    debugger: &dyn Debugger,
) -> ControlResponse {
    let params: DebugVariablesParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let page = params.page();
    let state = match debugger.read_state() {
        Ok(state) => state,
        Err(error) => return failed(id, error),
    };
    let io: Option<IoSnapshot> = match debugger.io_snapshot() {
        IoSnapshotRead::Published(io) => Some(io),
        IoSnapshotRead::Missing => None,
        IoSnapshotRead::Unavailable => return failed(id, "I/O snapshot unavailable"),
    };
    let mut reply = None;
    debugger.with_handles(&mut |handles| {
        // The handle is out of the table while its members are listed, because listing hands out
        // their handles, and it is back under its own identifier afterwards. A listing that the
        // bound refuses is rolled back, so the table is as it was (specification 11 §6.13.3).
        reply = Some(match handles.take(params.variables_reference) {
            Some(handle) => {
                let mark = handles.mark();
                let members = reads::variables(&state, &handle, io.as_ref(), handles, page);
                handles.restore(params.variables_reference, handle);
                if handles.take_overflow() {
                    handles.rollback(mark);
                    replies::handles_full(id)
                } else {
                    match members {
                        Ok(variables) => replies::variables(id, &variables),
                        Err(error) => failed(id, error),
                    }
                }
            }
            None => failed(
                id,
                alloc::format!("unknown variables reference {}", params.variables_reference),
            ),
        });
    });
    reply.unwrap_or_else(|| failed(id, "debug variables unavailable"))
}

fn breakpoint_locations(
    id: u64,
    params: Option<&serde_json::Value>,
    sources: &LineSources,
) -> ControlResponse {
    let params: DebugBreakpointLocationsParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let Some(file) = sources.file_index_of(&params.source) else {
        return failed(id, "unknown source path");
    };
    let max_line = params.end_line.unwrap_or(params.line);
    let mut breakpoints = Vec::new();
    for (line, column) in sources.statement_starts(file) {
        let (line, column) = (line.saturating_sub(1), column.saturating_sub(1));
        if line < params.line || line > max_line {
            continue;
        }
        if let Some(min_column) = params.column
            && line == params.line
            && column < min_column
        {
            continue;
        }
        if let Some(max_column) = params.end_column
            && line == max_line
            && column > max_column
        {
            continue;
        }
        breakpoints.push(replies::position(line, column));
    }
    replies::breakpoint_locations(id, &breakpoints)
}

/// Whether `params` ask for a breakpoint condition or a logpoint, which are debug expressions.
fn carries_expression(params: &serde_json::Value) -> bool {
    ["condition", "log_message"]
        .into_iter()
        .any(|member| params.get(member).is_some_and(|value| !value.is_null()))
}

/// Whether `debugger` evaluates compiled expressions: a compiled program's does.
fn evaluates_expressions(debugger: &dyn Debugger) -> bool {
    #[cfg(feature = "native")]
    {
        debugger.program_memory().is_some()
    }
    #[cfg(not(feature = "native"))]
    {
        let _ = debugger;
        false
    }
}

/// Each line's options of `params`, decoded, or the refusal of the whole request (specification 11
/// §6.13.2).
fn line_options(
    id: u64,
    params: &mut BreakpointsParams,
    evaluates: bool,
) -> Result<Vec<BreakpointOptions>, ControlResponse> {
    let Some(options) = params.options.take() else {
        return Ok(vec![BreakpointOptions::default(); params.lines.len()]);
    };
    if options.len() != params.lines.len() {
        return Err(failed(id, "the options are not as many as the lines"));
    }
    options
        .into_iter()
        .map(|entry| {
            let Some(entry) = entry else {
                return Ok(BreakpointOptions::default());
            };
            if entry.has_expressions() && !evaluates {
                return Err(replies::expressions_unavailable(id));
            }
            entry.decode().map_err(|error| match error {
                OptionsError::Version(version) => replies::expr_version_unsupported(id, version),
                OptionsError::Invalid(detail) => {
                    ControlResponse::error_with_code(id, detail, "expr_invalid")
                }
                OptionsError::Malformed(detail) => failed(id, detail),
            })
        })
        .collect()
}

fn breakpoints_set(
    id: u64,
    params: Option<&serde_json::Value>,
    debugger: &dyn Debugger,
    sources: &LineSources,
) -> ControlResponse {
    let evaluates = evaluates_expressions(debugger);
    if params.is_some_and(carries_expression) {
        // A condition or a message belongs to one line, in `options`.
        if evaluates {
            return failed(
                id,
                "a condition or a log message goes in `options`, one per line",
            );
        }
        return replies::expressions_unavailable(id);
    }
    let mut params: BreakpointsParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let options = match line_options(id, &mut params, evaluates) {
        Ok(options) => options,
        Err(refusal) => return refusal,
    };
    let Some(file) = sources.file_index_of(&params.source) else {
        return failed(id, "unknown source path");
    };
    let mut spans = Vec::new();
    let mut resolved = Vec::new();
    for (line, options) in params.lines.into_iter().zip(options) {
        // The request carries a zero-based line. As trust-runtime resolves a request, the column
        // is the zero-based column 1, so the table is asked for one-based line + 1 and column 2.
        let Some(statement) = line
            .checked_add(1)
            .and_then(|line| sources.resolve_breakpoint(file, line, 2))
        else {
            continue;
        };
        spans.push((
            Span::new(
                (statement.line, statement.column),
                (statement.end_line, statement.end_column),
            ),
            options,
        ));
        resolved.push(replies::position(
            statement.line.saturating_sub(1),
            statement.column.saturating_sub(1),
        ));
    }
    let generation = debugger.replace_breakpoints(file, spans);
    replies::breakpoints_set(id, file, &resolved, Some(generation))
}

fn breakpoints_clear(
    id: u64,
    params: Option<&serde_json::Value>,
    debugger: &dyn Debugger,
    sources: &LineSources,
) -> ControlResponse {
    let params: BreakpointsParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    let Some(file) = sources.file_index_of(&params.source) else {
        return failed(id, "unknown source path");
    };
    debugger.replace_breakpoints(file, Vec::new());
    replies::breakpoints_cleared(id)
}

fn breakpoints_clear_id(
    id: u64,
    params: Option<&serde_json::Value>,
    debugger: &dyn Debugger,
    sources: &LineSources,
) -> ControlResponse {
    let params: BreakpointsClearIdParams = match parse(params) {
        Ok(params) => params,
        Err(error) => return failed(id, error),
    };
    if sources.label(params.file_id).is_none() {
        return failed(id, "unknown file id");
    }
    debugger.replace_breakpoints(params.file_id, Vec::new());
    replies::breakpoints_cleared_for_file(id, params.file_id)
}

fn breakpoints_list(id: u64, debugger: &dyn Debugger) -> ControlResponse {
    let breakpoints = debugger
        .breakpoints()
        .iter()
        .map(|breakpoint| {
            replies::breakpoint_entry(
                breakpoint.file_id,
                breakpoint.start.0.saturating_sub(1),
                breakpoint.start.1.saturating_sub(1),
                breakpoint.end.0.saturating_sub(1),
                breakpoint.end.1.saturating_sub(1),
            )
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}

fn step(id: u64, debugger: &dyn Debugger, kind: StepKind) -> ControlResponse {
    if !debugger.accepts_step() {
        return failed(id, replies::STEP_REQUIRES_HOLD);
    }
    debugger.step(kind);
    replies::stepping(id)
}
