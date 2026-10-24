//! The admission of a program image on a device (specification 44 §10.2): the checks that run
//! before an image runs, in their order, the first failure naming its reason. The image is read
//! where it lies and nothing is allocated.

use core::fmt;

use smol_str::SmolStr;
use trust_plc_abi::INTERFACE_VERSION;

use super::image::{Image, ImageError, IoBindRecord, Target};
use super::library::{library_hash, PROGRAM_LIBRARY};
use super::signature::{self, SignatureError};
use crate::error::RuntimeError;

/// What a device offers a program (specification 44 §10.2, steps 2 and 6).
#[derive(Clone, Copy, Debug)]
pub struct Device {
    /// The device's CPU.
    pub target: Target,
    /// The bytes the device has for programs.
    pub program_memory: u64,
    /// The bytes of the task stack.
    pub task_stack: u64,
    /// The statements one activation may execute (specification 44 §8.4).
    pub max_statements: u32,
    /// The public keys the device trusts (§5.11): only an image one of them signed is admitted; with
    /// none, every image, its signature unchecked.
    pub trusted_keys: &'static [[u8; 32]],
}

/// Why a device refuses an image; the first failed check names its reason (specification 44
/// §10.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The image is damaged or inconsistent.
    Image(ImageError),
    /// The image is for another CPU.
    Target {
        /// The image's target number.
        image: u32,
        /// The device's.
        device: Target,
    },
    /// The image was built against another kernel interface.
    Interface {
        /// The image's version.
        image: u16,
    },
    /// The image links a program library this kernel does not know (§10.2, step 2).
    Library {
        /// The library hash of the image's header.
        image: [u8; 16],
        /// The hash of this kernel's library for the device's CPU.
        kernel: [u8; 16],
    },
    /// The I/O binding of this index lies outside its process image (§10.2, step 5).
    Binding(u32),
    /// The image's signature is malformed, does not verify, or is not by a key the device trusts
    /// (§10.2, step 4).
    Signature(SignatureError),
    /// The program needs more memory than the device has.
    Memory {
        /// The program's memory figure.
        needs: u64,
        /// The device's memory for programs.
        has: u64,
    },
    /// The program's stack bound exceeds the task stack.
    Stack {
        /// The bound.
        needs: u64,
        /// The task stack.
        has: u64,
    },
    /// An entry point the platform cannot reach.
    Entry(u32),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(f),
            Self::Target { image, device } => write!(
                f,
                "the image is for target {image}, this device is {device:?}: rebuild the project for it"
            ),
            Self::Interface { image } => write!(
                f,
                "kernel interface {image} required, this runtime has {INTERFACE_VERSION}: rebuild the project"
            ),
            Self::Library { image, kernel } => {
                f.write_str("the program was built with another program library (")?;
                write_hex(f, image)?;
                write!(f, "), this runtime has {PROGRAM_LIBRARY} (")?;
                write_hex(f, kernel)?;
                f.write_str("): rebuild the project with this release's toolchain")
            }
            Self::Binding(index) => write!(
                f,
                "I/O binding {index} lies outside its process image"
            ),
            Self::Signature(SignatureError::Malformed) => {
                f.write_str("the program's signature section is malformed")
            }
            Self::Signature(SignatureError::Invalid) => f.write_str(
                "the program's signature does not verify: it was changed after signing",
            ),
            Self::Signature(SignatureError::Untrusted) => {
                f.write_str("the program is not signed by a key this device trusts")
            }
            Self::Memory { needs, has } => write!(
                f,
                "insufficient memory: the program needs {needs} bytes, the device has {has}"
            ),
            Self::Stack { needs, has } => write!(
                f,
                "insufficient stack: the program needs {needs} bytes, the task stack has {has}"
            ),
            Self::Entry(offset) => write!(f, "the entry point at code offset {offset} cannot be reached"),
        }
    }
}

impl From<Refusal> for RuntimeError {
    fn from(refusal: Refusal) -> Self {
        RuntimeError::InvalidBundle(SmolStr::new(alloc::format!("{refusal}")))
    }
}

/// Admits the image in `bytes` on `device`: its header and the program library it links, its section
/// table, its signature against the device's trusted keys, its tables and I/O bindings, its memory
/// figure and its stack bound (specification 44 §10.2, steps 2 to 6). The image is read where it
/// lies and nothing is allocated.
///
/// # Errors
///
/// The first [`Refusal`].
pub fn admit<'i>(bytes: &'i [u8], device: &Device) -> Result<Image<'i>, Refusal> {
    let image = Image::parse(bytes).map_err(Refusal::Image)?;
    let header = *image.header();
    if Target::from_raw(header.target) != Some(device.target) {
        return Err(Refusal::Target {
            image: header.target,
            device: device.target,
        });
    }
    if u32::from(header.interface_version) != INTERFACE_VERSION {
        return Err(Refusal::Interface {
            image: header.interface_version,
        });
    }
    let kernel = library_hash(PROGRAM_LIBRARY, device.target);
    if header.library != kernel {
        return Err(Refusal::Library {
            image: header.library,
            kernel,
        });
    }
    signature::check(&image, device.trusted_keys).map_err(Refusal::Signature)?;
    image.check_tables().map_err(Refusal::Image)?;
    check_bindings(&image)?;
    let figure = image.memory_figure();
    if figure.total > device.program_memory {
        return Err(Refusal::Memory {
            needs: figure.total,
            has: device.program_memory,
        });
    }
    if figure.stack > device.task_stack {
        return Err(Refusal::Stack {
            needs: figure.stack,
            has: device.task_stack,
        });
    }
    Ok(image)
}

/// Every I/O binding inside its process image, the inputs, the outputs or the markers, as the area
/// table sizes them (§10.2, step 5).
fn check_bindings(image: &Image<'_>) -> Result<(), Refusal> {
    let bindings = image.table::<IoBindRecord>().map_err(Refusal::Image)?;
    for (index, binding) in (0u32..).zip(bindings.iter()) {
        // A bit and a byte take one byte, a word two, a double word four, a long word eight.
        let width = match binding.size {
            0 | 1 => 1,
            2 => 2,
            3 => 4,
            _ => 8,
        };
        let length = image.area_length(binding.area).unwrap_or(0);
        if u64::from(binding.byte) + width > length {
            return Err(Refusal::Binding(index));
        }
    }
    Ok(())
}

/// `bytes` in lowercase hexadecimal.
fn write_hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    bytes.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
}
