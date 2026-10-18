//! The control link over the STM32U5A9's OTG_HS (design §2.2 and §2.3; specification 34, PORT-05):
//! the USB device, the static line link that the OTG_HS interrupt and the runner both enter in one
//! critical section, the interrupt itself, and the [`ControlLink`] the runner polls. The runner's
//! clock is passed in: the monotonic clock, wait and wake of the runner are S1b-8u-2b. The wake
//! counter this module advances is the one that wait reads.
//!
//! The bus, the descriptors and the class are in [`crate::usb`], and the servicing pass in
//! [`crate::usb_service`]. The chip build compiles this module only.

use core::cell::RefCell;
use core::ptr::{addr_of_mut, read_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cortex_m::peripheral::NVIC;
use critical_section::Mutex;
use stm32u5::stm32u5a9::{self as pac, interrupt};
use synopsys_usb_otg::UsbBus as OtgBus;
use trust_runtime_core::ports::{
    ControlLink, LinkFrame, MonotonicClock, PeerClass, PortError, WakeSignal,
};
use trust_runtime_services::link::{DeviceLineLink, ReplyRefused};
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid};
use usb_device::LangID;
use usbd_serial::USB_CLASS_CDC;

use crate::usb::{
    bcd_device, serial_text, ControlClass, OtgHs, MANUFACTURER, PRODUCT, PRODUCT_ID, SERIAL_DIGITS,
    VENDOR_ID,
};
use crate::usb_service::LinkService;

/// The chip's 96-bit unique ID: three words at this address, in address order (design §2.5, S1b
/// §2.4).
const UID_ADDRESS: usize = 0x0BFA_0700;
/// The words of the endpoint memory the bus carves its endpoint buffers from, 0.5 KiB (design
/// §2.4).
const ENDPOINT_MEMORY_WORDS: usize = 128;
/// `PortError` code of a reply the link refused because another reply is in flight.
const LINK_BUSY: u16 = 1;
/// `PortError` code of a reply the link refused because it is longer than the frame.
const LINK_TOO_LONG: u16 = 2;
/// The `PortError` device number of the one control link.
const LINK_DEVICE: u16 = 0;

/// The memory the USB stack keeps for the rest of the program: the bus allocator, the endpoint
/// memory the allocator carves, and the serial text the device names itself with. [`bind`] fills
/// it once, and the device borrows it for `'static`, because usb-device borrows the allocator for
/// the device's whole life.
struct Storage {
    bus: Option<UsbBusAllocator<OtgBus<OtgHs>>>,
    endpoint_memory: [u32; ENDPOINT_MEMORY_WORDS],
    serial: [u8; SERIAL_DIGITS],
}

/// The USB stack's storage. Only [`bind`] takes a reference to it, once, under [`BOUND`].
static mut STORAGE: Storage = Storage {
    bus: None,
    endpoint_memory: [0; ENDPOINT_MEMORY_WORDS],
    serial: [0; SERIAL_DIGITS],
};

/// The USB device, its control class and the servicing state, which the interrupt and the runner
/// both reach through [`DEVICE`].
struct Device {
    usb: UsbDevice<'static, OtgBus<OtgHs>>,
    class: ControlClass<'static, OtgBus<OtgHs>>,
    service: LinkService,
}

/// The device, once bound. Both the interrupt and the runner enter it under the critical section.
static DEVICE: Mutex<RefCell<Option<Device>>> = Mutex::new(RefCell::new(None));

/// The shared line link (specification 34, PORT-05). The interrupt and the runner enter it under
/// the same critical section as [`DEVICE`], so no byte moves twice.
static LINK: Mutex<RefCell<DeviceLineLink>> =
    Mutex::new(RefCell::new(DeviceLineLink::new(PeerClass::Local)));

/// Set by the first [`bind`]: the chip has one control link, so a second bind is refused.
static BOUND: AtomicBool = AtomicBool::new(false);

/// The wake counter (PORT-03): every servicing pass that completes a request or moves a reply
/// advances it, and the runner's wait ends when it moves.
static WAKES: AtomicU32 = AtomicU32::new(0);

/// Why the control link could not be bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindError {
    /// The link is bound already: the chip has one control link.
    AlreadyBound,
    /// The crate's version does not start with `major.minor` below 100 each.
    Version,
    /// The bus refused the device's strings.
    Descriptors,
    /// The serial text is not UTF-8. It is ASCII by construction, so this is a defect.
    Serial,
}

/// The control link of the STM32U5A9's OTG_HS: the runner's [`ControlLink`] over the shared line
/// link (specification 34, PORT-05). It is made by [`bind`].
pub struct UsbLink<C> {
    clock: C,
}

/// Binds the control link to the OTG_HS: builds the USB device with truST's identity (design
/// §2.5), its CDC-ACM control class and the static line link, and enables the OTG_HS interrupt. The
/// device connects when its bus starts, so the interrupt is unmasked after the device is in place.
///
/// `clock` is the runner's monotonic clock; the link's stall rule reads it.
///
/// # Errors
///
/// [`BindError`] names the step that refused. A bind that refuses after it takes the storage leaves
/// the chip without a control link until reset; with this crate's own values that cannot happen.
pub fn bind<C: MonotonicClock>(clock: C) -> Result<UsbLink<C>, BindError> {
    let bcd = bcd_device(env!("CARGO_PKG_VERSION")).ok_or(BindError::Version)?;
    let serial = serial_text(read_uid());
    if BOUND.swap(true, Ordering::AcqRel) {
        return Err(BindError::AlreadyBound);
    }
    // SAFETY: `BOUND` was false until this call, so this is the first and only reference to
    // `STORAGE` the program makes. It lives for the rest of the program, and nothing else reaches
    // it: the device borrows its parts for `'static`.
    let storage: &'static mut Storage = unsafe { &mut *addr_of_mut!(STORAGE) };
    storage.serial = serial;
    let bus: &'static UsbBusAllocator<OtgBus<OtgHs>> = storage
        .bus
        .insert(OtgBus::new(OtgHs, &mut storage.endpoint_memory));
    let serial_str = core::str::from_utf8(&storage.serial).map_err(|_| BindError::Serial)?;
    // The class allocates its interfaces and endpoints before the device freezes the allocator.
    let class = ControlClass::new(bus);
    let usb = UsbDeviceBuilder::new(bus, UsbVidPid(VENDOR_ID, PRODUCT_ID))
        .device_class(USB_CLASS_CDC)
        .device_release(bcd)
        .strings(&[StringDescriptors::new(LangID::EN)
            .manufacturer(MANUFACTURER)
            .product(PRODUCT)
            .serial_number(serial_str)])
        .map_err(|_| BindError::Descriptors)?
        .build();
    critical_section::with(|cs| {
        *DEVICE.borrow_ref_mut(cs) = Some(Device {
            usb,
            class,
            service: LinkService::default(),
        });
    });
    // SAFETY: the vector table has the `OTG_HS` handler below, and the device it serves is in place.
    unsafe { NVIC::unmask(pac::Interrupt::OTG_HS) };
    Ok(UsbLink { clock })
}

/// The chip's 96-bit unique ID: its three words in address order (design §2.5).
fn read_uid() -> [u32; 3] {
    let words = UID_ADDRESS as *const u32;
    // SAFETY: 0x0BFA_0700 is the read-only unique ID of the STM32U5A9 (design §2.5), readable at any
    // time. Volatile reads keep the compiler from merging or dropping them.
    unsafe {
        [
            read_volatile(words),
            read_volatile(words.add(1)),
            read_volatile(words.add(2)),
        ]
    }
}

/// The OTG_HS interrupt: one `usb-device` poll, which reports the bus events and the packets, then
/// one servicing pass over the link (design §2.2). It runs in the same critical section as the
/// runner's calls.
#[allow(
    non_snake_case,
    reason = "the vector's symbol is the PAC's interrupt name"
)]
#[interrupt]
fn OTG_HS() {
    critical_section::with(|cs| {
        let mut slot = DEVICE.borrow_ref_mut(cs);
        let mut link = LINK.borrow_ref_mut(cs);
        if let Some(device) = slot.as_mut() {
            device.usb.poll(&mut [&mut device.class]);
        }
        service(slot.as_mut(), &mut link);
    });
}

/// One servicing pass over the device, if it is bound, and the OUT interrupt the pass asks for.
fn service(device: Option<&mut Device>, link: &mut DeviceLineLink) {
    if let Some(device) = device {
        let receive = device.service.pass(&mut device.class, link, &LinkWake);
        set_out_interrupt(receive);
    }
}

/// Enables the OTG_HS receive interrupt (`GINTMSK.RXFLVLM`) while the link can take an OUT packet,
/// and masks it while it cannot, so the peripheral keeps its packet without raising the interrupt
/// again and again (design §2.2).
fn set_out_interrupt(enabled: bool) {
    // SAFETY: `OTG_HS` is the chip's own register block (PAC 0.16.0). The read-modify-write touches
    // only the RXFLVLM bit of GINTMSK, and runs under the critical section every caller holds. The
    // USB driver writes GINTMSK once, in its enable, before the device exists.
    let otg = unsafe { &*pac::OTG_HS::ptr() };
    otg.gintmsk().modify(|_, w| w.rxflvlm().bit(enabled));
}

/// The wake counter's advance, for the servicing pass.
struct LinkWake;

impl WakeSignal for LinkWake {
    fn wake(&self) {
        WAKES.fetch_add(1, Ordering::Release);
    }
}

impl<C: MonotonicClock> ControlLink for UsbLink<C> {
    fn max_frame_len(&self) -> usize {
        DeviceLineLink::MAX_FRAME_LEN
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let now = self.clock.now();
        critical_section::with(|cs| {
            let mut slot = DEVICE.borrow_ref_mut(cs);
            let mut link = LINK.borrow_ref_mut(cs);
            let frame = link.poll_frame(buffer, now);
            // Taking a request frees room: the pass moves the packets that were waiting for it.
            service(slot.as_mut(), &mut link);
            frame
        })
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        let now = self.clock.now();
        critical_section::with(|cs| -> Result<(), PortError> {
            let mut slot = DEVICE.borrow_ref_mut(cs);
            let mut link = LINK.borrow_ref_mut(cs);
            link.send_frame(frame, now).map_err(refused_error)?;
            // The pass writes the reply as far as the endpoint takes it now; the rest follows on
            // the endpoint's interrupts and the runner's polls.
            service(slot.as_mut(), &mut link);
            Ok(())
        })
    }

    fn sending(&self) -> bool {
        // While a reply is in flight the runner services the link at least once a millisecond
        // (specification 34, PORT-05).
        critical_section::with(|cs| LINK.borrow_ref(cs).reply_in_flight())
    }
}

/// The `PortError` of a reply the link refused: another reply was in flight, or it was longer than
/// the frame (specification 34, §7.3).
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
