//! The C6's USB-Serial/JTAG control link (design §2.1, §2.2 and specification 34, PORT-05): the
//! interrupt glue that moves bytes between the peripheral's 64-byte FIFOs and the shared
//! [`DeviceLineLink`], and the [`ControlLink`] the device runner polls.
//!
//! The packet interrupt drains the OUT FIFO into the link, the IN FIFO's empty interrupt refills
//! it 64 bytes at a time, and a bus reset starts a new host session. The interrupt and the runner
//! enter the one link through the one critical section below, so no byte moves twice. Nothing here
//! calls esp-hal's blocking `write` or `flush`, which spin while no host reads (design §2.2): the
//! link only ever moves bytes the peripheral's FIFOs hold or take now.

use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::{
    handler,
    interrupt::bind_handler,
    peripherals::{Interrupt, USB_DEVICE},
    ram,
};
use trust_runtime_core::ports::{ControlLink, LinkFrame, PeerClass, PortError};
use trust_runtime_services::link::{DeviceLineLink, ReplyRefused};

use super::wait::{now_instant, LinkWake};
use crate::usb_service::{self, Pending, UsbFifo, FIFO_BYTES};

/// `PortError` code of a reply the link refused because another reply is in flight.
const LINK_BUSY: u16 = 1;
/// `PortError` code of a reply the link refused because it is longer than the frame.
const LINK_TOO_LONG: u16 = 2;
/// The `PortError` device number of the one USB-Serial/JTAG link.
const LINK_DEVICE: u16 = 0;

/// The link, in fixed static storage of one frame's bytes per buffer (design §2.1): the interrupt
/// and the runner both enter it through this one critical section.
static LINK: Mutex<RefCell<DeviceLineLink>> =
    Mutex::new(RefCell::new(DeviceLineLink::new(PeerClass::Local)));

/// The USB-Serial/JTAG interrupt: one servicing pass over the FIFOs, in the same critical section
/// as the runner's polls.
///
/// It runs from HP SRAM: the USB controller raises it while the flash cache may be off (a flash
/// operation in progress), so a fetch from flash would fault (design §2.2).
#[handler]
#[ram]
fn usb_link_interrupt() {
    critical_section::with(|cs| {
        let mut fifo = UsbRegs;
        let mut link = LINK.borrow_ref_mut(cs);
        usb_service::service(&mut fifo, &mut link, &LinkWake);
    });
}

/// The peripheral's registers, the [`UsbFifo`] the servicing pass drives. Each method runs from HP
/// SRAM, with the interrupt handler that calls it (design §2.2).
struct UsbRegs;

impl UsbFifo for UsbRegs {
    #[ram]
    fn take_pending(&mut self) -> Pending {
        let regs = USB_DEVICE::regs();
        let status = regs.int_st().read();
        let pending = Pending {
            out_packet: status.serial_out_recv_pkt().bit_is_set(),
            in_empty: status.serial_in_empty().bit_is_set(),
            bus_reset: status.usb_bus_reset().bit_is_set(),
        };
        regs.int_clr().write(|w| {
            if pending.out_packet {
                w.serial_out_recv_pkt().clear_bit_by_one();
            }
            if pending.in_empty {
                w.serial_in_empty().clear_bit_by_one();
            }
            if pending.bus_reset {
                w.usb_bus_reset().clear_bit_by_one();
            }
            w
        });
        pending
    }

    #[ram]
    fn read_out(&mut self, out: &mut [u8]) -> usize {
        let regs = USB_DEVICE::regs();
        let mut read = 0;
        while let Some(slot) = out.get_mut(read) {
            if !regs
                .ep1_conf()
                .read()
                .serial_out_ep_data_avail()
                .bit_is_set()
            {
                break;
            }
            *slot = regs.ep1().read().rdwr_byte().bits();
            read += 1;
        }
        read
    }

    #[ram]
    fn write_in(&mut self, bytes: &[u8]) -> usize {
        let regs = USB_DEVICE::regs();
        let mut written = 0;
        while let Some(&byte) = bytes.get(written) {
            if written == FIFO_BYTES {
                break;
            }
            if !regs.ep1_conf().read().serial_in_ep_data_free().bit_is_set() {
                break;
            }
            // SAFETY: `RDWR_BYTE` is the FIFO's eight-bit data field and every eight-bit value is
            // valid in it; the loop has checked that the IN FIFO has room for the byte.
            unsafe { regs.ep1().write(|w| w.rdwr_byte().bits(byte)) };
            written += 1;
        }
        // `WR_DONE` makes the bytes just written available to the host. With an empty FIFO it
        // sends the zero-length packet that ends a full one, as ESP-IDF's driver flushes too.
        regs.ep1_conf().modify(|_, w| w.wr_done().set_bit());
        written
    }

    #[ram]
    fn set_in_empty_enabled(&mut self, enabled: bool) {
        USB_DEVICE::regs().int_ena().modify(|_, w| {
            if enabled {
                w.serial_in_empty().set_bit();
            } else {
                w.serial_in_empty().clear_bit();
            }
            w
        });
    }

    #[ram]
    fn set_out_packet_enabled(&mut self, enabled: bool) {
        USB_DEVICE::regs().int_ena().modify(|_, w| {
            if enabled {
                w.serial_out_recv_pkt().set_bit();
            } else {
                w.serial_out_recv_pkt().clear_bit();
            }
            w
        });
    }
}

/// The C6's control link: the interrupt glue above and the runner's
/// [`ControlLink`](trust_runtime_core::ports::ControlLink) over the same link.
pub struct UsbLink {
    _private: (),
}

impl UsbLink {
    /// Binds the USB-Serial/JTAG interrupt to the link, clears its events, and enables the
    /// packet-received and bus-reset interrupts.
    ///
    /// Call once, after [`Board::take`](super::Board::take): the console driver that takes the
    /// same peripheral disables its interrupts as it is built.
    #[must_use]
    pub fn bind() -> Self {
        let regs = USB_DEVICE::regs();
        // No event of an earlier session starts a pass. The IN FIFO's empty interrupt is enabled
        // by `send_frame` while a reply is unsent and disabled once nothing is unsent (design
        // §2.2), so it starts disabled.
        regs.int_clr().write(|w| {
            w.serial_out_recv_pkt().clear_bit_by_one();
            w.serial_in_empty().clear_bit_by_one();
            w.usb_bus_reset().clear_bit_by_one();
            w
        });
        regs.int_ena().modify(|_, w| {
            w.serial_out_recv_pkt().set_bit();
            w.usb_bus_reset().set_bit();
            w.serial_in_empty().clear_bit();
            w
        });
        bind_handler(Interrupt::USB_DEVICE, usb_link_interrupt);
        Self { _private: () }
    }
}

impl ControlLink for UsbLink {
    fn max_frame_len(&self) -> usize {
        DeviceLineLink::MAX_FRAME_LEN
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        critical_section::with(|cs| {
            let mut fifo = UsbRegs;
            let mut link = LINK.borrow_ref_mut(cs);
            // The handler leaves bytes in the OUT FIFO when the receive ring is full and stops
            // asking for them; the loop takes them here, once it has room, and asks again.
            let _ = usb_service::drain_out(&mut fifo, &mut link);
            fifo.set_out_packet_enabled(true);
            link.poll_frame(buffer, now_instant())
        })
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        critical_section::with(|cs| {
            let mut fifo = UsbRegs;
            let mut link = LINK.borrow_ref_mut(cs);
            link.send_frame(frame, now_instant())
                .map_err(refused_error)?;
            // The reply is unsent: the IN FIFO's empty interrupt writes it 64 bytes at a time.
            fifo.set_in_empty_enabled(true);
            Ok(())
        })
    }
}

/// The `PortError` of a reply the link refused: another reply was in flight, or the reply was
/// longer than the frame (specification 34, §7.3).
const fn refused_error(reason: ReplyRefused) -> PortError {
    let code = match reason {
        ReplyRefused::Busy => LINK_BUSY,
        ReplyRefused::TooLong => LINK_TOO_LONG,
    };
    PortError {
        code,
        device: LINK_DEVICE,
    }
}
