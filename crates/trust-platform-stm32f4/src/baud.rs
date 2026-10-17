//! Baud-rate divisor of the console USART (RM0090, USART_BRR): pure arithmetic, so the host can
//! test what the F407 console writes into the register.

/// Console speed: 230400 baud, 8N1.
pub const CONSOLE_BAUD: u32 = 230_400;

/// The divisor of USART_BRR, a 12.4 fixed-point number: USARTDIV = mantissa + fraction / 16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Divisor {
    /// The integer part, 12 bits.
    pub mantissa: u16,
    /// The fractional part in sixteenths, 4 bits.
    pub fraction: u8,
}

impl Divisor {
    /// The whole register value, `USARTDIV * 16`.
    pub const fn raw(self) -> u16 {
        (self.mantissa << 4) | (self.fraction as u16)
    }

    /// The baud rate this divisor gives on a bus clocked at `pclk_hz` (16 times oversampling);
    /// 0 for the divisor 0, which no USART accepts.
    pub const fn baud(self, pclk_hz: u32) -> u32 {
        match pclk_hz.checked_div(self.raw() as u32) {
            Some(baud) => baud,
            None => 0,
        }
    }
}

/// The divisor for `baud` on a USART bus clocked at `pclk_hz`, rounded to the nearest, with the
/// 16 times oversampling the console uses. `None` when the baud rate is zero or too fast for the
/// bus clock, or too slow for the 12-bit mantissa.
pub const fn divisor(pclk_hz: u32, baud: u32) -> Option<Divisor> {
    if baud == 0 || pclk_hz / 16 < baud {
        return None;
    }
    let raw = pclk_hz.saturating_add(baud / 2) / baud;
    if raw > 0xffff {
        return None;
    }
    Some(Divisor {
        mantissa: (raw >> 4) as u16,
        fraction: (raw & 0xf) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_console_divisor_on_the_apb1_clock_of_the_f407() {
        // The F407 at 168 MHz runs APB1 at 42 MHz.
        let divisor = divisor(42_000_000, CONSOLE_BAUD).unwrap();
        assert_eq!(
            divisor,
            Divisor {
                mantissa: 11,
                fraction: 6
            }
        );
        assert_eq!(divisor.raw(), 182);
        // 42 MHz / 182 = 230769 baud, 0.16 % fast: far inside what 8N1 tolerates.
        let actual = divisor.baud(42_000_000);
        assert_eq!(actual, 230_769);
        assert!(actual.abs_diff(CONSOLE_BAUD) * 100 < CONSOLE_BAUD);
    }

    #[test]
    fn the_divisor_is_the_nearest_not_the_floor() {
        // 16 MHz / 115200 = 138.9: 139, not 138.
        assert_eq!(divisor(16_000_000, 115_200).unwrap().raw(), 139);
        // 8 MHz / 9600 = 833.3: 833.
        assert_eq!(divisor(8_000_000, 9_600).unwrap().raw(), 833);
        let wide = divisor(42_000_000, 9_600).unwrap();
        assert_eq!(wide.raw(), 4_375);
        assert_eq!((wide.mantissa, wide.fraction), (273, 7));
    }

    #[test]
    fn rates_the_bus_cannot_make_give_no_divisor() {
        assert_eq!(divisor(42_000_000, 0), None);
        // Faster than the bus clock divided by 16 needs 8 times oversampling.
        assert_eq!(divisor(42_000_000, 2_625_001), None);
        assert!(divisor(42_000_000, 2_625_000).is_some());
        assert_eq!(divisor(0, 9_600), None);
        // Slower than the 12-bit mantissa can count.
        assert_eq!(divisor(42_000_000, 600), None);
        assert!(divisor(42_000_000, 641).is_some());
        // The largest numbers do not overflow.
        assert_eq!(divisor(u32::MAX, u32::MAX), None);
        assert!(divisor(u32::MAX, 230_400).is_some());
        assert_eq!(
            Divisor {
                mantissa: 0,
                fraction: 0
            }
            .baud(42_000_000),
            0
        );
    }

    #[test]
    fn the_fixed_point_split_and_the_raw_value_agree() {
        for raw in [16_u32, 17, 182, 4_095, 0xfff0, 0xffff] {
            let pclk = raw * 9_600;
            let divisor = divisor(pclk, 9_600).unwrap();
            assert_eq!(u32::from(divisor.raw()), raw);
            assert!(divisor.mantissa <= 0xfff);
            assert!(divisor.fraction <= 0xf);
        }
    }
}
