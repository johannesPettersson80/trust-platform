//! The STM32U5A9's pins by name (specification 34, TARGET-01 and D.5; RM0456's GPIO registers): the
//! claim of a line against the chip table, the set-up order of an output and of an input, the process
//! image of a wiring, and the emergency path that drives every configured output to its safe level.
//! The claim, the order and the record of the outputs are pure and host-tested; the register writes
//! are compiled for the chip only.

use crate::chip::Chip;
use crate::pins::{Claims, Pin, PinError, Pull};

/// Claims `line` for the chip. The chip's table names the pin and refuses a line that is no pin, a pin
/// the chip lacks, or one the firmware keeps for itself; the claims then refuse a pin that another line
/// already uses (specification 34, TARGET-01).
///
/// # Errors
///
/// The [`PinError`] that says why the line cannot be used.
pub fn claim_pin(chip: &Chip, claims: &mut Claims, line: &str) -> Result<Pin, PinError> {
    let pin = chip.pin(line)?;
    claims.claim(pin)?;
    Ok(pin)
}

/// One register step of a pin's set-up, in the order the hardware takes it (RM0456).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Drives the output latch of the pin to `high` through BSRR.
    Latch {
        /// The pin.
        pin: Pin,
        /// Whether the latch takes the high level.
        high: bool,
    },
    /// Sets the pin's output type to push-pull (OTYPER).
    PushPull(Pin),
    /// Sets the pin's output speed to low (OSPEEDR).
    LowSpeed(Pin),
    /// Sets the pin's pull-up or pull-down (PUPDR).
    Pull(Pin, Pull),
    /// Sets the pin's mode to general-purpose output (MODER).
    Output(Pin),
    /// Sets the pin's mode to input (MODER).
    Input(Pin),
}

/// The steps that make `pin` an output whose safe level is `safe_high`. The output latch comes first, so
/// the pin never shows a level other than its safe one; the mode comes last (specification 34, D.4 and
/// TARGET-01).
#[must_use]
pub fn output_steps(pin: Pin, safe_high: bool) -> [Step; 5] {
    [
        Step::Latch {
            pin,
            high: safe_high,
        },
        Step::PushPull(pin),
        Step::LowSpeed(pin),
        Step::Pull(pin, Pull::Floating),
        Step::Output(pin),
    ]
}

/// The steps that make `pin` an input with `pull`.
#[must_use]
pub fn input_steps(pin: Pin, pull: Pull) -> [Step; 2] {
    [Step::Pull(pin, pull), Step::Input(pin)]
}

/// An input pin claimed by the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    pin: Pin,
}

impl Input {
    /// The pin.
    #[must_use]
    pub const fn pin(self) -> Pin {
        self.pin
    }
}

/// An output pin claimed by the adapter, with the level it takes in its safe state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pin: Pin,
    safe_high: bool,
}

impl Output {
    /// The pin.
    #[must_use]
    pub const fn pin(self) -> Pin {
        self.pin
    }

    /// Whether the pin is high in the output's safe state.
    #[must_use]
    pub const fn safe_high(self) -> bool {
        self.safe_high
    }
}

/// The process image of a wiring: its one input and its one output. The STM32F407 adapter calls the
/// same set `Process`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessIo {
    input: Input,
    output: Output,
}

impl ProcessIo {
    /// The process image of `input` and `output`.
    #[must_use]
    pub const fn new(input: Input, output: Output) -> Self {
        Self { input, output }
    }

    /// The input.
    #[must_use]
    pub const fn input(self) -> Input {
        self.input
    }

    /// The output.
    #[must_use]
    pub const fn output(self) -> Output {
        self.output
    }
}

/// The bits one output adds to its port's record for the emergency path: bit `n` marks pin `n` as an
/// output, and bit `16 + n` is set too when the pin's safe level is high. A port's record is the OR of
/// these bits over its outputs.
#[must_use]
pub fn record_bits(pin: Pin, safe_high: bool) -> u32 {
    let mask = u32::from(pin.mask());
    if safe_high {
        mask | (mask << 16)
    } else {
        mask
    }
}

/// The pins of one port's record to drive in the safe state, as two 16-bit masks: the pins to drive high
/// and the pins to drive low. A pin is in one mask or neither, and never in both.
#[must_use]
pub fn safe_levels(record: u32) -> (u16, u16) {
    let outputs = (record & 0xFFFF) as u16;
    let high = ((record >> 16) as u16) & outputs;
    (high, outputs & !high)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::{drive_outputs_to_safe_levels, DigitalIo, Ports};

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware {
    use core::sync::atomic::{AtomicU32, Ordering};

    use stm32u5::stm32u5a9 as pac;

    use super::{
        claim_pin, input_steps, output_steps, record_bits, safe_levels, Input, Output, ProcessIo,
        Step,
    };
    use crate::chip::CHIP;
    use crate::pins::{Claims, Pin, PinError, Pull, Wiring, PORT_COUNT};

    /// The outputs the firmware has claimed, one record per port in the layout of `record_bits`, by
    /// `Port::index` (port A first). The fault path reads them; only `claim_output` writes them.
    static OUTPUTS: [AtomicU32; PORT_COUNT] = [const { AtomicU32::new(0) }; PORT_COUNT];

    /// The GPIO ports of the chip, owned by the adapter: one register block per port, A to J.
    pub struct Ports {
        /// Port A.
        pub a: pac::GPIOA,
        /// Port B.
        pub b: pac::GPIOB,
        /// Port C.
        pub c: pac::GPIOC,
        /// Port D.
        pub d: pac::GPIOD,
        /// Port E.
        pub e: pac::GPIOE,
        /// Port F.
        pub f: pac::GPIOF,
        /// Port G.
        pub g: pac::GPIOG,
        /// Port H.
        pub h: pac::GPIOH,
        /// Port I.
        pub i: pac::GPIOI,
        /// Port J.
        pub j: pac::GPIOJ,
    }

    /// Runs `$body` with `$regs` bound to the register block of the port of `$pin`. Port K does not exist
    /// on this chip and the chip table refuses every name of it, so the last arm is never reached; it
    /// yields `$absent`.
    macro_rules! with_port {
        ($ports:expr, $pin:expr, |$regs:ident| $body:block, $absent:expr) => {
            match $pin.port().index() {
                0 => {
                    let $regs = &*$ports.a;
                    $body
                }
                1 => {
                    let $regs = &*$ports.b;
                    $body
                }
                2 => {
                    let $regs = &*$ports.c;
                    $body
                }
                3 => {
                    let $regs = &*$ports.d;
                    $body
                }
                4 => {
                    let $regs = &*$ports.e;
                    $body
                }
                5 => {
                    let $regs = &*$ports.f;
                    $body
                }
                6 => {
                    let $regs = &*$ports.g;
                    $body
                }
                7 => {
                    let $regs = &*$ports.h;
                    $body
                }
                8 => {
                    let $regs = &*$ports.i;
                    $body
                }
                9 => {
                    let $regs = &*$ports.j;
                    $body
                }
                _ => $absent,
            }
        };
    }

    /// Every pin of the chip, named at run time: the claims, the ports and the register writes behind
    /// them.
    pub struct DigitalIo {
        ports: Ports,
        claims: Claims,
    }

    impl DigitalIo {
        /// Takes the ports and switches on their clocks (`RCC_AHB2ENR1`). No pin changes until it is
        /// claimed.
        #[must_use]
        pub fn new(ports: Ports, rcc: &pac::RCC) -> Self {
            rcc.ahb2enr1().modify(|_, w| {
                w.gpioaen()
                    .set_bit()
                    .gpioben()
                    .set_bit()
                    .gpiocen()
                    .set_bit()
                    .gpioden()
                    .set_bit()
                    .gpioeen()
                    .set_bit()
                    .gpiofen()
                    .set_bit()
                    .gpiogen()
                    .set_bit()
                    .gpiohen()
                    .set_bit()
                    .gpioien()
                    .set_bit()
                    .gpiojen()
                    .set_bit()
            });
            // The ports answer a few cycles after their clock is on, so the enable is read back.
            let _ = rcc.ahb2enr1().read();
            Self {
                ports,
                claims: Claims::new(),
            }
        }

        /// Claims `line` as an output with the safe level `safe_high`. Its latch is set before it becomes
        /// an output (see `output_steps`), and the pin is recorded for `drive_outputs_to_safe_levels`.
        ///
        /// # Errors
        ///
        /// The [`PinError`] that says why the line cannot be used.
        pub fn claim_output(&mut self, line: &str, safe_high: bool) -> Result<Output, PinError> {
            let pin = claim_pin(&CHIP, &mut self.claims, line)?;
            for step in output_steps(pin, safe_high) {
                self.apply(step);
            }
            OUTPUTS[pin.port().index()].fetch_or(record_bits(pin, safe_high), Ordering::Relaxed);
            Ok(Output { pin, safe_high })
        }

        /// Claims `line` as an input with `pull`.
        ///
        /// # Errors
        ///
        /// The [`PinError`] that says why the line cannot be used.
        pub fn claim_input(&mut self, line: &str, pull: Pull) -> Result<Input, PinError> {
            let pin = claim_pin(&CHIP, &mut self.claims, line)?;
            for step in input_steps(pin, pull) {
                self.apply(step);
            }
            Ok(Input { pin })
        }

        /// Claims the process image of `wiring`: its output first, then its input.
        ///
        /// # Errors
        ///
        /// The [`PinError`] of the first line that cannot be used.
        pub fn claim_process(&mut self, wiring: &Wiring<'_>) -> Result<ProcessIo, PinError> {
            let output = self.claim_output(wiring.output, wiring.output_safe_high)?;
            let input = self.claim_input(wiring.input, wiring.input_pull)?;
            Ok(ProcessIo::new(input, output))
        }

        /// The level on an input pin now.
        #[must_use]
        pub fn read(&self, input: Input) -> bool {
            self.level(input.pin)
        }

        /// Drives an output pin high or low.
        pub fn drive(&mut self, output: Output, high: bool) {
            self.set_level(output.pin, high);
        }

        /// Drives an output pin to its safe level.
        pub fn drive_safe(&mut self, output: Output) {
            self.set_level(output.pin, output.safe_high);
        }

        /// The level on an output's pad (IDR), not its latch, so a short or a load that pulls the pin
        /// elsewhere shows. It does not show what an LED or an actuator does.
        #[must_use]
        pub fn readback(&self, output: Output) -> bool {
            self.level(output.pin)
        }

        /// Runs one step of a pin's set-up on the registers of its port.
        fn apply(&mut self, step: Step) {
            match step {
                Step::Latch { pin, high } => self.set_level(pin, high),
                Step::PushPull(pin) => {
                    let n = pin.number();
                    with_port!(
                        self.ports,
                        pin,
                        |regs| {
                            regs.otyper().modify(|_, w| w.ot(n).push_pull());
                        },
                        ()
                    );
                }
                Step::LowSpeed(pin) => {
                    let n = pin.number();
                    with_port!(
                        self.ports,
                        pin,
                        |regs| {
                            regs.ospeedr().modify(|_, w| w.ospeed(n).low_speed());
                        },
                        ()
                    );
                }
                Step::Pull(pin, pull) => {
                    let n = pin.number();
                    with_port!(
                        self.ports,
                        pin,
                        |regs| {
                            regs.pupdr().modify(|_, w| match pull {
                                Pull::Floating => w.pupd(n).floating(),
                                Pull::Up => w.pupd(n).pull_up(),
                                Pull::Down => w.pupd(n).pull_down(),
                            });
                        },
                        ()
                    );
                }
                Step::Output(pin) => {
                    let n = pin.number();
                    with_port!(
                        self.ports,
                        pin,
                        |regs| {
                            regs.moder().modify(|_, w| w.mode(n).output());
                        },
                        ()
                    );
                }
                Step::Input(pin) => {
                    let n = pin.number();
                    with_port!(
                        self.ports,
                        pin,
                        |regs| {
                            regs.moder().modify(|_, w| w.mode(n).input());
                        },
                        ()
                    );
                }
            }
        }

        /// Sets the output latch of `pin` through BSRR, which touches only that pin.
        fn set_level(&mut self, pin: Pin, high: bool) {
            let n = pin.number();
            with_port!(
                self.ports,
                pin,
                |regs| {
                    regs.bsrr().write(|w| {
                        if high {
                            w.bs(n).set_()
                        } else {
                            w.br(n).reset()
                        }
                    });
                },
                ()
            );
        }

        /// The level on the pad of `pin` (IDR).
        fn level(&self, pin: Pin) -> bool {
            let n = pin.number();
            with_port!(
                self.ports,
                pin,
                |regs| { regs.idr().read().id(n).is_high() },
                false
            )
        }
    }

    /// Drives every configured output to its safe level, for the terminal fault path (specification 34,
    /// D.5). It takes no owner and allocates nothing: it reads the records that `claim_output` kept, and
    /// writes each port's BSRR once, setting or resetting only the outputs it records.
    pub fn drive_outputs_to_safe_levels() {
        // SAFETY: the ten ports are the chip's own GPIO register blocks at their fixed addresses (RM0456).
        // Only BSRR is written, and a BSRR write touches only the pins named in it. This is the fault path,
        // which takes the outputs from every other owner from here on.
        let registers = unsafe {
            [
                (&*pac::GPIOA::ptr()).bsrr(),
                (&*pac::GPIOB::ptr()).bsrr(),
                (&*pac::GPIOC::ptr()).bsrr(),
                (&*pac::GPIOD::ptr()).bsrr(),
                (&*pac::GPIOE::ptr()).bsrr(),
                (&*pac::GPIOF::ptr()).bsrr(),
                (&*pac::GPIOG::ptr()).bsrr(),
                (&*pac::GPIOH::ptr()).bsrr(),
                (&*pac::GPIOI::ptr()).bsrr(),
                (&*pac::GPIOJ::ptr()).bsrr(),
            ]
        };
        for (record, bsrr) in OUTPUTS.iter().zip(registers) {
            let (high, low) = safe_levels(record.load(Ordering::Relaxed));
            if (high | low) != 0 {
                bsrr.write(|w| {
                    for pin in 0..16_u8 {
                        if (high & (1 << pin)) != 0 {
                            w.bs(pin).set_();
                        }
                        if (low & (1 << pin)) != 0 {
                            w.br(pin).reset();
                        }
                    }
                    w
                });
            }
        }
    }
}
