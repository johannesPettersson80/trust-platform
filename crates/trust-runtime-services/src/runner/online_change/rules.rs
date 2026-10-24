//! The rules of an online change that every device shares (specification 34 §11.3; UPDATE-06): the
//! memory a candidate may use and the refusal when it needs more, and the match between the
//! candidate's bindings and the lines of the running program's record.
//!
//! The device supplies the numbers, its memory ([`DeviceMemory`]), the heap's room now and the reserve it
//! keeps; the rules decide what they allow. Nothing here allocates a program or claims a line.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use trust_runtime_core::bytecode::CallDepth;
use trust_runtime_core::deployment::{GpioInput, GpioOutput};
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::error_code::StableErrorCode;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

use crate::runner::engine::{device_limits, DeviceMemory};

/// The memory an online change (or the check of an install) may use, and the numbers its refusal
/// names (specification 34 §11.3, UPDATE-06): the length of the program, the bytes of heap that were
/// free when it was taken, and the reserve the device keeps for the machine.
///
/// The program may use the room less the reserve ([`available`](Self::available)), and the limits it
/// is prepared with are the device's limits for a heap of that size ([`limits`](Self::limits)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeMemory {
    /// What asks for the memory, as the refusal words it: `a change` or `a program`.
    subject: &'static str,
    /// The length of the program, in bytes.
    bytes: usize,
    /// The bytes of heap that were free.
    room: usize,
    /// The bytes of the room that stay free for the machine.
    reserve: usize,
    /// The call levels the device's main stack holds, which the refusal of a program that calls
    /// deeper names ([`with_call_depth`](Self::with_call_depth)).
    call_depth: usize,
}

impl ChangeMemory {
    /// The memory of an online change of `bytes` bytes, taken when the heap had `room` free bytes,
    /// with `reserve` of them kept.
    #[must_use]
    pub const fn for_change(bytes: usize, room: usize, reserve: usize) -> Self {
        Self {
            subject: "a change",
            bytes,
            room,
            reserve,
            call_depth: 0,
        }
    }

    /// The memory of the check of an install of a program of `bytes` bytes, taken when the heap had
    /// `room` free bytes, with `reserve` of them kept.
    #[must_use]
    pub const fn for_install(bytes: usize, room: usize, reserve: usize) -> Self {
        Self {
            subject: "a program",
            bytes,
            room,
            reserve,
            call_depth: 0,
        }
    }

    /// This memory with the `call_depth` levels that the device's main stack holds
    /// ([`DeviceMemory::call_depth`]), which the refusal of a program that calls deeper names.
    #[must_use]
    pub const fn with_call_depth(mut self, call_depth: usize) -> Self {
        self.call_depth = call_depth;
        self
    }

    /// The bytes of heap the program may use: the room less the reserve.
    #[must_use]
    pub const fn available(self) -> usize {
        self.room.saturating_sub(self.reserve)
    }

    /// The preparation limits of the program: the device's limits for a heap of
    /// [`available`](Self::available) bytes ([`device_limits`]), and an artifact no longer than those
    /// bytes, since an artifact longer than the heap that remains cannot be prepared in it.
    ///
    /// # Errors
    ///
    /// The `insufficient memory` refusal when no heap is available, so that the device's limits for
    /// it give the preparation no byte: no program can be prepared in it.
    pub fn limits(self, memory: DeviceMemory) -> Result<PreparationLimits, String> {
        let available = self.available();
        let limits = device_limits(DeviceMemory {
            region_bytes: memory.region_bytes.min(available),
            heap_bytes: available,
            ..memory
        });
        if limits.max_preparation_bytes == 0 {
            return Err(self.needs_more());
        }
        Ok(limits)
    }

    /// The reply for `error`, which the preparation or the construction of the program returned: the
    /// `insufficient memory` refusal with its numbers when the memory limits refused the program, and
    /// the error's own text otherwise, as a computer replies with the error of its transaction.
    ///
    /// A limit of the preparation counts, whichever step met it: the artifact's length, the memory or
    /// the work of the decoder, the cumulative preparation budget, or the storage or the work of the
    /// validator ([`RuntimeError::is_preparation_limit`]). The limits are the device's limits for the
    /// heap that remains, so a program that needs more than they allow needs more memory than it has.
    ///
    /// A construction that calls deeper than the call levels the device's main stack holds is the
    /// `insufficient stack` refusal with that number ([`is_call_depth_limit`]; MEM-03).
    #[must_use]
    pub fn refusal(self, error: &RuntimeError) -> String {
        match error {
            error if error.is_preparation_limit() => self.needs_more(),
            error if is_call_depth_limit(error) => format!(
                "insufficient stack: {} of {} bytes calls deeper than the {} call levels that \
                 the device's main stack holds",
                self.subject, self.bytes, self.call_depth,
            ),
            RuntimeError::ExecutionLimit {
                kind:
                    kind @ (ExecutionLimitKind::AllocationBytes | ExecutionLimitKind::ConstructedValues),
                used,
                requested,
                limit,
            } => format!(
                "insufficient memory: building {} of {} bytes needs {requested} more {kind} after \
                 {used}, above the limit of {limit} that the {} bytes of heap that remain for it \
                 allow ({} bytes are free, {} are kept in reserve)",
                self.subject,
                self.bytes,
                self.available(),
                self.room,
                self.reserve,
            ),
            other => other.to_string(),
        }
    }

    /// The `insufficient memory` refusal of a program whose memory figure (specification 34 MEM-12)
    /// is `needed` bytes, more than the heap that remains for it once its upload is freed: the room
    /// less the reserve, with the upload's bytes. `None` when the figure fits.
    #[must_use]
    pub fn figure_refusal(self, needed: usize) -> Option<String> {
        let available = self.available().saturating_add(self.bytes);
        (needed > available).then(|| {
            format!(
                "insufficient memory: {} of {} bytes needs {needed} bytes of memory, more than \
                 the {available} bytes of heap that remain for it once it is prepared ({} bytes \
                 are free, {} are kept in reserve)",
                self.subject, self.bytes, self.room, self.reserve,
            )
        })
    }

    /// The `insufficient memory` refusal of a compiled program whose areas, bookkeeping and retain
    /// capture take `needed` bytes, its memory figure less its stack bound, which the running
    /// program and the new one share, when they are more than the heap that remains beside the
    /// running program and the new image, which stays in the heap for as long as the changed program
    /// runs (specification 44 §12.3, step 2). `None` when they fit.
    #[must_use]
    pub fn held_figure_refusal(self, needed: usize) -> Option<String> {
        let available = self.available();
        (needed > available).then(|| {
            format!(
                "insufficient memory: {} of {} bytes needs {needed} bytes of memory, more than \
                 the {available} bytes of heap that remain for it beside its image ({} bytes are \
                 free, {} are kept in reserve)",
                self.subject, self.bytes, self.room, self.reserve,
            )
        })
    }

    /// The `insufficient stack` refusal of `prepared` when its static call depth is more than the call
    /// levels the device's main stack holds ([`too_deep`]; specification 34 MEM-03). `None` when the
    /// program fits.
    #[must_use]
    pub fn stack_refusal(self, prepared: &PreparedModule) -> Option<String> {
        too_deep(prepared, self.call_depth)
            .map(|depth| stack_shortage(self.subject, self.bytes, depth, self.call_depth))
    }

    /// The `insufficient memory` refusal of a program that needs more than the heap that remains
    /// for it.
    #[must_use]
    pub fn needs_more(self) -> String {
        format!(
            "insufficient memory: {} of {} bytes needs more than the {} bytes of heap that remain \
             for it ({} bytes are free, {} are kept in reserve)",
            self.subject,
            self.bytes,
            self.available(),
            self.room,
            self.reserve,
        )
    }
}

/// The static call depth of `prepared` when it is more than the `call_depth` levels that a device's
/// main stack holds (specification 34 MEM-03; owner, 21 October 2026, "Stack decides"): a device
/// refuses such a program before it constructs it. A program whose call graph is recursive has no
/// static depth: the depth check refuses its call when the call is made, as on a computer.
#[must_use]
pub fn too_deep(prepared: &PreparedModule, call_depth: usize) -> Option<usize> {
    match prepared.static_call_depth() {
        CallDepth::Static(depth) if depth > call_depth => Some(depth),
        _ => None,
    }
}

/// The `insufficient stack` refusal of `subject` (`a change`, `a program`) of `bytes` bytes, which
/// calls `depth` levels deep, more than the `call_depth` levels the device's main stack holds
/// ([`too_deep`]).
#[must_use]
pub fn stack_shortage(subject: &str, bytes: usize, depth: usize, call_depth: usize) -> String {
    format!(
        "insufficient stack: {subject} of {bytes} bytes needs {depth} call levels, more than the \
         {call_depth} that the device's main stack holds"
    )
}

/// Whether `error` is a call deeper than the admitted call depth (`max_call_depth`), which on a device
/// is the call levels its main stack holds and [`ChangeMemory::refusal`] words as `insufficient stack`
/// (specification 34 MEM-03). The answer comes from the error's stable code, not from its text.
#[must_use]
pub fn is_call_depth_limit(error: &RuntimeError) -> bool {
    error.stable_code() == StableErrorCode::VmCallStackOverflow
}

/// Whether `error` is the memory limits refusing a program, which [`ChangeMemory::refusal`] words as
/// `insufficient memory`: a limit of the preparation ([`RuntimeError::is_preparation_limit`]), or the
/// construction exceeding its allowance of allocation bytes or constructed values.
#[must_use]
pub fn is_memory_limit(error: &RuntimeError) -> bool {
    error.is_preparation_limit()
        || matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes | ExecutionLimitKind::ConstructedValues,
                ..
            }
        )
}

/// Prepares the module of an online change from `program`, the upload of the request, with the limits
/// that the heap allows when `room` bytes are free and `reserve` of them are kept (specification 34
/// §11.3, UPDATE-06), and frees the upload at once. Returns the module and the memory, whose numbers
/// the refusal of the candidate's construction names too.
///
/// # Errors
///
/// The `insufficient memory` refusal when the heap allows no limits or the program needs more than
/// they allow, the `insufficient stack` refusal when it calls deeper than the device's main stack
/// holds, and the error of the preparation otherwise (a program that is not valid bytecode, for
/// example).
pub fn prepare_candidate(
    program: Vec<u8>,
    memory: DeviceMemory,
    room: usize,
    reserve: usize,
) -> Result<(PreparedModule, ChangeMemory), String> {
    let budget =
        ChangeMemory::for_change(program.len(), room, reserve).with_call_depth(memory.call_depth);
    let limits = budget.limits(memory)?;
    let prepared =
        PreparedModule::from_bytes(&program, limits).map_err(|error| budget.refusal(&error))?;
    drop(program);
    if let Some(refusal) = budget.stack_refusal(&prepared) {
        return Err(refusal);
    }
    Ok((prepared, budget))
}

/// The first input or output binding of `bindings` whose address none of the record's lines covers
/// (specification 34 §11.3). A change keeps the running program's lines and brings no record, so a
/// binding at an address where the record gives no line cannot be wired and is refused. A marker (`%M`)
/// binding needs no line. A line covers a bit binding when it is that bit, and a binding of a byte or
/// more when the line lies in one of its bytes. A hierarchical address has no line.
#[must_use]
pub fn unwired_binding(
    bindings: &[PreparedIoBinding],
    inputs: &[GpioInput],
    outputs: &[GpioOutput],
) -> Option<IoAddress> {
    let wired = |binding: &PreparedIoBinding| match binding.address.area {
        IoArea::Input => inputs
            .iter()
            .any(|line| covers(&binding.address, &line.address)),
        IoArea::Output => outputs
            .iter()
            .any(|line| covers(&binding.address, &line.address)),
        IoArea::Memory => true,
    };
    bindings
        .iter()
        .find(|binding| !wired(binding))
        .map(|binding| binding.address.clone())
}

/// Whether the line at `line` lies in the bits or bytes that the binding at `binding` covers.
fn covers(binding: &IoAddress, line: &IoAddress) -> bool {
    if binding.path.len() > 1 || line.path.len() > 1 {
        return false;
    }
    let Ok(Some(end)) = binding.flat_byte_end() else {
        return false;
    };
    match binding.size {
        IoSize::Bit => line.byte == binding.byte && line.bit == binding.bit,
        _ => (binding.byte..end).contains(&line.byte),
    }
}

/// The IEC text of `address`, for example `%QX0.5` or `%IW2`, as a refusal names it.
#[must_use]
pub fn address_text(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    let size = match address.size {
        IoSize::Bit => 'X',
        IoSize::Byte | IoSize::Bytes(_) => 'B',
        IoSize::Word => 'W',
        IoSize::DWord => 'D',
        IoSize::LWord => 'L',
    };
    let mut text = format!("%{area}{size}");
    if address.path.len() > 1 {
        for (index, part) in address.path.iter().enumerate() {
            if index > 0 {
                text.push('.');
            }
            text.push_str(&part.to_string());
        }
    } else {
        text.push_str(&address.byte.to_string());
    }
    // Writing into a `String` cannot fail.
    let _ = match address.size {
        IoSize::Bit => write!(text, ".{}", address.bit),
        IoSize::Bytes(length) => write!(text, "[{length}]"),
        _ => Ok(()),
    };
    text
}

#[cfg(test)]
mod contract_tests;
