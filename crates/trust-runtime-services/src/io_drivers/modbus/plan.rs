//! The requests of a raw exchange (specification 11 §6.5): without a point map, the driver reads the
//! whole input image from `input_start` and writes the whole output image from `output_start`, as
//! many coils or registers as the image takes, in requests of at most the quantity that V1.1b3
//! allows the function, in address order, within the one exchange of a cycle.

use alloc::format;
use alloc::string::String;
use core::ops::Range;

use trust_modbus::FunctionCode;

/// One past the last address: coils, inputs and registers have the addresses 0 to 65535 (V1.1b3
/// §6).
pub(super) const ADDRESS_END: u32 = 65_536;

/// How a function's quantity counts the image's bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    /// Eight coils or inputs a byte, the first in the least significant bit of the first byte
    /// (V1.1b3 §6.1, §6.2, §6.11).
    Bits,
    /// A register for every two bytes, its high byte first (V1.1b3 §4.2): `ceil(bytes / 2)`.
    Registers,
    /// One coil, the least significant bit of the first byte (FC05), or one register, the first
    /// two bytes (FC06).
    Single,
}

/// How `function` counts the image's bytes.
const fn unit(function: FunctionCode) -> Unit {
    match function {
        FunctionCode::ReadCoils
        | FunctionCode::ReadDiscreteInputs
        | FunctionCode::WriteMultipleCoils => Unit::Bits,
        FunctionCode::ReadHoldingRegisters
        | FunctionCode::ReadInputRegisters
        | FunctionCode::WriteMultipleRegisters => Unit::Registers,
        FunctionCode::WriteSingleCoil | FunctionCode::WriteSingleRegister => Unit::Single,
    }
}

/// What `function` reads or writes, in the words of a refusal.
const fn noun(function: FunctionCode) -> &'static str {
    match function {
        FunctionCode::ReadCoils | FunctionCode::WriteMultipleCoils => "coils",
        FunctionCode::WriteSingleCoil => "coil",
        FunctionCode::ReadDiscreteInputs => "discrete inputs",
        FunctionCode::ReadHoldingRegisters | FunctionCode::WriteMultipleRegisters => {
            "holding registers"
        }
        FunctionCode::WriteSingleRegister => "holding register",
        FunctionCode::ReadInputRegisters => "input registers",
    }
}

/// The requests that exchange an image with one function from one address: the coils or registers
/// that the image takes, `total` of them from `start`, in requests of at most the function's
/// quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RawPlan {
    /// The function of every request.
    pub(super) function: FunctionCode,
    /// The first request's address.
    start: u16,
    /// How many coils, inputs or registers the image takes.
    total: u32,
}

impl RawPlan {
    /// The plan of `function` from `start` for the `image` image (`input` or `output`) of `len`
    /// bytes: `len × 8` coils or inputs, `ceil(len / 2)` registers, or for FC05 and FC06 one coil
    /// or register when the image has a byte; none for an empty image.
    ///
    /// # Errors
    ///
    /// The reason, when the coils or registers run past address 65535 (specification 11 §6.5: such
    /// a range is refused at configuration).
    pub(super) fn new(
        function: FunctionCode,
        start: u16,
        len: usize,
        image: &str,
    ) -> Result<Self, String> {
        let bytes = u64::try_from(len).unwrap_or(u64::MAX);
        let total = match unit(function) {
            Unit::Bits => bytes.saturating_mul(8),
            Unit::Registers => bytes.div_ceil(2),
            Unit::Single => bytes.min(1),
        };
        let end = u64::from(start).saturating_add(total);
        match u32::try_from(total) {
            Ok(total) if end <= u64::from(ADDRESS_END) => Ok(Self {
                function,
                start,
                total,
            }),
            _ => Err(format!(
                "{total} {} from address {start} for the {len}-byte {image} image run past \
                 address 65535",
                noun(function)
            )),
        }
    }

    /// How many requests the plan sends: `ceil(total / max)`, where `max` is the function's
    /// greatest quantity.
    pub(super) fn requests(self) -> usize {
        let count = self.total.div_ceil(u32::from(self.function.max_quantity()));
        usize::try_from(count).unwrap_or(usize::MAX)
    }

    /// The address and the quantity of request `index`, in address order: each but the last takes
    /// the function's greatest quantity. `None` past the last request.
    pub(super) fn request(self, index: usize) -> Option<(u16, u16)> {
        let max = u32::from(self.function.max_quantity());
        let first = u32::try_from(index).ok()?.checked_mul(max)?;
        let left = self.total.checked_sub(first).filter(|left| *left > 0)?;
        let address = u16::try_from(u32::from(self.start).checked_add(first)?).ok()?;
        let quantity = u16::try_from(left.min(max)).ok()?;
        Some((address, quantity))
    }

    /// The image bytes of request `index`: 250 bytes a request for FC01 to FC04 (2,000 bits or 125
    /// registers), 246 for FC15 and FC16 (1,968 bits or 123 registers), the first byte for FC05 and
    /// the first two for FC06. The last register of an image of an odd length ends one byte past
    /// the image. Empty past the last request.
    pub(super) fn bytes(self, index: usize) -> Range<usize> {
        let Some((_, quantity)) = self.request(index) else {
            return 0..0;
        };
        let max = usize::from(self.function.max_quantity());
        let quantity = usize::from(quantity);
        let (first, count) = match unit(self.function) {
            Unit::Bits => (index * (max / 8), quantity / 8),
            Unit::Registers => (index * max * 2, quantity * 2),
            Unit::Single if self.function == FunctionCode::WriteSingleRegister => (0, 2),
            Unit::Single => (0, 1),
        };
        first..first + count
    }
}
