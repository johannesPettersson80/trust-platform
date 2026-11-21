//! The Rust of the example `rust_signal`: two blocks of signal handling, where loops, arrays and bit
//! operations are plainer and safer in Rust than in Structured Text. `MEDIAN5` gives the median of a
//! sensor's last five readings, which drops a lone spike that an average would smear over five
//! readings; `MODBUS_CRC` checks a Modbus RTU frame's CRC before the program trusts its data.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// The median of the last five readings of `x`.
#[trust_fb(name = "MEDIAN5")]
#[budget("2us")]
pub struct Median5 {
    /// The reading.
    #[input]
    pub x: f32,
    /// The median of the last five readings; at the first call, the first reading.
    #[output]
    pub y: f32,
    /// Their average, for comparison: a spike moves it by a fifth of its size.
    #[output]
    pub mean: f32,
    /// The last five readings, a ring.
    history: Array<f32, 0, 5>,
    /// The ring's place for the next reading.
    next: i16,
    /// FALSE until the first reading has filled the ring.
    primed: bool,
}

impl Body for Median5 {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if !self.primed {
            self.history = Array::filled(self.x);
            self.primed = true;
        } else if let Some(slot) = self.history.get_mut(i64::from(self.next)) {
            *slot = self.x;
        }
        self.next = if self.next >= 4 { 0 } else { self.next + 1 };
        if let &[a, b, c, d, e] = self.history.as_slice() {
            self.y = median([a, b, c, d, e]);
            self.mean = (a + b + c + d + e) / 5.0;
        }
        Ok(())
    }
}

/// The median of five readings, by a sorting network of nine exchanges: no index, so nothing that
/// could panic.
fn median([mut a, mut b, mut c, mut d, mut e]: [f32; 5]) -> f32 {
    fn order(low: &mut f32, high: &mut f32) {
        if *low > *high {
            core::mem::swap(low, high);
        }
    }
    order(&mut a, &mut b);
    order(&mut d, &mut e);
    order(&mut c, &mut e);
    order(&mut c, &mut d);
    order(&mut a, &mut d);
    order(&mut a, &mut c);
    order(&mut b, &mut e);
    order(&mut b, &mut d);
    order(&mut b, &mut c);
    c
}

/// Checks a Modbus RTU frame: its last two bytes, low byte first, must be the CRC-16 of the bytes
/// before them (Modbus over Serial Line, §6.2.2: polynomial 0xA001 reflected, starting at 0xFFFF).
#[trust_fb(name = "MODBUS_CRC")]
#[budget("5us")]
pub struct ModbusCrc {
    /// The frame's bytes, its CRC last.
    #[input]
    pub frame: Array<Byte, 0, 32>,
    /// How many of `frame`'s bytes the frame has, its CRC's two included.
    #[input]
    pub length: i16,
    /// The CRC of the frame's bytes before its last two.
    #[output]
    pub crc: Word,
    /// TRUE when the frame's last two bytes are that CRC.
    #[output]
    pub ok: bool,
}

impl Body for ModbusCrc {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        let frame = usize::try_from(self.length)
            .ok()
            .and_then(|length| self.frame.as_slice().get(..length));
        let parts = frame.and_then(|bytes| bytes.split_at_checked(bytes.len().checked_sub(2)?));
        match parts {
            Some((data, [low, high])) => {
                let crc = crc16(data.iter().map(|byte| byte.0));
                self.crc = Word(crc);
                self.ok = crc == u16::from(low.0) | (u16::from(high.0) << 8);
            }
            _ => {
                self.crc = Word(0);
                self.ok = false;
            }
        }
        Ok(())
    }
}

/// The CRC-16 of Modbus RTU over `bytes`.
fn crc16(bytes: impl Iterator<Item = u8>) -> u16 {
    bytes.fold(0xFFFF, |crc, byte| {
        (0..8).fold(crc ^ u16::from(byte), |crc, _| {
            if crc & 1 == 1 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame of the README: slave 1, function 3 (read holding registers), 2 bytes, the register
    /// value 42, and its CRC 0x9B39, low byte first.
    const FRAME: [u8; 7] = [0x01, 0x03, 0x02, 0x00, 0x2A, 0x39, 0x9B];

    #[test]
    fn the_crc_is_modbus_s() {
        // The check value of CRC-16/MODBUS, the CRC of the nine digits "123456789", is 0x4B37, and
        // the frame's five bytes give 0x9B39. Catches another polynomial, start value or bit order.
        assert_eq!(crc16(b"123456789".iter().copied()), 0x4B37);
        assert_eq!(crc16(FRAME.iter().copied().take(5)), 0x9B39);
    }

    /// `MODBUS_CRC` called on `bytes`.
    fn check(bytes: &[u8]) -> (u16, bool) {
        let mut block = ModbusCrc::new();
        for (slot, byte) in block.frame.as_mut_slice().iter_mut().zip(bytes) {
            *slot = Byte(*byte);
        }
        block.length = i16::try_from(bytes.len()).expect("a short frame");
        block.body(&mut trust_plc::test::cx()).expect("the call");
        (block.crc.0, block.ok)
    }

    #[test]
    fn a_frame_is_trusted_only_with_its_crc() {
        // The README's frame passes; with the register's low byte changed from 0x2A to 0x2B its CRC
        // is 0x5BF8, not the 0x9B39 it carries; a frame of fewer than two bytes has no CRC. Catches
        // the CRC's bytes read high first, and a check that trusts a changed frame.
        assert_eq!(check(&FRAME), (0x9B39, true));
        let mut changed = FRAME;
        changed[4] = 0x2B;
        assert_eq!(check(&changed), (0x5BF8, false));
        assert_eq!(check(&[0x01]), (0, false));
    }

    #[test]
    fn the_median_drops_a_lone_spike() {
        // Five readings around 50 with one spike of 80: sorted, the middle one is 50.1. Catches an
        // average, which would give 280 / 5 = 56.0, and a network that leaves the five unsorted.
        assert_eq!(median([50.2, 49.8, 80.0, 50.1, 49.9]), 50.1);
        assert_eq!(median([20.0, 50.0, 50.1, 49.9, 50.2]), 50.0);
        assert_eq!(median([5.0, 4.0, 3.0, 2.0, 1.0]), 3.0);
    }

    #[test]
    fn the_average_moves_with_a_spike_and_the_median_does_not() {
        // The block's two outputs over the same five readings: the spike of 80 moves the average to
        // 280 / 5 = 56.0, a fifth of the spike's 30 above the others' 50, and leaves the median at
        // 50.1. Catches outputs swapped, and an average over fewer than five readings.
        let mut cx = trust_plc::test::cx();
        let mut block = Median5::new();
        for x in [50.2, 49.8, 80.0, 50.1, 49.9] {
            block.x = x;
            block.body(&mut cx).expect("the call");
        }
        assert_eq!((block.y, block.mean), (50.1, 56.0));
    }

    #[test]
    fn the_block_keeps_the_last_five_readings() {
        // The first reading fills the ring, so the median is that reading; each next one replaces the
        // oldest. Catches a ring that starts at zeros, which would give 0 for four calls.
        let mut cx = trust_plc::test::cx();
        let mut block = Median5::new();
        let mut medians = [0.0; 6];
        for (median, x) in medians.iter_mut().zip([50.0, 80.0, 50.2, 49.8, 49.9, 50.1]) {
            block.x = x;
            block.body(&mut cx).expect("the call");
            *median = block.y;
        }
        // The ring after each call, from slot 0: [50 x5] -> 50; [50,80,50,50,50] -> 50;
        // [50,80,50.2,50,50] -> 50; [50,80,50.2,49.8,50] -> 50; [50,80,50.2,49.8,49.9] -> 50; and
        // [50.1,80,50.2,49.8,49.9], the last five readings -> 50.1, while the spike of 80 is still in it.
        assert_eq!(medians, [50.0, 50.0, 50.0, 50.0, 50.0, 50.1]);
    }
}
