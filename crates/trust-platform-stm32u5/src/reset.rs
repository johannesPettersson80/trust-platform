//! The STM32U5A9's reset record (specification 34, D.4 and D.5; design §1.5): the RCC_CSR reset flags,
//! read once at boot and then cleared, and the two-word cookie of the intentional reset phase in the
//! TAMP backup registers `TAMP_BKP0R` and `TAMP_BKP1R`. The cookie's encoding is the shared
//! [`ResetCookie`](trust_platform_stm32::ResetCookie); the flags are read at this chip's positions.

use trust_platform_stm32::reset::ResetBits;

/// RCC_CSR's reset flags on the STM32U5A9 (ST `stm32u5a9xx.h`, `RCC_CSR_*_Pos`). The U5 has no
/// power-on flag of its own: BORRSTF (bit 27) is set at a power-on as well as at a brownout (design
/// §1.5), so the power-on and the brownout meanings read the same bit.
pub const RESET_BITS: ResetBits = ResetBits {
    watchdog: 29,
    software: 28,
    pin: 26,
    power_on: 27,
    brownout: 27,
    option_bytes: Some(25),
};

/// Why a phase cookie could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieError {
    /// Phase zero is not a phase; the F407 adapter refuses it as well.
    ZeroPhase,
    /// The backup registers did not read back the pair that was written.
    Readback,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::ResetControl;

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware {
    use stm32u5::stm32u5a9 as pac;
    use trust_platform_stm32::{ResetCookie, ResetReason};

    use super::{CookieError, RESET_BITS};

    /// The reset flags of this boot and the TAMP backup registers that keep the cookie.
    pub struct ResetControl {
        tamp: pac::TAMP,
        reason: ResetReason,
    }

    impl ResetControl {
        /// Reads RCC_CSR, clears its flags (RMVF), and enables the backup domain for the cookie: the
        /// RTC/TAMP APB clock, then PWR_DBPR.DBP as ST's `HAL_PWR_EnableBkUpAccess` sets it, read back
        /// before the backup registers are used.
        pub fn take(rcc: &pac::RCC, pwr: &pac::PWR, tamp: pac::TAMP) -> Self {
            let reason = ResetReason::new(rcc.csr().read().bits(), RESET_BITS);
            rcc.csr().modify(|_, w| w.rmvf().set_bit());
            rcc.apb3enr().modify(|_, w| w.rtcapben().set_bit());
            // ST's clock enable reads the register back once the write has settled.
            let _ = rcc.apb3enr().read();
            pwr.dbpr().modify(|_, w| w.dbp().set_bit());
            let _ = pwr.dbpr().read();
            Self { tamp, reason }
        }

        /// The reset flags of this boot, captured before RMVF cleared them.
        pub fn reason(&self) -> ResetReason {
            self.reason
        }

        /// The phase marker of the previous boot, when its flags allow trusting it.
        pub fn cookie(&self) -> Option<ResetCookie> {
            if !self.reason.cookie_trusted() {
                return None;
            }
            ResetCookie::decode([
                self.tamp.bkp0r().read().bits(),
                self.tamp.bkp1r().read().bits(),
            ])
        }

        /// Writes a checked phase marker, invalidating the old pair before the replacement, as the
        /// F407 adapter does.
        pub fn set_cookie(&mut self, cookie: ResetCookie) -> Result<(), CookieError> {
            if cookie.phase == 0 {
                return Err(CookieError::ZeroPhase);
            }
            let words = cookie.words();
            self.tamp.bkp1r().write(|w| unsafe { w.bits(0) });
            self.tamp.bkp0r().write(|w| unsafe { w.bits(words[0]) });
            self.tamp.bkp1r().write(|w| unsafe { w.bits(words[1]) });
            let observed = [
                self.tamp.bkp0r().read().bits(),
                self.tamp.bkp1r().read().bits(),
            ];
            if observed != words {
                return Err(CookieError::Readback);
            }
            Ok(())
        }

        /// Invalidates the phase marker once the reset observation has been recorded.
        pub fn clear_cookie(&mut self) {
            self.tamp.bkp1r().write(|w| unsafe { w.bits(0) });
            self.tamp.bkp0r().write(|w| unsafe { w.bits(0) });
        }
    }
}
