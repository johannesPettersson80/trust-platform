//! Pins named at run time. This driver owns the chip's GPIO ports as peripheral tokens, so a
//! line such as `PD12` needs no pin type known at compile time (specification 34, TARGET-01), and
//! no board's LEDs or buttons appear here: the wiring comes in from the firmware.
use super::emergency;
use crate::{
    chip::CHIP,
    pac,
    pins::{Claims, Pin, PinError, Pull},
};
use stm32f4xx_hal::rcc::Rcc;

/// An input pin handed out by [`DigitalIo::claim_input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input(Pin);

/// An output pin handed out by [`DigitalIo::claim_output`], with the level it takes on a fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    pin: Pin,
    safe_high: bool,
}

impl Input {
    /// The pin.
    pub const fn pin(self) -> Pin {
        self.0
    }
}

impl Output {
    /// The pin.
    pub const fn pin(self) -> Pin {
        self.pin
    }

    /// Whether the pin is high in the output's safe state.
    pub const fn safe_high(self) -> bool {
        self.safe_high
    }
}

/// The GPIO ports of the chip, as the peripheral access crate hands them out.
pub(super) struct Ports {
    pub a: pac::GPIOA,
    pub b: pac::GPIOB,
    pub c: pac::GPIOC,
    pub d: pac::GPIOD,
    pub e: pac::GPIOE,
    pub f: pac::GPIOF,
    pub g: pac::GPIOG,
    pub h: pac::GPIOH,
    pub i: pac::GPIOI,
}

/// Every pin of the chip, named at run time. The register accessors of the ports have the same
/// methods on every port, so one body serves all of them; a port the chip does not have is
/// never reached, because a pin is claimed only after the chip's own table has accepted it.
pub struct DigitalIo {
    ports: Ports,
    claims: Claims,
}

// Runs `$body` with `$regs` bound to the register block of the pin's port. `$absent` is the
// value for a port the chip lacks.
macro_rules! with_port {
    ($io:expr, $pin:expr, |$regs:ident| $body:expr, $absent:expr) => {
        match $pin.port().index() {
            0 => {
                let $regs = &*$io.ports.a;
                $body
            }
            1 => {
                let $regs = &*$io.ports.b;
                $body
            }
            2 => {
                let $regs = &*$io.ports.c;
                $body
            }
            3 => {
                let $regs = &*$io.ports.d;
                $body
            }
            4 => {
                let $regs = &*$io.ports.e;
                $body
            }
            5 => {
                let $regs = &*$io.ports.f;
                $body
            }
            6 => {
                let $regs = &*$io.ports.g;
                $body
            }
            7 => {
                let $regs = &*$io.ports.h;
                $body
            }
            8 => {
                let $regs = &*$io.ports.i;
                $body
            }
            _ => $absent,
        }
    };
}

// Sets the pull-up/pull-down of pin `$n` in the port registers `$regs`.
macro_rules! set_pull {
    ($regs:expr, $n:expr, $pull:expr) => {
        $regs.pupdr().modify(|_, w| match $pull {
            Pull::Floating => w.pupdr($n).floating(),
            Pull::Up => w.pupdr($n).pull_up(),
            Pull::Down => w.pupdr($n).pull_down(),
        })
    };
}

impl DigitalIo {
    /// Takes the ports and switches their clocks on. No pin changes until it is claimed.
    pub(super) fn new(ports: Ports, rcc: &mut Rcc) -> Self {
        rcc.ahb1enr().modify(|_, w| {
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
        });
        // The ports answer a few cycles after their clock is on.
        let _ = rcc.ahb1enr().read();
        Self {
            ports,
            claims: Claims::new(),
        }
    }

    /// Claims the pin a line names as an output. The pin first latches its safe level and then
    /// becomes a push-pull output, so it never shows another level. The level is also what
    /// `emergency_halt` drives it to.
    pub fn claim_output(&mut self, line: &str, safe_high: bool) -> Result<Output, PinError> {
        let pin = self.claim(line)?;
        let n = pin.number();
        with_port!(
            self,
            pin,
            |regs| {
                regs.bsrr().write(|w| {
                    if safe_high {
                        w.bs(n).set_bit()
                    } else {
                        w.br(n).set_bit()
                    }
                });
                regs.otyper().modify(|_, w| w.ot(n).push_pull());
                regs.ospeedr().modify(|_, w| w.ospeedr(n).low_speed());
                set_pull!(regs, n, Pull::Floating);
                regs.moder().modify(|_, w| w.moder(n).output());
            },
            ()
        );
        emergency::register_output(pin, safe_high);
        Ok(Output { pin, safe_high })
    }

    /// Claims the pin a line names as an input with the given pull.
    pub fn claim_input(&mut self, line: &str, pull: Pull) -> Result<Input, PinError> {
        let pin = self.claim(line)?;
        let n = pin.number();
        with_port!(
            self,
            pin,
            |regs| {
                set_pull!(regs, n, pull);
                regs.moder().modify(|_, w| w.moder(n).input());
            },
            ()
        );
        Ok(Input(pin))
    }

    /// The level on an input pin.
    pub fn read(&self, input: Input) -> bool {
        self.level(input.0)
    }

    /// Drives an output pin high or low.
    pub fn drive(&mut self, output: Output, high: bool) {
        let pin = output.pin;
        let n = pin.number();
        with_port!(
            self,
            pin,
            |regs| {
                regs.bsrr().write(|w| {
                    if high {
                        w.bs(n).set_bit()
                    } else {
                        w.br(n).set_bit()
                    }
                });
            },
            ()
        );
    }

    /// Drives an output pin to its safe level, without consulting any runtime state.
    pub fn drive_safe(&mut self, output: Output) {
        self.drive(output, output.safe_high);
    }

    /// The electrical level on an output pin: its pad (IDR), not the output latch, so a short or
    /// a load that pulls the pin elsewhere shows. It does not show an LED or an actuator.
    pub fn readback(&self, output: Output) -> bool {
        self.level(output.pin)
    }

    /// Hands the chip's console pins to USART2 (alternate function 7). The console pins are
    /// reserved in the chip table, so no line can claim them.
    pub(super) fn route_console(&mut self) {
        self.route_to_af7(CHIP.console.tx, Pull::Floating);
        self.route_to_af7(CHIP.console.rx, Pull::Up);
    }

    fn claim(&mut self, line: &str) -> Result<Pin, PinError> {
        let pin = CHIP.pin(line)?;
        self.claims.claim(pin)?;
        Ok(pin)
    }

    fn level(&self, pin: Pin) -> bool {
        let n = pin.number();
        with_port!(
            self,
            pin,
            |regs| regs.idr().read().idr(n).bit_is_set(),
            false
        )
    }

    fn route_to_af7(&mut self, pin: Pin, pull: Pull) {
        let n = pin.number();
        with_port!(
            self,
            pin,
            |regs| {
                regs.otyper().modify(|_, w| w.ot(n).push_pull());
                regs.ospeedr().modify(|_, w| w.ospeedr(n).low_speed());
                set_pull!(regs, n, pull);
                if n < 8 {
                    regs.afrl().modify(|_, w| w.afr(n).af7());
                } else {
                    regs.afrh().modify(|_, w| w.afr(n - 8).af7());
                }
                regs.moder().modify(|_, w| w.moder(n).alternate());
            },
            ()
        );
    }
}
