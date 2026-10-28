//! `trust-runtime firmware install`: the first install of truST's firmware on a chip on USB, through
//! the chip's own boot loader (specification 34 TARGET-04). A chip that runs truST is updated over its
//! control connection instead (`trust-runtime ctl firmware`).
//!
//! The firmware folder of a release holds the boot loader, the partition table and the firmware
//! image. The command writes each where the chip file's layout puts it, through the ROM's download
//! mode, and erases the boot loader's selection so that the chip boots the first firmware slot. The
//! program store, the checkpoints and the device settings are left as they are, so a chip keeps its
//! program, its retained values and its settings across a new install; `--reset-settings` also
//! erases the device settings, the way back to a chip whose control token is lost.

use std::path::Path;

use anyhow::{anyhow, bail, Context};
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use espflash::flasher::Flasher;
use espflash::image_format::Segment;
use espflash::target::{Chip, ProgressCallbacks};
use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo};
use trust_chips::{Catalog, ChipFile};

use crate::cli::FirmwareAction;

/// The files of a chip's firmware folder, in the order they are written.
const FILES: [&str; 3] = ["bootloader.bin", "partitions.bin", "firmware.bin"];

/// Runs `trust-runtime firmware`.
pub fn run_firmware(action: FirmwareAction) -> anyhow::Result<()> {
    match action {
        FirmwareAction::Install {
            chip,
            port,
            folder,
            reset_settings,
        } => install(&chip, port, &folder, reset_settings),
    }
}

/// One file of the folder and where the chip's flash holds it.
#[derive(Debug, PartialEq, Eq)]
struct Part {
    name: &'static str,
    address: u32,
    bytes: Vec<u8>,
}

/// The files of `folder` with their flash addresses from the chip file: the boot loader and the
/// firmware begin with the ESP image magic 0xE9 and the partition table with its magic 0xAA 0x50.
fn parts(file: &ChipFile, folder: &Path) -> anyhow::Result<Vec<Part>> {
    let layout = &file.layout;
    let address = |name: &str| match name {
        "bootloader.bin" => layout.bootloader.map(|region| region.start),
        "partitions.bin" => layout.partition_table.map(|region| region.start),
        _ => Some(layout.firmware.start),
    };
    FILES
        .iter()
        .map(|&name| {
            let path = folder.join(name);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("the firmware folder has no {}", path.display()))?;
            let magic: &[u8] = if name == "partitions.bin" {
                &[0xAA, 0x50]
            } else {
                &[0xE9]
            };
            if !bytes.starts_with(magic) {
                bail!("{} is not a {} file of the chip", path.display(), name);
            }
            let address = address(name).ok_or_else(|| {
                anyhow!("the chip file of {} places no {name}", file.identity.name)
            })?;
            Ok(Part {
                name,
                address,
                bytes,
            })
        })
        .collect()
}

/// The serial port of the one chip on USB with the boot loader's identity `vid`:`pid`.
fn find_port(
    ports: &[SerialPortInfo],
    vid: u16,
    pid: u16,
) -> anyhow::Result<(String, UsbPortInfo)> {
    let mut found = ports.iter().filter_map(|port| match &port.port_type {
        SerialPortType::UsbPort(usb) if usb.vid == vid && usb.pid == pid => {
            Some((port.port_name.clone(), usb.clone()))
        }
        _ => None,
    });
    let first = found.next().ok_or_else(|| {
        anyhow!("no chip on USB with the boot loader {vid:04x}:{pid:04x}; give --port")
    })?;
    if let Some(second) = found.next() {
        bail!(
            "two chips on USB with the boot loader {vid:04x}:{pid:04x}, {} and {}; give --port",
            first.0,
            second.0
        );
    }
    Ok(first)
}

/// Installs the firmware in `folder` on the chip `chip` on USB, and erases its device settings and
/// its pairings when `reset_settings` asks for it.
fn install(
    chip: &str,
    port: Option<String>,
    folder: &Path,
    reset_settings: bool,
) -> anyhow::Result<()> {
    let catalog = Catalog::embedded().map_err(|error| anyhow!("the chip files: {error}"))?;
    let file = catalog
        .chip(chip)
        .ok_or_else(|| anyhow!("truST has no chip file of {chip}"))?;
    let parts = parts(file, folder)?;
    let select = file
        .layout
        .firmware_select
        .ok_or_else(|| anyhow!("the chip file of {chip} names no firmware selection"))?;
    let settings = if reset_settings {
        let device = |region: Option<trust_chips::Region>| {
            region.ok_or_else(|| anyhow!("the chip file of {chip} names no device settings"))
        };
        // The pairings are credentials too (specification 11 §6.9.6): a chip whose control token
        // is lost may have lost a pairing token with it.
        let mut regions = vec![device(file.layout.device_a)?, device(file.layout.device_b)?];
        regions.extend(file.layout.pairing_a);
        regions.extend(file.layout.pairing_b);
        regions
    } else {
        Vec::new()
    };
    let usb = file.identity.usb_bootloader;
    let ports = serialport::available_ports().context("list the serial ports")?;
    let (name, info) = match port {
        Some(name) => {
            // A stable name such as /dev/serial/by-id/... is a link to the port the list names.
            let real = std::fs::canonicalize(&name)
                .map_or_else(|_| name.clone(), |path| path.display().to_string());
            let info = ports
                .iter()
                .find(|port| port.port_name == name || port.port_name == real)
                .and_then(|port| match &port.port_type {
                    SerialPortType::UsbPort(info) => Some(info.clone()),
                    _ => None,
                })
                .ok_or_else(|| anyhow!("{name} is not a USB serial port of a chip"))?;
            (name, info)
        }
        None => find_port(&ports, usb.vid, usb.pid)?,
    };
    let target = match chip {
        "esp32c6" => Chip::Esp32c6,
        other => bail!("truST cannot install firmware on {other} through its boot loader yet"),
    };
    let serial = serialport::new(&name, 115_200)
        .timeout(std::time::Duration::from_secs(3))
        .open_native()
        .with_context(|| format!("open {name}"))?;
    let connection = Connection::new(
        serial,
        info,
        ResetAfterOperation::HardReset,
        ResetBeforeOperation::DefaultReset,
        115_200,
    );
    let mut flasher = Flasher::connect(connection, true, true, false, Some(target), None)
        .map_err(|error| anyhow!("the chip's boot loader on {name} did not answer: {error}"))?;
    let segments: Vec<Segment<'_>> = parts
        .iter()
        .map(|part| Segment::new(part.address, &part.bytes))
        .collect();
    println!("installing truST on {chip} at {name}");
    flasher
        .erase_region(select.start, select.kib * 1024)
        .map_err(|error| anyhow!("erase the firmware selection: {error}"))?;
    for region in &settings {
        flasher
            .erase_region(region.start, region.kib * 1024)
            .map_err(|error| anyhow!("erase the device settings: {error}"))?;
    }
    if !settings.is_empty() {
        println!("erased the device settings: the chip boots without a control token or a network");
    }
    flasher
        .write_bins_to_flash(&segments, &mut Progress { parts: &parts })
        .map_err(|error| anyhow!("write the firmware: {error}"))?;
    flasher
        .connection()
        .reset()
        .map_err(|error| anyhow!("reset the chip: {error}"))?;
    println!("installed truST on {chip} at {name}; it starts the firmware now");
    Ok(())
}

/// The progress of a write, a line for each part. espflash counts a part in compressed blocks, so
/// the line names the part's own length.
struct Progress<'a> {
    parts: &'a [Part],
}

impl ProgressCallbacks for Progress<'_> {
    fn init(&mut self, addr: u32, _blocks: usize) {
        match self.parts.iter().find(|part| part.address == addr) {
            Some(part) => println!(
                "writing {} ({} bytes) at 0x{addr:x}",
                part.name,
                part.bytes.len()
            ),
            None => println!("writing at 0x{addr:x}"),
        }
    }

    fn update(&mut self, _current: usize) {}

    fn verifying(&mut self) {}

    fn finish(&mut self, _skipped: bool) {}
}

#[cfg(test)]
#[path = "firmware/tests.rs"]
mod tests;
