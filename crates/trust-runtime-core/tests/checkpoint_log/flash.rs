//! A simulated checkpoint flash with a chip's program and erase units. Erasing sets 0xFF; a
//! program writes whole units and ANDs each byte, so it never sets a bit back to 1. A power cut
//! can stop any program or erase at any byte, and the flash then refuses every operation until it
//! is powered again.

use trust_runtime_core::retain::checkpoint::{CheckpointFlash, FlashError, FlashGeometry, Slot};

/// What an erase cut by a power loss leaves after the byte where it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseCut {
    /// The bytes not yet erased keep their old values.
    LeaveOld,
    /// The bytes not yet erased read 0x00, as after the pre-program phase some flash runs first.
    Zero,
}

/// Where power is cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    /// The bytes programmed or erased in full before the cut.
    pub after: u64,
    /// Whether the byte at the cut is half done: half of the bits a program clears are cleared,
    /// or half of its bits are erased.
    pub partial: bool,
    /// What an erase leaves after the cut.
    pub erase: EraseCut,
}

/// One program or erase, as the flash saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Op {
    /// An erase rather than a program.
    pub erase: bool,
    pub slot: Slot,
    pub offset: u32,
    pub len: u32,
}

#[derive(Debug, Clone)]
pub struct SimFlash {
    geometry: FlashGeometry,
    /// Slot A, then slot B.
    bytes: Vec<u8>,
    cut: Option<Cut>,
    /// The bytes programmed or erased since the cut was set.
    spent: u64,
    powered: bool,
    /// A byte whose bits in the mask never program: `(slot, offset, mask)`.
    stuck: Option<(Slot, u32, u8)>,
    /// A byte that reads 0x00 once this many more programs have completed:
    /// `(programs, slot, offset)`.
    disturbance: Option<(usize, Slot, u32)>,
    recording: bool,
    ops: Vec<Op>,
    erases: [u32; 2],
}

impl SimFlash {
    /// A blank flash: both slots erased.
    pub fn new(geometry: FlashGeometry) -> Self {
        Self {
            geometry,
            bytes: vec![0xFF; 2 * geometry.slot_len() as usize],
            cut: None,
            spent: 0,
            powered: true,
            stuck: None,
            disturbance: None,
            recording: false,
            ops: Vec::new(),
            erases: [0; 2],
        }
    }

    /// Cut power at `cut`, counting from now.
    pub fn cut_at(&mut self, cut: Cut) {
        self.cut = Some(cut);
        self.spent = 0;
    }

    /// Power the flash again after a cut.
    pub fn power_cycle(&mut self) {
        self.cut = None;
        self.powered = true;
    }

    pub fn powered(&self) -> bool {
        self.powered
    }

    /// Record every program and erase from now.
    pub fn record_operations(&mut self) {
        self.recording = true;
        self.ops.clear();
    }

    pub fn operations(&self) -> &[Op] {
        &self.ops
    }

    /// The erases started in `slot`.
    pub fn erases(&self, slot: Slot) -> u32 {
        self.erases[index(slot)]
    }

    /// Make the bits of `mask` in one byte refuse to program.
    pub fn stick(&mut self, slot: Slot, offset: u32, mask: u8) {
        self.stuck = Some((slot, offset, mask));
    }

    pub fn unstick(&mut self) {
        self.stuck = None;
    }

    /// Make one byte read 0x00, as if disturbed, once `programs` more programs have completed.
    pub fn disturb_after(&mut self, programs: usize, slot: Slot, offset: u32) {
        self.disturbance = Some((programs, slot, offset));
    }

    /// The bytes of `slot`.
    pub fn slot(&self, slot: Slot) -> &[u8] {
        &self.bytes[self.span(slot, 0, self.geometry.slot_len() as usize)]
    }

    /// Damage one byte after it was written: flip the bits of `mask`.
    pub fn flip(&mut self, slot: Slot, offset: u32, mask: u8) {
        let at = self.span(slot, offset, 1).start;
        self.bytes[at] ^= mask;
    }

    fn span(&self, slot: Slot, offset: u32, len: usize) -> core::ops::Range<usize> {
        let slot_len = self.geometry.slot_len() as usize;
        let end = offset as usize + len;
        assert!(
            end <= slot_len,
            "{len} bytes at {offset} run past slot {slot:?}"
        );
        let base = index(slot) * slot_len;
        base + offset as usize..base + end
    }

    fn whole_units(&self, offset: u32, len: usize) -> bool {
        let unit = self.geometry.program_unit();
        offset.is_multiple_of(unit) && (len as u32).is_multiple_of(unit)
    }

    fn note(&mut self, op: Op) {
        if self.recording {
            self.ops.push(op);
        }
    }

    /// The cut, when power fails before the next byte; otherwise the byte is counted.
    fn cut_before_next_byte(&mut self) -> Option<Cut> {
        let cut = self.cut?;
        if self.spent == cut.after {
            return Some(cut);
        }
        self.spent += 1;
        None
    }
}

impl CheckpointFlash for SimFlash {
    fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        if !self.powered {
            return Err(FlashError::Read);
        }
        assert!(
            self.whole_units(offset, bytes.len()),
            "read of {} bytes at {offset} is not whole program units",
            bytes.len()
        );
        bytes.copy_from_slice(&self.bytes[self.span(slot, offset, bytes.len())]);
        Ok(())
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        if !self.powered {
            return Err(FlashError::Program);
        }
        assert!(
            self.whole_units(offset, bytes.len()),
            "program of {} bytes at {offset} is not whole program units",
            bytes.len()
        );
        let span = self.span(slot, offset, bytes.len());
        assert!(
            self.bytes[span.clone()].iter().all(|&byte| byte == 0xFF),
            "the log programmed over data: {} bytes at {offset} in slot {slot:?}",
            bytes.len()
        );
        self.note(Op {
            erase: false,
            slot,
            offset,
            len: bytes.len() as u32,
        });
        for (index, &value) in bytes.iter().enumerate() {
            let at = span.start + index;
            let old = self.bytes[at];
            let mut new = old & value;
            if let Some((stuck_slot, stuck_offset, mask)) = self.stuck
                && stuck_slot == slot
                && stuck_offset as usize == offset as usize + index
            {
                new |= old & mask;
            }
            if let Some(cut) = self.cut_before_next_byte() {
                if cut.partial {
                    self.bytes[at] = old & !((old & !new) & 0x55);
                }
                self.powered = false;
                return Err(FlashError::Program);
            }
            self.bytes[at] = new;
        }
        match self.disturbance {
            Some((0 | 1, slot, offset)) => {
                let at = self.span(slot, offset, 1).start;
                self.bytes[at] = 0x00;
                self.disturbance = None;
            }
            Some((programs, slot, offset)) => {
                self.disturbance = Some((programs - 1, slot, offset));
            }
            None => {}
        }
        Ok(())
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        if !self.powered {
            return Err(FlashError::Erase);
        }
        let unit = self.geometry.erase_unit();
        assert!(
            offset.is_multiple_of(unit),
            "erase at {offset} is not an erase unit"
        );
        let span = self.span(slot, offset, unit as usize);
        self.note(Op {
            erase: true,
            slot,
            offset,
            len: unit,
        });
        self.erases[index(slot)] += 1;
        let left = self.cut.map_or(u64::MAX, |cut| cut.after - self.spent);
        if left >= u64::from(unit) {
            self.bytes[span].fill(0xFF);
            self.spent += u64::from(unit);
            return Ok(());
        }
        let Some(cut) = self.cut else {
            unreachable!("only a cut stops an erase");
        };
        let at = span.start + left as usize;
        self.bytes[span.start..at].fill(0xFF);
        let mut rest = at;
        if cut.partial {
            self.bytes[at] |= 0x55;
            rest += 1;
        }
        if cut.erase == EraseCut::Zero {
            self.bytes[rest..span.end].fill(0x00);
        }
        self.powered = false;
        Err(FlashError::Erase)
    }
}

fn index(slot: Slot) -> usize {
    match slot {
        Slot::A => 0,
        Slot::B => 1,
    }
}
