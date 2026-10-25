//! The rules of an online change that every device shares (specification 34 §11.3; UPDATE-06;
//! specification 44 §12.3): the memory a candidate may use and the refusal when it needs more, and
//! the match between the candidate's bindings and the lines of the running program's record.
//!
//! The device supplies the numbers, the heap's room now and the reserve it keeps; the rules decide
//! what they allow. Nothing here allocates a program or claims a line.

use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::Write as _;

use trust_runtime_core::deployment::{GpioInput, GpioOutput};
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::memory::IoArea;

use crate::io_drivers::EntryBytes;

/// The memory an online change may use, and the numbers its refusal names (specification 34 §11.3,
/// UPDATE-06): the length of the program, the bytes of heap that were free when it was taken, and
/// the reserve the device keeps for the machine. The program may use the room less the reserve
/// ([`available`](Self::available)).
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
        }
    }

    /// The bytes of heap the program may use: the room less the reserve.
    #[must_use]
    pub const fn available(self) -> usize {
        self.room.saturating_sub(self.reserve)
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

/// The first input or output binding of `bindings` whose address neither one of the record's lines
/// nor one of its other drivers' entries covers (specification 34 §11.3). A change keeps the running
/// program's lines and drivers and brings no record, so a binding at an address where the record
/// gives neither cannot be wired and is refused. A marker (`%M`) binding needs neither. A line covers
/// a bit binding when it is that bit, and a binding of a byte or more when the line lies in one of
/// its bytes; a driver's entry covers a binding that shares one of its bytes (`entries`, from
/// [`crate::io_drivers::entry_bytes`]). A hierarchical address has neither.
#[must_use]
pub fn unwired_binding(
    bindings: &[PreparedIoBinding],
    inputs: &[GpioInput],
    outputs: &[GpioOutput],
    entries: &[EntryBytes],
) -> Option<IoAddress> {
    let driven = |address: &IoAddress| {
        if address.path.len() > 1 {
            return false;
        }
        let Ok(Some(end)) = address.flat_byte_end() else {
            return false;
        };
        entries.iter().any(|entry| {
            entry.area == address.area && entry.bytes.start < end && address.byte < entry.bytes.end
        })
    };
    let wired = |binding: &PreparedIoBinding| match binding.address.area {
        IoArea::Input => {
            inputs
                .iter()
                .any(|line| covers(&binding.address, &line.address))
                || driven(&binding.address)
        }
        IoArea::Output => {
            outputs
                .iter()
                .any(|line| covers(&binding.address, &line.address))
                || driven(&binding.address)
        }
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
