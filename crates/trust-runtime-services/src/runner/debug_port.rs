//! The loop's debugger as the debug operations see it (specification 13; specification 11 §6.9
//! and §6.13; specification 34, ARCH-11 and SERVICE-06).
//!
//! The shared request module ([`crate::debug::requests`]) parses a request and builds its reply.
//! It reads and changes the debugger through [`Debugger`], and this module implements that over the
//! loop: the state of reads is the storage the program owns, between scans or lent by the hold, and
//! the frames of the call stack are named from the prepared program on the way, so nothing is
//! copied (specification 11 §6.13.1).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use trust_runtime_core::bytecode::StorageRole;
use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::FrameId;
use trust_runtime_core::vm::{PreparedModule, StatementFrame};

use super::host::RunnerHost;
use super::RunnerPlatform;
use crate::debug::projection::DebugVariableHandles;
use crate::debug::reads::{NamedFrame, ReadState};
use crate::debug::replies::NO_SNAPSHOT;
use crate::debug::{BreakpointView, Debugger, LineSources, Span, StepKind, StopRecord};
use crate::live_values::io_requests::IoSnapshotRead;

impl<P: RunnerPlatform, C> Debugger for RunnerHost<'_, P, C> {
    fn sources(&self) -> Option<&LineSources> {
        self.program.debug_state()?.sources
    }

    /// The halted state while the debugger holds the scan; between scans the storage of the last
    /// scan that ended, with no call stack, once a scan has ended (specification 11 §6.13.1).
    fn read_state(&self) -> Result<ReadState<'_>, String> {
        let view = self
            .program
            .debug_state()
            .ok_or_else(|| String::from(NO_SNAPSHOT))?;
        let Some(frame) = view.frame else {
            if !self.debug.published() {
                return Err(String::from(NO_SNAPSHOT));
            }
            return Ok(ReadState {
                storage: view.storage,
                frames: Vec::new(),
            });
        };
        let frames = named_frames(view.prepared, frame).map_err(|error| error.to_string())?;
        Ok(ReadState {
            storage: view.storage,
            frames,
        })
    }

    fn io_snapshot(&self) -> IoSnapshotRead {
        self.platform.io_snapshot()
    }

    fn is_held(&self) -> bool {
        self.debug.is_held()
    }

    fn accepts_step(&self) -> bool {
        self.debug.accepts_step()
    }

    fn step(&self, kind: StepKind) {
        self.debug.step(kind);
    }

    fn last_stop(&self) -> Option<StopRecord> {
        self.debug.last_stop()
    }

    fn drain_stops(&self) -> Vec<StopRecord> {
        self.debug.drain_stops()
    }

    fn frame_span(&self, frame: FrameId) -> Option<StatementSpan> {
        self.debug.frame_span(frame)
    }

    fn breakpoints(&self) -> Vec<BreakpointView> {
        self.debug.breakpoints()
    }

    fn replace_breakpoints(&self, file_id: u32, spans: Vec<Span<(u32, u32)>>) -> u64 {
        self.debug.replace_breakpoints(file_id, spans)
    }

    fn clear_breakpoints(&self) {
        self.debug.clear_breakpoints();
    }

    fn with_handles(&self, operation: &mut dyn FnMut(&mut DebugVariableHandles)) {
        self.debug.with_handles(operation);
    }
}

/// The frames of the call stack of `frame`, outermost first, each with its locals named from the
/// prepared program: the declarations of the frame's POU give each slot its name. The compiler's
/// scratch slots are skipped and take their slots, so the rest keep their positions. Nothing is
/// copied: the names are the program's and the values are the frames' own.
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] when a frame names no POU, or its declarations do not
/// cover its slots exactly.
fn named_frames<'a>(
    prepared: &'a PreparedModule,
    frame: StatementFrame<'a>,
) -> Result<Vec<NamedFrame<'a>>, RuntimeError> {
    let strings = prepared.strings();
    let mut frames = Vec::new();
    for invocation in frame.call_stack() {
        let Some(id) = invocation.activation() else {
            continue;
        };
        let pou = invocation
            .pou_id()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let owner = prepared
            .pou_name(pou)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let locals = invocation.locals();
        let mut variables = Vec::new();
        let mut next_slot = 0_usize;
        for declaration in prepared.frame_declarations(pou) {
            let slot = usize::try_from(declaration.slot)
                .map_err(|_| RuntimeError::InvalidExecutionState)?;
            if slot != next_slot {
                return Err(RuntimeError::InvalidExecutionState);
            }
            if declaration.role == StorageRole::Scratch {
                let nodes = usize::try_from(declaration.construction_nodes)
                    .map_err(|_| RuntimeError::InvalidExecutionState)?;
                next_slot = slot.checked_add(nodes).ok_or(RuntimeError::Overflow)?;
                continue;
            }
            let name = usize::try_from(declaration.name_idx)
                .ok()
                .and_then(|index| strings.get(index))
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let value = locals
                .get(slot)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            variables.push((name.as_str(), value));
            next_slot = slot + 1;
        }
        if next_slot != locals.len() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        frames.push(NamedFrame {
            id,
            owner,
            instance: invocation.runtime_instance(),
            variables,
        });
    }
    Ok(frames)
}
