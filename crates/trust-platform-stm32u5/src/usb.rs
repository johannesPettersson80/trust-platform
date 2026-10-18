//! The USB identity and the control class of the STM32U5A9 (design §2.1, §2.2 and §2.5;
//! specification 34, PORT-05): the descriptor values truST sets and their pure builders, the
//! CDC-ACM class that carries the shared line link, and the OTG_HS peripheral the chip's USB bus
//! drives. The servicing pass is in [`crate::usb_service`]; the interrupt and the static link are in
//! the chip-only module `usb_link`.

use usb_device::bus::{StringIndex, UsbBus, UsbBusAllocator};
use usb_device::class::{ControlIn, ControlOut, UsbClass};
use usb_device::descriptor::DescriptorWriter;
use usb_device::{LangID, Result as UsbResult};
use usbd_serial::CdcAcmClass;

use crate::usb_service::{CdcPort, PACKET_BYTES};

/// The USB vendor ID of the development build (design §2.5, decision O2).
pub const VENDOR_ID: u16 = 0x1209;
/// The USB product ID of the development build (design §2.5, decision O2: 1209:0001 in development).
pub const PRODUCT_ID: u16 = 0x0001;
/// The manufacturer string (design §2.5).
pub const MANUFACTURER: &str = "truST";
/// The product string (design §2.5).
pub const PRODUCT: &str = "truST runtime";
/// The name of the communication interface, which the control class carries (design §2.5).
pub const INTERFACE: &str = "truST control";
/// The digits of the serial number: the chip's 96-bit unique ID as three 32-bit words (design §2.5).
pub const SERIAL_DIGITS: usize = 24;
/// The OTG_HS register block (design §2.1: the header's `USB_OTG_HS_BASE_NS`, 0x4204_0000).
pub const OTG_HS_BASE: usize = 0x4204_0000;
/// The OTG RAM of the peripheral, 4 KiB of endpoint FIFOs, in 32-bit words (design §2.1, DS13543
/// §3.55).
pub const OTG_FIFO_WORDS: usize = 4096 / 4;
/// The device endpoints of the peripheral, EP0 to EP8: the PAC's OTG_HS block has `DIEPCTL0` to
/// `DIEPCTL8` (PAC 0.16.0, `stm32u5a9/otg_hs.rs`).
pub const OTG_ENDPOINTS: usize = 9;

/// The serial number of the device: the three words of the chip's unique ID in address order, each
/// as eight uppercase hexadecimal digits, most significant first (design §2.5, S1b §2.4). The bench
/// F407 of specification 34 D.4 reads `0034002D3431471535363630` for its three words.
#[must_use]
pub fn serial_text(words: [u32; 3]) -> [u8; SERIAL_DIGITS] {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut text = [0u8; SERIAL_DIGITS];
    for (index, digit) in text.iter_mut().enumerate() {
        let shift = 28 - 4 * (index % 8);
        let nibble = (words[index / 8] >> shift) & 0xF;
        *digit = HEX[nibble as usize];
    }
    text
}

/// The `bcdDevice` of truST's version: the major version in the high byte and the minor version in
/// the low byte, each in binary-coded decimal (USB 2.0 §9.6.1). The patch number is not part of it.
/// `None` when the text does not start with `major.minor`, or a number is above 99 and does not fit
/// two BCD digits.
#[must_use]
pub fn bcd_device(version: &str) -> Option<u16> {
    let mut parts = version.split('.');
    let major = bcd_byte(parts.next()?)?;
    let minor = bcd_byte(parts.next()?)?;
    Some((u16::from(major) << 8) | u16::from(minor))
}

/// One decimal number of at most two digits, in binary-coded decimal.
fn bcd_byte(part: &str) -> Option<u8> {
    let value: u8 = part.parse().ok()?;
    (value <= 99).then_some(((value / 10) << 4) | (value % 10))
}

/// The control class: usbd-serial's packet-level CDC-ACM class, which holds no bytes of its own. The
/// link's rings and its reply are the only buffers (design §2.2). The class also records the bus
/// reset that usb-device reports to it, for the servicing pass.
pub struct ControlClass<'a, B: UsbBus> {
    cdc: CdcAcmClass<'a, B>,
    bus_reset: bool,
}

impl<'a, B: UsbBus> ControlClass<'a, B> {
    /// Allocates the class's two interfaces and three endpoints from `alloc`, and names the
    /// communication interface `truST control` (design §2.5). usb-device needs the class allocated
    /// before the device is built.
    pub fn new(alloc: &'a UsbBusAllocator<B>) -> Self {
        Self {
            cdc: CdcAcmClass::new_with_interface_names(
                alloc,
                PACKET_BYTES as u16,
                Some(INTERFACE),
                None,
            ),
            bus_reset: false,
        }
    }
}

impl<B: UsbBus> UsbClass<B> for ControlClass<'_, B> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> UsbResult<()> {
        self.cdc.get_configuration_descriptors(writer)
    }

    fn get_string(&self, index: StringIndex, lang_id: LangID) -> Option<&str> {
        self.cdc.get_string(index, lang_id)
    }

    fn reset(&mut self) {
        // usb-device resets every class at a bus reset; the servicing pass reads the report.
        self.bus_reset = true;
        self.cdc.reset();
    }

    fn control_in(&mut self, xfer: ControlIn<B>) {
        self.cdc.control_in(xfer);
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {
        self.cdc.control_out(xfer);
    }
}

impl<B: UsbBus> CdcPort for ControlClass<'_, B> {
    // The `None` of both packet methods is usb-device's WouldBlock: no packet waits, or the endpoint
    // still holds one. Its other errors need a packet longer than the endpoint, which the servicing
    // pass never reads or sends.
    fn read_packet(&mut self, out: &mut [u8]) -> Option<usize> {
        self.cdc.read_packet(out).ok()
    }

    fn write_packet(&mut self, bytes: &[u8]) -> Option<usize> {
        self.cdc.write_packet(bytes).ok()
    }

    fn dtr(&self) -> bool {
        self.cdc.dtr()
    }

    fn take_bus_reset(&mut self) -> bool {
        core::mem::take(&mut self.bus_reset)
    }
}

/// The OTG_HS peripheral of the STM32U5A9 for the synopsys-usb-otg register driver (design §2.2).
/// Only the chip has its registers, so the type exists in the chip build.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use otg::OtgHs;

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod otg {
    use stm32u5::stm32u5a9 as pac;
    use synopsys_usb_otg::{PhyType, UsbPeripheral};

    use super::{OTG_ENDPOINTS, OTG_FIFO_WORDS, OTG_HS_BASE};
    use crate::clock::SYSCLK_HZ;

    /// The OTG_HS of the STM32U5A9, driven in full-speed mode on its internal full-speed PHY.
    pub struct OtgHs;

    // SAFETY: the trait's contract is an STM32-style OTG register block at `REGISTERS`, with the FIFO
    // depth, the endpoint count and the clock the driver needs. Each value is the chip's own, as the
    // constants above and design §2.1 and §2.2 give them, and the register block is the one the PAC
    // lists at that address.
    unsafe impl UsbPeripheral for OtgHs {
        const REGISTERS: *const () = OTG_HS_BASE as *const ();
        // Full speed is the design's choice (§2.1): the class and the relay stay the same on both
        // STM32 chips, and the internal PHY is driven at full speed.
        const HIGH_SPEED: bool = false;
        const FIFO_DEPTH_WORDS: usize = OTG_FIFO_WORDS;
        const ENDPOINT_COUNT: usize = OTG_ENDPOINTS;

        /// Turns on the peripheral's AHB2 clock and the USB supply, before the driver configures the
        /// core (design §2.2).
        fn enable() {
            // SAFETY: RCC and PWR are the chip's own register blocks at their addresses (PAC 0.16.0).
            // Each access is a read-modify-write of bits this crate owns: OTGEN in RCC_AHB2ENR1 (bit
            // 14), USV in PWR_SVMCR (bit 28), and USBPWREN and USBBOOSTEN in PWR_VOSR (bits 19 and 20).
            let rcc = unsafe { &*pac::RCC::ptr() };
            rcc.ahb2enr1().modify(|_, w| w.otgen().set_bit());
            let pwr = unsafe { &*pac::PWR::ptr() };
            // USV is "mandatory to use the USB OTG peripheral" (PAC 0.16.0, `pwr/svmcr.rs`).
            pwr.svmcr().modify(|_, w| w.usv().present());
            // USBPWREN and USBBOOSTEN: the booster "must be set in range 1 and range 2 before enabling
            // the USB peripheral" (PAC 0.16.0, `pwr/vosr.rs`); the chip runs in range 1 (`clock.rs`).
            // RM0456's USB section owns the exact order, which is unverified here (design §2.2).
            pwr.vosr()
                .modify(|_, w| w.usbpwren().enabled().usbboosten().enabled());
        }

        fn ahb_frequency_hz(&self) -> u32 {
            // The AHB clock is SYSCLK undivided (`clock.rs`, HPRE = 1).
            SYSCLK_HZ
        }

        fn phy_type(&self) -> PhyType {
            // The internal full-speed PHY (design §2.1).
            PhyType::InternalFullSpeed
        }
    }
}
