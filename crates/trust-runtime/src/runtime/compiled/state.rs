//! The state of a compiled program as a scan left it, or as the debugger holds it, which a
//! computer's other threads read (specification 44 §8.8, §13.2, §13.3): a copy of the program's areas
//! with the image they belong to, and of a held scan's call stack, read through the image's symbol
//! and type tables. Nothing in it decodes a variable it is not asked for.

use std::sync::Arc;

use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::memory::{pou_symbol, ProgramMemory, StopFrame};
use trust_runtime_core::native::program::{AdmittedImage, ModuleCode, NativeProgram};

use crate::error::RuntimeError;
use crate::value::{Duration, Value};

/// A copy of a compiled program's state at the end of a scan, or of a held scan.
#[derive(Clone)]
pub struct CompiledState {
    image: Arc<AdmittedImage>,
    /// The bytes of areas 1 to 7, in their order.
    areas: [Vec<u8>; 7],
    /// The held scan's call stack, innermost frame first, each with the bytes of its frame
    /// variables; empty between scans.
    frames: Vec<(StopFrame, Vec<u8>)>,
    now: Duration,
}

impl std::fmt::Debug for CompiledState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledState")
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

impl CompiledState {
    /// The state of `program` at the logical time `now`: a copy of each of its areas.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] for a program that does not keep its image, as a
    /// device's does.
    pub fn of<C: ModuleCode>(
        program: &NativeProgram<'static, C>,
        now: Duration,
    ) -> Result<Self, RuntimeError> {
        let image = program
            .kept_image()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(Self::copied(image, program, Vec::new(), now))
    }

    /// The state of the scan the debugger holds, whose memory is `memory`, of the program of
    /// `image`: its areas and the frames of its call stack.
    #[must_use]
    pub fn held(image: Arc<AdmittedImage>, memory: &dyn ProgramMemory, now: Duration) -> Self {
        let frames = memory
            .frames()
            .into_iter()
            .enumerate()
            .map(|(index, frame)| {
                let size = if frame.locals {
                    pou_symbol(&image.image(), frame.pou)
                        .map_or(0, |(_, symbol)| symbol.size as usize)
                } else {
                    0
                };
                let mut bytes = vec![0; size];
                if !memory.read_frame(index, 0, &mut bytes) {
                    bytes.clear();
                }
                (frame, bytes)
            })
            .collect();
        Self::copied(image, memory, frames, now)
    }

    /// The copy of the areas of `memory`, of the program of `image`, with `frames`.
    fn copied(
        image: Arc<AdmittedImage>,
        memory: &dyn ProgramMemory,
        frames: Vec<(StopFrame, Vec<u8>)>,
        now: Duration,
    ) -> Self {
        let areas = std::array::from_fn(|index| {
            let id = u8::try_from(index + 1).unwrap_or(u8::MAX);
            let length = image
                .image()
                .area_length(id)
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or(0);
            let mut bytes = vec![0; length];
            if !memory.read_area(id, 0, &mut bytes) {
                bytes.clear();
            }
            bytes
        });
        Self {
            image,
            areas,
            frames,
            now,
        }
    }

    /// The logical time of the scan the state is of.
    #[must_use]
    pub fn now(&self) -> Duration {
        self.now
    }

    /// The value of the variable `name` names, as the test harness resolves a name: a global, a
    /// program's variable `Program.var`, or a unique unqualified name; `None` for a name that names
    /// no single variable or a variable that holds no value.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<Value> {
        let image = self.image();
        let symbol = image.find_variable(name).ok()?;
        let (place, type_index) =
            trust_runtime_core::native::memory::symbol_place(&image, symbol).ok()?;
        trust_runtime_core::native::memory::read_place(self, place, type_index).ok()
    }
}

impl ProgramMemory for CompiledState {
    fn image(&self) -> Image<'_> {
        self.image.image()
    }

    fn read_area(&self, id: u8, offset: usize, out: &mut [u8]) -> bool {
        let Some(area) = usize::from(id)
            .checked_sub(1)
            .and_then(|index| self.areas.get(index))
        else {
            return false;
        };
        match offset
            .checked_add(out.len())
            .and_then(|end| area.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    /// The state is a copy for reading: a write reaches the program through the Live Values queue,
    /// never through the copy.
    fn write_area(&mut self, _id: u8, _offset: usize, _data: &[u8]) -> bool {
        false
    }

    fn frames(&self) -> Vec<StopFrame> {
        self.frames.iter().map(|(frame, _)| *frame).collect()
    }

    fn read_frame(&self, frame: usize, offset: usize, out: &mut [u8]) -> bool {
        let Some((_, bytes)) = self.frames.get(frame) else {
            return false;
        };
        match offset
            .checked_add(out.len())
            .and_then(|end| bytes.get(offset..end))
        {
            Some(bytes) => {
                out.copy_from_slice(bytes);
                true
            }
            None => false,
        }
    }

    fn write_image(
        &mut self,
        _address: &trust_runtime_core::io_address::IoAddress,
        _value: Value,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
}
