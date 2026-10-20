//! The register file of the register tier (specification 34 ARCH-05; checklist C1-20).
//!
//! An operand stack serves as one activation's registers: its bottom operands are the registers,
//! addressed by index, and the operands above them are those of a call the activation makes. A
//! register holds the stack's operand forms (a value, a place, a temporary text, an enumeration or a
//! copy), so a load of a compound leaves it where it lies and a text keeps its characters in the
//! stack's temporary area, which ends at its topmost live temporary as the stack's does (C1-19c2).
//! A register's lifetime nests as the stack operand's it was lowered from does, so the area needs
//! no more room than the body's operand stack.

use super::*;

impl OperandStack {
    /// Make the stack a register file of `count` registers, each the null value, with no operand
    /// above them. The slots it keeps hold the null value already: the file is new, or its
    /// buffers' clear set them so ([`Self::clear_registers`]); a reset touches no slot (C1-20).
    pub(crate) fn reset_registers(&mut self, count: usize) -> Result<(), VmTrap> {
        debug_assert!(
            self.values
                .iter()
                .all(|operand| matches!(operand, Operand::Value(Value::Null))),
            "a register file is reset only once it is cleared"
        );
        self.values.truncate(count);
        if self.values.len() < count {
            self.values
                .try_reserve(count - self.values.len())
                .map_err(|_| VmTrap::BudgetExceeded)?;
            self.values
                .resize_with(count, || Operand::Value(Value::Null));
        }
        self.floor = count;
        self.places = 0;
        self.texts = 0;
        self.enums = 0;
        self.copies = 0;
        self.temp_bytes.clear();
        Ok(())
    }

    /// The operand register `index` holds.
    pub(crate) fn register(&self, index: usize) -> Result<&Operand, VmTrap> {
        self.values[..self.floor]
            .get(index)
            .ok_or(VmTrap::Runtime(RuntimeError::InvalidExecutionState))
    }

    /// The value register `index` holds, when it holds a value.
    pub(crate) fn register_value(&self, index: usize) -> Result<Option<&Value>, VmTrap> {
        Ok(match self.register(index)? {
            Operand::Value(value) => Some(value),
            _ => None,
        })
    }

    /// Put `operand` into register `index`. The room of the text or the copy the register held is
    /// given back.
    pub(crate) fn set_register(&mut self, index: usize, operand: Operand) -> Result<(), VmTrap> {
        let Some(slot) = self.values[..self.floor].get_mut(index) else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        };
        // A value over a value, the most common write, counts no form.
        if matches!((&*slot, &operand), (Operand::Value(_), Operand::Value(_))) {
            *slot = operand;
            return Ok(());
        }
        let old = core::mem::replace(slot, operand);
        self.count_in_at(index);
        if self.count_out(&old) {
            self.forget_temps();
        }
        Ok(())
    }

    /// Put the value `value` into register `index`: a value over a value is written in place
    /// (C1-20), any other operand as [`Self::set_register`] puts it.
    pub(crate) fn write_register_value(
        &mut self,
        index: usize,
        value: Value,
    ) -> Result<(), VmTrap> {
        let Some(slot) = self.values[..self.floor].get_mut(index) else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        };
        if let Operand::Value(slot) = slot {
            *slot = value;
            return Ok(());
        }
        self.set_register(index, Operand::Value(value))
    }

    /// Take the value register `index` holds out, leaving the null value, when it holds a value:
    /// the last read of a value (C1-20). `None`, the register untouched, for any other operand.
    pub(crate) fn take_register_value(&mut self, index: usize) -> Option<Value> {
        match self.values[..self.floor].get_mut(index) {
            Some(Operand::Value(slot)) => Some(core::mem::replace(slot, Value::Null)),
            _ => None,
        }
    }

    /// Take register `index`'s operand out, leaving the null value: its last read. A text's
    /// characters and a copy's bytes stay readable until [`Self::release_temps`] or the next text or
    /// copy is pushed.
    pub(crate) fn take_register(&mut self, index: usize) -> Result<Operand, VmTrap> {
        if index >= self.floor {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        let operand = core::mem::replace(&mut self.values[index], Operand::Value(Value::Null));
        self.count_out(&operand);
        Ok(operand)
    }

    /// A second operand for register `index`, which the reader keeps: a value cloned, a place, an
    /// enumeration as they are, and a text's characters or a copy's bytes copied to the top of the
    /// temporary area (as `duplicate_text` copies a text, C1-12).
    pub(crate) fn duplicate_register(&mut self, index: usize) -> Result<Operand, VmTrap> {
        Ok(match self.register(index)? {
            Operand::Value(value) => Operand::Value(value.clone()),
            Operand::Place(place) => Operand::Place(*place),
            Operand::Enum(temp) => Operand::Enum(*temp),
            Operand::Text(temp) => {
                let temp = *temp;
                Operand::Text(TextTemp {
                    start: self.copy_temp(temp.start, temp.len)?,
                    len: temp.len,
                    wide: temp.wide,
                })
            }
            Operand::Copy(temp) => {
                let temp = *temp;
                Operand::Copy(CopyTemp {
                    start: self.copy_temp(temp.start, temp.len)?,
                    len: temp.len,
                    type_id: temp.type_id,
                })
            }
        })
    }

    /// Put `text`, a `WSTRING` when `wide`, into register `index`, its characters at the top of the
    /// temporary area (as `push_text` pushes one, C1-12).
    pub(crate) fn set_register_text(
        &mut self,
        index: usize,
        text: &str,
        wide: bool,
    ) -> Result<(), VmTrap> {
        if index >= self.floor {
            return Err(VmTrap::Runtime(RuntimeError::InvalidExecutionState));
        }
        self.forget_temps();
        let start = self.temp_bytes.len();
        self.temp_bytes
            .try_reserve(text.len())
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.temp_bytes.extend_from_slice(text.as_bytes());
        let temp = TextTemp {
            start: u32::try_from(start).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            len: u32::try_from(text.len()).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
            wide,
        };
        self.set_register(index, Operand::Text(temp))
    }

    /// The value of the temporary text `temp`, a `STRING` or a `WSTRING` holding its characters.
    pub(crate) fn text_value(&self, temp: TextTemp) -> Result<Value, VmTrap> {
        text_value(&self.temp_bytes, temp)
    }

    /// Give back the room of the texts and copies no operand stands for any more: the area ends at
    /// its topmost live temporary.
    pub(crate) fn release_temps(&mut self) {
        self.forget_temps();
    }

    /// Set every register to the null value and remove the operands above them, keeping the room.
    pub(crate) fn clear_registers(&mut self) {
        let count = self.floor;
        self.truncate(count);
        for operand in &mut self.values {
            if !matches!(operand, Operand::Value(Value::Null)) {
                *operand = Operand::Value(Value::Null);
            }
        }
        self.places = 0;
        self.texts = 0;
        self.enums = 0;
        self.copies = 0;
        self.temp_bytes.clear();
    }

    /// A register file holding `values` (test support).
    #[cfg(any(test, feature = "test-support"))]
    pub fn from_register_values(values: impl IntoIterator<Item = Value>) -> Self {
        let mut stack = Self::default();
        stack.values.extend(values.into_iter().map(Operand::Value));
        stack.floor = stack.values.len();
        stack
    }

    /// The registers' values; a register that holds a place, a text, an enumeration or a copy shows
    /// the null value (test support).
    #[cfg(any(test, feature = "test-support"))]
    pub fn register_values(&self) -> Vec<Value> {
        self.values[..self.floor]
            .iter()
            .map(|operand| match operand {
                Operand::Value(value) => value.clone(),
                _ => Value::Null,
            })
            .collect()
    }

    /// Put `value` into register `index` (test support).
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_register_value(&mut self, index: usize, value: Value) {
        let _ = self.set_register(index, Operand::Value(value));
    }

    /// The most operands, registers and a call's together, the stack holds without growing.
    pub(crate) fn register_capacity(&self) -> usize {
        self.values.capacity()
    }

    /// Make every place among the registers but register `keep` a copy, in the temporary area, of
    /// the bytes `bytes_of` gives for it: before an instruction that writes a block or calls, so that
    /// a place keeps the bytes its load read (C1-19b). A place `bytes_of` gives no bytes for stays.
    pub(crate) fn detach_register_places<'b>(
        &mut self,
        keep: Option<usize>,
        mut bytes_of: impl FnMut(Place) -> Option<&'b [u8]>,
    ) -> Result<(), VmTrap> {
        if self.places == 0 {
            return Ok(());
        }
        self.forget_temps();
        for index in 0..self.floor {
            if Some(index) == keep {
                continue;
            }
            let Operand::Place(place) = self.values[index] else {
                continue;
            };
            let Some(bytes) = bytes_of(place) else {
                continue;
            };
            let start = self.temp_bytes.len();
            let copy = CopyTemp {
                start: u32::try_from(start).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
                len: u32::try_from(bytes.len())
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))?,
                type_id: place.type_id,
            };
            self.temp_bytes
                .try_reserve(bytes.len())
                .map_err(|_| VmTrap::BudgetExceeded)?;
            self.temp_bytes.extend_from_slice(bytes);
            self.values[index] = Operand::Copy(copy);
            self.places -= 1;
            self.copies += 1;
        }
        Ok(())
    }

    /// Remove the operands above the registers, a call's that it left.
    pub(crate) fn truncate_to_registers(&mut self) {
        self.truncate(self.floor);
    }

    /// Read the enumerations and the copies above the registers, and the places too when `places`,
    /// into the values they stand for, as the stack's call reads its operands (C1-19a, C1-19b):
    /// `place_value`, `enum_value` and `copy_value` read them. A copy's room is given back.
    ///
    /// # Errors
    ///
    /// The first error of a read; the operands before it are values then, and the rest stay.
    pub(crate) fn read_call_operands(
        &mut self,
        places: bool,
        mut place_value: impl FnMut(Place) -> Result<Value, RuntimeError>,
        mut enum_value: impl FnMut(EnumTemp) -> Result<Value, RuntimeError>,
        mut copy_value: impl FnMut(&[u8], u32) -> Result<Value, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let mut released = false;
        for index in self.floor..self.values.len() {
            let value = match &self.values[index] {
                Operand::Place(place) if places => place_value(*place)?,
                Operand::Enum(temp) => enum_value(*temp)?,
                Operand::Copy(temp) => {
                    let temp = *temp;
                    let bytes = self
                        .copy_bytes(temp)
                        .map_err(|_| RuntimeError::InvalidExecutionState)?;
                    copy_value(bytes, temp.type_id)?
                }
                _ => continue,
            };
            let operand = core::mem::replace(&mut self.values[index], Operand::Value(value));
            released |= self.count_out(&operand);
        }
        if released {
            self.forget_temps();
        }
        Ok(())
    }

    /// Push `operand` above the registers, counting its form: an argument of a call.
    pub(crate) fn push_any(&mut self, operand: Operand) -> Result<(), VmTrap> {
        if self.values.len() - self.floor >= VM_MAX_OPERAND_STACK {
            return Err(VmTrap::StackOverflow);
        }
        self.count_in(&operand);
        self.values.push(operand);
        Ok(())
    }

    /// Copy `len` bytes from byte `start` of the temporary area to its top; where the copy starts.
    fn copy_temp(&mut self, start: u32, len: u32) -> Result<u32, VmTrap> {
        let from = start as usize;
        let to = from
            .checked_add(len as usize)
            .filter(|end| *end <= self.temp_bytes.len())
            .ok_or(VmTrap::Runtime(RuntimeError::InvalidExecutionState))?;
        let at = self.temp_bytes.len();
        self.temp_bytes
            .try_reserve(to - from)
            .map_err(|_| VmTrap::BudgetExceeded)?;
        self.temp_bytes.extend_from_within(from..to);
        u32::try_from(at).map_err(|_| VmTrap::Runtime(RuntimeError::Overflow))
    }

    /// Count the form of the operand at `index` in.
    fn count_in_at(&mut self, index: usize) {
        match self.values[index] {
            Operand::Place(_) => self.places += 1,
            Operand::Text(_) => self.texts += 1,
            Operand::Enum(_) => self.enums += 1,
            Operand::Copy(_) => self.copies += 1,
            Operand::Value(_) => {}
        }
    }

    /// Count `operand`'s form in.
    fn count_in(&mut self, operand: &Operand) {
        match operand {
            Operand::Place(_) => self.places += 1,
            Operand::Text(_) => self.texts += 1,
            Operand::Enum(_) => self.enums += 1,
            Operand::Copy(_) => self.copies += 1,
            Operand::Value(_) => {}
        }
    }

    /// Count `operand`'s form out; whether it held room in the temporary area.
    fn count_out(&mut self, operand: &Operand) -> bool {
        match operand {
            Operand::Place(_) => self.places -= 1,
            Operand::Text(_) => {
                self.texts -= 1;
                return true;
            }
            Operand::Enum(_) => self.enums -= 1,
            Operand::Copy(_) => {
                self.copies -= 1;
                return true;
            }
            Operand::Value(_) => {}
        }
        false
    }
}
