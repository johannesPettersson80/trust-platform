//! Pin names of the STM32F4 family. `PD12` names pin 12 of port D, the way a project's `gpio`
//! line names a pin of the chip (specification 34, TARGET-01).
//!
//! The syntax is the same on every F4 chip. Which pins a chip has, and which of them the firmware
//! keeps for itself, is chip data: see [`Chip::pin`](crate::chip::Chip::pin).

use core::fmt;

/// Number of GPIO ports the F4 family names, `A` to `K`.
pub const PORT_COUNT: usize = 11;
/// Pins in a full GPIO port of the F4 family.
pub const PINS_PER_PORT: u8 = 16;

/// A GPIO port of the F4 family, `A` to `K`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Port(u8);

impl Port {
    /// Port A.
    pub const A: Self = Self(0);
    /// Port B.
    pub const B: Self = Self(1);
    /// Port C.
    pub const C: Self = Self(2);
    /// Port D.
    pub const D: Self = Self(3);
    /// Port E.
    pub const E: Self = Self(4);
    /// Port F.
    pub const F: Self = Self(5);
    /// Port G.
    pub const G: Self = Self(6);
    /// Port H.
    pub const H: Self = Self(7);
    /// Port I.
    pub const I: Self = Self(8);
    /// Port J.
    pub const J: Self = Self(9);
    /// Port K.
    pub const K: Self = Self(10);

    /// The port a letter names, in either case.
    pub const fn from_letter(letter: u8) -> Option<Self> {
        match letter {
            b'A'..=b'K' => Some(Self(letter - b'A')),
            b'a'..=b'k' => Some(Self(letter - b'a')),
            _ => None,
        }
    }

    /// The capital letter that names the port.
    pub const fn letter(self) -> char {
        (b'A' + self.0) as char
    }

    /// Position of the port, 0 for A: the index into per-port tables.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One pin of a GPIO port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    port: Port,
    number: u8,
}

/// A pin as a chip table lists it. The number is taken modulo 16, so a table entry cannot name a
/// pin the family lacks; the table tests compare every entry with its printed name.
pub(crate) const fn at(port: Port, number: u8) -> Pin {
    Pin {
        port,
        number: number % PINS_PER_PORT,
    }
}

impl Pin {
    /// Pin `number` of `port`, or `None` above 15.
    pub const fn new(port: Port, number: u8) -> Option<Self> {
        if number < PINS_PER_PORT {
            Some(Self { port, number })
        } else {
            None
        }
    }

    /// The port of the pin.
    pub const fn port(self) -> Port {
        self.port
    }

    /// The number of the pin within its port, 0 to 15.
    pub const fn number(self) -> u8 {
        self.number
    }

    /// The pin's bit in the 16-bit port registers.
    pub const fn mask(self) -> u16 {
        1 << self.number
    }

    /// Reads a pin name: `P`, a port letter `A` to `K`, and a decimal pin number 0 to 15 without
    /// leading zeros, for example `PD12`. Letters may be capitals or small letters.
    pub const fn parse(name: &str) -> Result<Self, PinNameError> {
        let text = name.as_bytes();
        if text.len() < 3 || text.len() > 4 || (text[0] != b'P' && text[0] != b'p') {
            return Err(PinNameError::Syntax);
        }
        let port = match Port::from_letter(text[1]) {
            Some(port) => port,
            None => return Err(PinNameError::Port),
        };
        let number = match (text[2], text.len()) {
            (digit @ b'0'..=b'9', 3) => digit - b'0',
            (first @ b'1'..=b'9', 4) => match text[3] {
                second @ b'0'..=b'9' => (first - b'0') * 10 + (second - b'0'),
                _ => return Err(PinNameError::Number),
            },
            _ => return Err(PinNameError::Number),
        };
        match Self::new(port, number) {
            Some(pin) => Ok(pin),
            None => Err(PinNameError::Range),
        }
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{}{}", self.port.letter(), self.number)
    }
}

/// Why a text is not a pin name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinNameError {
    /// The text is not `P`, a port letter and one or two digits.
    Syntax,
    /// The port letter is not `A` to `K`.
    Port,
    /// The pin number is not a decimal number without leading zeros.
    Number,
    /// The pin number is above 15.
    Range,
}

impl fmt::Display for PinNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Syntax => "a pin name is P, a port letter and a pin number, for example PD12",
            Self::Port => "the port letter must be A to K",
            Self::Number => "the pin number must be decimal, without leading zeros",
            Self::Range => "the pin number must be 0 to 15",
        })
    }
}

/// Why the firmware keeps a pin for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reserved {
    /// The console UART.
    Console,
    /// The debug port (SWD).
    Debug,
    /// The crystal oscillator.
    Oscillator,
}

impl fmt::Display for Reserved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Console => "the console UART uses it",
            Self::Debug => "the debug port uses it",
            Self::Oscillator => "the crystal oscillator uses it",
        })
    }
}

/// Why a line cannot be used as a pin of the chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinError {
    /// The line is not a pin name.
    Name(PinNameError),
    /// The chip has no such port.
    NoSuchPort(Port),
    /// The chip has the port but not the pin.
    NoSuchPin(Pin),
    /// The firmware keeps the pin for itself.
    Reserved(Pin, Reserved),
    /// Another line already uses the pin.
    AlreadyClaimed(Pin),
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => fmt::Display::fmt(error, f),
            Self::NoSuchPort(port) => write!(f, "the chip has no port {}", port.letter()),
            Self::NoSuchPin(pin) => write!(f, "the chip has no pin {pin}"),
            Self::Reserved(pin, why) => write!(f, "{pin} is not available: {why}"),
            Self::AlreadyClaimed(pin) => write!(f, "{pin} is already used by another line"),
        }
    }
}

/// How an input pin is pulled when nothing drives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pull {
    /// No internal resistor.
    Floating,
    /// Internal pull-up.
    Up,
    /// Internal pull-down.
    Down,
}

/// The pins of the one input and the one output bit of the board's process image, named as
/// `io.toml` names pins. This is all a board configuration says about its wiring.
/// [`Chip::check_wiring`](crate::chip::Chip::check_wiring) checks it against a chip's table, in
/// a constant too.
#[derive(Debug, Clone, Copy)]
pub struct Wiring<'a> {
    /// The input pin, for example a button.
    pub input: &'a str,
    /// The pull of the input pin; the board's own resistors decide what is needed.
    pub input_pull: Pull,
    /// The output pin, for example an LED.
    pub output: &'a str,
    /// Whether the output pin is high when the output is in its safe state.
    pub output_safe_high: bool,
}

/// The pins a firmware has handed out, so that no two lines drive one pin.
#[derive(Debug, Clone, Copy, Default)]
pub struct Claims {
    ports: [u16; PORT_COUNT],
}

impl Claims {
    /// No pin claimed yet.
    pub const fn new() -> Self {
        Self {
            ports: [0; PORT_COUNT],
        }
    }

    /// Marks the pin as used, or reports that it already is.
    pub fn claim(&mut self, pin: Pin) -> Result<(), PinError> {
        let used = self
            .ports
            .get_mut(pin.port().index())
            .ok_or(PinError::NoSuchPort(pin.port()))?;
        if *used & pin.mask() != 0 {
            return Err(PinError::AlreadyClaimed(pin));
        }
        *used |= pin.mask();
        Ok(())
    }

    /// Whether the pin is already used.
    pub fn is_claimed(&self, pin: Pin) -> bool {
        self.ports
            .get(pin.port().index())
            .is_some_and(|used| used & pin.mask() != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    fn pin(name: &str) -> Pin {
        Pin::parse(name).expect("valid pin name")
    }

    #[test]
    fn a_name_is_a_port_and_a_pin_number() {
        assert_eq!(pin("PD12"), Pin::new(Port::D, 12).unwrap());
        assert_eq!(pin("PA0"), Pin::new(Port::A, 0).unwrap());
        assert_eq!(pin("PA15"), Pin::new(Port::A, 15).unwrap());
        assert_eq!(pin("PK9"), Pin::new(Port::K, 9).unwrap());
        assert_eq!(pin("PD12").port(), Port::D);
        assert_eq!(pin("PD12").number(), 12);
        assert_eq!(pin("PD12").mask(), 1 << 12);
        assert_eq!(pin("PA0").mask(), 1);
        assert_eq!(pin("PA15").mask(), 0x8000);
    }

    #[test]
    fn every_pin_of_the_family_round_trips_through_its_name() {
        for letter in b'A'..=b'K' {
            let port = Port::from_letter(letter).unwrap();
            assert_eq!(port.index(), usize::from(letter - b'A'));
            assert_eq!(port.letter(), char::from(letter));
            for number in 0..PINS_PER_PORT {
                let name = std::format!("P{}{number}", char::from(letter));
                let parsed = Pin::parse(&name).unwrap();
                assert_eq!(parsed, Pin::new(port, number).unwrap());
                assert_eq!(parsed.to_string(), name);
            }
        }
        assert_eq!(PORT_COUNT, 11);
    }

    #[test]
    fn small_letters_name_the_same_pin_and_print_as_capitals() {
        assert_eq!(pin("pd12"), pin("PD12"));
        assert_eq!(pin("Pd12"), pin("PD12"));
        assert_eq!(pin("pA0"), pin("PA0"));
        assert_eq!(pin("pk15").to_string(), "PK15");
    }

    #[test]
    fn names_that_are_not_pins_say_why() {
        use PinNameError::{Number, Port as BadPort, Range, Syntax};
        let cases = [
            ("", Syntax),
            ("P", Syntax),
            ("PD", Syntax),
            ("D12", Syntax),
            ("XD12", Syntax),
            ("PD012", Syntax),
            ("PD123", Syntax),
            (" PD12", Syntax),
            ("PD12 ", Syntax),
            ("PL1", BadPort),
            ("PZ1", BadPort),
            ("P11", BadPort),
            ("P-1", BadPort),
            ("PDX", Number),
            ("PD-1", Number),
            ("PD+1", Number),
            ("PD01", Number),
            ("PD00", Number),
            ("PD1X", Number),
            ("PD1.", Number),
            ("PD16", Range),
            ("PD99", Range),
            ("PA32", Range),
        ];
        for (name, error) in cases {
            assert_eq!(Pin::parse(name), Err(error), "{name:?}");
        }
        // Bytes outside ASCII are rejected, not split.
        assert_eq!(Pin::parse("P\u{c4}1"), Err(BadPort));
        assert_eq!(Pin::parse("P\u{e5}1"), Err(BadPort));
        assert_eq!(Pin::parse("PD\u{ff}"), Err(Number));
    }

    #[test]
    fn a_pin_number_above_fifteen_is_not_a_pin() {
        assert_eq!(Pin::new(Port::D, 16), None);
        assert_eq!(Pin::new(Port::D, 255), None);
        assert_eq!(at(Port::D, 12), pin("PD12"));
        assert_eq!(at(Port::D, 28), pin("PD12"));
    }

    #[test]
    fn names_are_checked_in_constants_too() {
        const LED: Result<Pin, PinNameError> = Pin::parse("PD12");
        const BAD: Result<Pin, PinNameError> = Pin::parse("PD16");
        assert_eq!(LED, Ok(pin("PD12")));
        assert_eq!(BAD, Err(PinNameError::Range));
    }

    #[test]
    fn errors_print_a_reason_a_user_can_act_on() {
        assert_eq!(
            PinError::Name(PinNameError::Range).to_string(),
            "the pin number must be 0 to 15"
        );
        assert_eq!(
            PinError::NoSuchPort(Port::J).to_string(),
            "the chip has no port J"
        );
        assert_eq!(
            PinError::NoSuchPin(pin("PI12")).to_string(),
            "the chip has no pin PI12"
        );
        assert_eq!(
            PinError::Reserved(pin("PA2"), Reserved::Console).to_string(),
            "PA2 is not available: the console UART uses it"
        );
        assert_eq!(
            PinError::AlreadyClaimed(pin("PD12")).to_string(),
            "PD12 is already used by another line"
        );
    }

    #[test]
    fn a_pin_is_claimed_once() {
        let mut claims = Claims::new();
        let led = pin("PD12");
        let neighbour = pin("PD13");
        let same_number_elsewhere = pin("PA12");
        assert!(!claims.is_claimed(led));
        assert_eq!(claims.claim(led), Ok(()));
        assert!(claims.is_claimed(led));
        assert_eq!(claims.claim(led), Err(PinError::AlreadyClaimed(led)));
        // Another pin of the port, and the same number on another port, stay free.
        assert!(!claims.is_claimed(neighbour));
        assert!(!claims.is_claimed(same_number_elsewhere));
        assert_eq!(claims.claim(neighbour), Ok(()));
        assert_eq!(claims.claim(same_number_elsewhere), Ok(()));
        // Every pin of the family can be claimed once.
        let mut all = Claims::default();
        for letter in b'A'..=b'K' {
            let port = Port::from_letter(letter).unwrap();
            for number in 0..PINS_PER_PORT {
                let each = Pin::new(port, number).unwrap();
                assert_eq!(all.claim(each), Ok(()));
                assert_eq!(all.claim(each), Err(PinError::AlreadyClaimed(each)));
            }
        }
    }
}
