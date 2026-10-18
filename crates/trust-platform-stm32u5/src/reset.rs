//! The STM32U5A9's reset record (specification 34, D.4 and D.5; design §1.5): the RCC_CSR reset flags,
//! read once at boot and then cleared, the two-word cookie of the intentional reset phase in the TAMP
//! backup registers `TAMP_BKP0R` and `TAMP_BKP1R`, and the reset record and the boot counter in the
//! backup registers BKP2R to BKP14R, the numbers the STM32F407 uses. The cookie's encoding and the
//! record's are shared (`trust_platform_stm32::ResetCookie`, `trust_platform_memory::reset`); the flags
//! are read at this chip's positions. The store is host-tested over a fake register file; the registers
//! are compiled for the chip only.

use trust_platform_memory::reset::{self, ResetCause, ResetRecord, ResetStore, RECORD_WORDS};
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

/// The first backup register of the reset record: BKP2R holds the record's first word, as the F407 keeps
/// it in RTC backup register 2. The record takes the `RECORD_WORDS` registers after it.
pub const RECORD_FIRST_REGISTER: usize = 2;
/// The backup register of the boot counter: BKP14R, as on the F407.
pub const BOOT_COUNT_REGISTER: usize = 14;
const _: () = assert!(RECORD_FIRST_REGISTER + RECORD_WORDS == BOOT_COUNT_REGISTER);

/// The TAMP backup registers as the reset store reads and writes them: number 0 is BKP0R. The chip
/// implements it over its register block; a test implements it over a fake register file.
pub trait BackupRegisters {
    /// The value of backup register `number`.
    fn read(&self, number: usize) -> u32;

    /// Writes `value` to backup register `number`.
    fn write(&mut self, number: usize, value: u32);
}

/// The reset record and the boot counter over the backup registers. It implements the shared
/// [`ResetStore`], so the STM32U5A9 and the STM32F407 store one record in the same form.
#[derive(Debug)]
pub struct BackupStore<B> {
    registers: B,
}

impl<B> BackupStore<B> {
    /// The store over `registers`.
    pub const fn new(registers: B) -> Self {
        Self { registers }
    }

    /// The backup registers.
    pub const fn registers(&self) -> &B {
        &self.registers
    }

    /// The backup registers, for writing.
    pub fn registers_mut(&mut self) -> &mut B {
        &mut self.registers
    }
}

impl<B: BackupRegisters> BackupStore<B> {
    /// Counts this boot in BKP14R and returns the new count, which the next record is stamped with.
    pub fn note_boot(&mut self) -> u32 {
        reset::note_boot(self)
    }

    /// Writes the record that a panic or a watchdog path leaves for the next boot.
    pub fn write_reset_record(&mut self, cause: ResetCause, text: &str) {
        reset::write_record(self, cause, text);
    }

    /// Reads the record the previous boot left and clears it, so a cause is reported once.
    pub fn take_reset_record(&mut self) -> Option<ResetRecord> {
        reset::take_record(self)
    }
}

impl<B: BackupRegisters> ResetStore for BackupStore<B> {
    fn record_words(&self) -> [u32; RECORD_WORDS] {
        core::array::from_fn(|index| self.registers.read(RECORD_FIRST_REGISTER + index))
    }

    fn set_record_words(&mut self, words: [u32; RECORD_WORDS]) {
        for (index, word) in words.into_iter().enumerate() {
            self.registers.write(RECORD_FIRST_REGISTER + index, word);
        }
    }

    fn boot_count(&self) -> u32 {
        self.registers.read(BOOT_COUNT_REGISTER)
    }

    fn set_boot_count(&mut self, count: u32) {
        self.registers.write(BOOT_COUNT_REGISTER, count);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::ResetControl;

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware {
    use stm32u5::{stm32u5a9 as pac, Readable, Reg, RegisterSpec, Resettable, Writable};
    use trust_platform_memory::reset::{ResetCause, ResetRecord};
    use trust_platform_stm32::{ResetCookie, ResetReason};

    use super::{BackupRegisters, BackupStore, CookieError, RESET_BITS};

    /// The TAMP backup registers of the chip, read and written through its register block (RM0456).
    struct Tamp {
        tamp: pac::TAMP,
    }

    impl BackupRegisters for Tamp {
        fn read(&self, number: usize) -> u32 {
            let tamp = &self.tamp;
            match number {
                0 => read_word(tamp.bkp0r()),
                1 => read_word(tamp.bkp1r()),
                2 => read_word(tamp.bkp2r()),
                3 => read_word(tamp.bkp3r()),
                4 => read_word(tamp.bkp4r()),
                5 => read_word(tamp.bkp5r()),
                6 => read_word(tamp.bkp6r()),
                7 => read_word(tamp.bkp7r()),
                8 => read_word(tamp.bkp8r()),
                9 => read_word(tamp.bkp9r()),
                10 => read_word(tamp.bkp10r()),
                11 => read_word(tamp.bkp11r()),
                12 => read_word(tamp.bkp12r()),
                13 => read_word(tamp.bkp13r()),
                14 => read_word(tamp.bkp14r()),
                15 => read_word(tamp.bkp15r()),
                16 => read_word(tamp.bkp16r()),
                17 => read_word(tamp.bkp17r()),
                18 => read_word(tamp.bkp18r()),
                19 => read_word(tamp.bkp19r()),
                20 => read_word(tamp.bkp20r()),
                21 => read_word(tamp.bkp21r()),
                22 => read_word(tamp.bkp22r()),
                23 => read_word(tamp.bkp23r()),
                24 => read_word(tamp.bkp24r()),
                25 => read_word(tamp.bkp25r()),
                26 => read_word(tamp.bkp26r()),
                27 => read_word(tamp.bkp27r()),
                28 => read_word(tamp.bkp28r()),
                29 => read_word(tamp.bkp29r()),
                30 => read_word(tamp.bkp30r()),
                31 => read_word(tamp.bkp31r()),
                // The block has 32 backup registers; no caller names another number.
                _ => 0,
            }
        }

        fn write(&mut self, number: usize, value: u32) {
            let tamp = &self.tamp;
            match number {
                0 => write_word(tamp.bkp0r(), value),
                1 => write_word(tamp.bkp1r(), value),
                2 => write_word(tamp.bkp2r(), value),
                3 => write_word(tamp.bkp3r(), value),
                4 => write_word(tamp.bkp4r(), value),
                5 => write_word(tamp.bkp5r(), value),
                6 => write_word(tamp.bkp6r(), value),
                7 => write_word(tamp.bkp7r(), value),
                8 => write_word(tamp.bkp8r(), value),
                9 => write_word(tamp.bkp9r(), value),
                10 => write_word(tamp.bkp10r(), value),
                11 => write_word(tamp.bkp11r(), value),
                12 => write_word(tamp.bkp12r(), value),
                13 => write_word(tamp.bkp13r(), value),
                14 => write_word(tamp.bkp14r(), value),
                15 => write_word(tamp.bkp15r(), value),
                16 => write_word(tamp.bkp16r(), value),
                17 => write_word(tamp.bkp17r(), value),
                18 => write_word(tamp.bkp18r(), value),
                19 => write_word(tamp.bkp19r(), value),
                20 => write_word(tamp.bkp20r(), value),
                21 => write_word(tamp.bkp21r(), value),
                22 => write_word(tamp.bkp22r(), value),
                23 => write_word(tamp.bkp23r(), value),
                24 => write_word(tamp.bkp24r(), value),
                25 => write_word(tamp.bkp25r(), value),
                26 => write_word(tamp.bkp26r(), value),
                27 => write_word(tamp.bkp27r(), value),
                28 => write_word(tamp.bkp28r(), value),
                29 => write_word(tamp.bkp29r(), value),
                30 => write_word(tamp.bkp30r(), value),
                31 => write_word(tamp.bkp31r(), value),
                // The block has 32 backup registers; no caller names another number.
                _ => {}
            }
        }
    }

    /// Reads a 32-bit register.
    fn read_word<REG: Readable + RegisterSpec<Ux = u32>>(register: &Reg<REG>) -> u32 {
        register.read().bits()
    }

    /// Writes a 32-bit register. A TAMP backup register holds any 32-bit value.
    fn write_word<REG: Writable + Resettable + RegisterSpec<Ux = u32>>(
        register: &Reg<REG>,
        value: u32,
    ) {
        // SAFETY: a TAMP backup register takes any 32-bit value (RM0456), and the write touches no other
        // register. The backup domain is enabled by `ResetControl::take` before any write.
        register.write(|w| unsafe { w.bits(value) });
    }

    /// The reset flags of this boot and the reset record and cookie of the backup registers.
    pub struct ResetControl {
        store: BackupStore<Tamp>,
        reason: ResetReason,
    }

    impl ResetControl {
        /// Reads RCC_CSR, clears its flags (RMVF), and enables the backup domain for the cookie and the
        /// record: the RTC/TAMP APB clock, then PWR_DBPR.DBP as ST's `HAL_PWR_EnableBkUpAccess` sets it,
        /// read back before the backup registers are used.
        pub fn take(rcc: &pac::RCC, pwr: &pac::PWR, tamp: pac::TAMP) -> Self {
            let reason = ResetReason::new(rcc.csr().read().bits(), RESET_BITS);
            rcc.csr().modify(|_, w| w.rmvf().set_bit());
            rcc.apb3enr().modify(|_, w| w.rtcapben().set_bit());
            // ST's clock enable reads the register back once the write has settled.
            let _ = rcc.apb3enr().read();
            pwr.dbpr().modify(|_, w| w.dbp().set_bit());
            let _ = pwr.dbpr().read();
            Self {
                store: BackupStore::new(Tamp { tamp }),
                reason,
            }
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
            let registers = self.store.registers();
            ResetCookie::decode([registers.read(0), registers.read(1)])
        }

        /// Writes a checked phase marker, invalidating the old pair before the replacement, as the F407
        /// adapter does.
        pub fn set_cookie(&mut self, cookie: ResetCookie) -> Result<(), CookieError> {
            if cookie.phase == 0 {
                return Err(CookieError::ZeroPhase);
            }
            let words = cookie.words();
            let registers = self.store.registers_mut();
            registers.write(1, 0);
            registers.write(0, words[0]);
            registers.write(1, words[1]);
            if [registers.read(0), registers.read(1)] != words {
                return Err(CookieError::Readback);
            }
            Ok(())
        }

        /// Invalidates the phase marker once the reset observation has been recorded.
        pub fn clear_cookie(&mut self) {
            let registers = self.store.registers_mut();
            registers.write(1, 0);
            registers.write(0, 0);
        }

        /// Writes the record that a panic or a watchdog path leaves for the next boot.
        pub fn write_reset_record(&mut self, cause: ResetCause, text: &str) {
            self.store.write_reset_record(cause, text);
        }

        /// Reads the record the previous boot left and clears it, so a cause is reported once.
        pub fn take_reset_record(&mut self) -> Option<ResetRecord> {
            self.store.take_reset_record()
        }

        /// Counts this boot and returns the new boot counter.
        pub fn note_boot(&mut self) -> u32 {
            self.store.note_boot()
        }
    }
}
