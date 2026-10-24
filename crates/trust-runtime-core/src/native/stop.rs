//! A stop of a debug image's scan (specification 44 §13.2): the module waits in the kernel's `stop`
//! at a statement whose breakpoint bit is set, and the scan's hooks decide there, with the
//! statement, its call frames and the program's memory as the scan left it. While the hooks hold
//! the scan they read and write the memory through [`Stop`], which is the program's memory then.

use alloc::vec::Vec;
use core::cell::Cell;

use trust_plc_abi::kernel::{self as abi_kernel, StopSink};
use trust_plc_abi::{Cx, FaultCode};

use super::image::{Image, StmtRecord};
use super::memory::{pou_symbol, Place, ProgramMemory, StopFrame};
use super::program::ScanHooks;
use crate::error::RuntimeError;
use crate::io_address::IoAddress;
use crate::io_image::write_flat_image;
use crate::memory::IoArea;
use crate::value::Value;

/// A held statement of a debug image's scan: the statement, its call frames and the program's
/// memory, valid while the module waits in the stop.
pub struct Stop<'s> {
    cx: &'s mut Cx,
    image: Image<'s>,
    bits: &'s [Cell<u8>],
}

impl<'s> Stop<'s> {
    pub(super) fn new(cx: &'s mut Cx, image: Image<'s>, bits: &'s [Cell<u8>]) -> Self {
        Self { cx, image, bits }
    }

    /// The id of the held statement (specification 44 §5.9).
    #[must_use]
    pub fn statement(&self) -> u32 {
        self.cx.stmt
    }

    /// The statement table's record of the held statement.
    #[must_use]
    pub fn record(&self) -> Option<StmtRecord> {
        self.image
            .table::<StmtRecord>()
            .ok()
            .and_then(|statements| statements.get(self.cx.stmt))
    }

    /// The call depth of the held statement: the frames the module linked (specification 44
    /// §13.2), which a step compares with the depth it started at.
    #[must_use]
    pub fn depth(&self) -> u32 {
        u32::try_from(abi_kernel::frames(self.cx).count()).unwrap_or(u32::MAX)
    }

    /// The task the held statement runs in, `u32::MAX` for none.
    #[must_use]
    pub fn task(&self) -> u32 {
        self.cx.task
    }

    /// Arms the statements the scan stops at from here: every one when `every` is set, otherwise
    /// those `wanted` takes (specification 44 §13.2).
    pub fn arm(&mut self, every: bool, wanted: &dyn Fn(u32) -> bool) {
        let armed = arm_bits(self.bits, every, wanted);
        self.cx.armed = u32::from(armed);
    }

    /// The place in its area of the instance at `address`, found by the areas' bases.
    fn place_of(&self, address: usize) -> Option<Place> {
        (1..=7u8).find_map(|area| {
            let (base, len) = abi_kernel::area_address(self.cx, area)?;
            let offset = address.checked_sub(base).filter(|offset| *offset < len)?;
            Some(Place::Area {
                area,
                offset: u32::try_from(offset).ok()?,
            })
        })
    }

    /// The size of the frame of POU `pou`: its POU symbol's size.
    fn frame_size(&self, pou: u32) -> Option<usize> {
        pou_symbol(&self.image, pou).map(|(_, symbol)| symbol.size as usize)
    }
}

impl ProgramMemory for Stop<'_> {
    fn image(&self) -> Image<'_> {
        self.image
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        abi_kernel::read_area(self.cx, id, offset, out)
    }

    fn write_area(&mut self, id: u8, offset: usize, data: &[u8]) -> bool {
        abi_kernel::write_area(self.cx, id, offset, data)
    }

    fn frames(&self) -> Vec<StopFrame> {
        let mut frames = Vec::new();
        let mut stmt = self.cx.stmt;
        for frame in abi_kernel::frames(self.cx) {
            frames.push(StopFrame {
                pou: frame.pou,
                stmt,
                instance: frame
                    .instance_address()
                    .and_then(|address| self.place_of(address)),
                locals: frame.has_locals(),
            });
            stmt = frame.call_stmt;
        }
        frames
    }

    fn read_frame(&self, frame: usize, offset: usize, out: &mut [u8]) -> bool {
        abi_kernel::frames(self.cx).nth(frame).is_some_and(|view| {
            self.frame_size(view.pou)
                .is_some_and(|size| view.read(size, offset, out))
        })
    }

    fn write_frame(&mut self, frame: usize, offset: usize, data: &[u8]) -> bool {
        abi_kernel::frames(self.cx).nth(frame).is_some_and(|view| {
            self.frame_size(view.pou)
                .is_some_and(|size| view.write(size, offset, data))
        })
    }

    /// The process images of a held scan are the areas the module reads (`%I`, sampled before the
    /// scan), writes (`%Q`, published after it) and shares (`%M`).
    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        let area = match address.area {
            IoArea::Input => trust_plc_abi::AREA_INPUTS,
            IoArea::Output => trust_plc_abi::AREA_OUTPUTS,
            IoArea::Memory => trust_plc_abi::AREA_MARKERS,
        };
        let len = abi_kernel::area_address(self.cx, area).map_or(0, |(_, len)| len);
        let mut bytes = alloc::vec![0u8; len];
        if !abi_kernel::read_area(self.cx, area, 0, &mut bytes) {
            return Err(RuntimeError::IoAddress(
                "the address lies outside the program's image".into(),
            ));
        }
        write_flat_image(&mut bytes, address, value)?;
        if abi_kernel::write_area(self.cx, area, 0, &bytes) {
            Ok(())
        } else {
            Err(RuntimeError::IoAddress(
                "the address lies outside the program's image".into(),
            ))
        }
    }
}

/// Sets the bit of every statement `wanted` takes, every bit when `every` is set, and clears the
/// rest; returns whether any bit is set.
pub(super) fn arm_bits(bits: &[Cell<u8>], every: bool, wanted: &dyn Fn(u32) -> bool) -> bool {
    let mut any = false;
    for (byte_index, byte) in bits.iter().enumerate() {
        let mut value = 0u8;
        for bit in 0..8u32 {
            let statement = u32::try_from(byte_index)
                .ok()
                .and_then(|index| index.checked_mul(8))
                .and_then(|base| base.checked_add(bit));
            if every || statement.is_some_and(wanted) {
                value |= 1 << bit;
            }
        }
        any |= value != 0;
        byte.set(value);
    }
    any
}

/// The kernel's sink for the stops of one call: it hands each to the scan's hooks, and keeps the
/// error a hook ended the scan with, which the call reports in place of the module's fault.
pub(super) struct HeldScan<'h, 'i> {
    pub(super) hooks: &'h mut dyn ScanHooks,
    pub(super) image: Image<'i>,
    pub(super) bits: &'h [Cell<u8>],
    pub(super) error: Option<RuntimeError>,
}

impl StopSink for HeldScan<'_, '_> {
    fn stop(&mut self, cx: &mut Cx, _stmt: u32) -> u32 {
        let mut stop = Stop::new(cx, self.image, self.bits);
        match self.hooks.on_stop(&mut stop) {
            Ok(()) => 0,
            Err(error) => {
                self.error = Some(error);
                FaultCode::ModuleInternal as u32
            }
        }
    }
}
