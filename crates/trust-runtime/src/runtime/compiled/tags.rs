//! The MQTT tags of a compiled program on a computer (specification 44 §13.3; specification 32): a
//! tag bound at a process-image address that the program does not bind lies past the program's own
//! process image, in bytes that the drivers read and drive with the image, and the boundaries of
//! the scan copy it from and to its variable, as the bytecode engine's binding does.

use std::sync::{Arc, Mutex, MutexGuard};

use smol_str::SmolStr;
use trust_runtime_core::io_image::{read_flat_image, write_flat_image, IoValueCodec};
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::live::symbol_codec;
use trust_runtime_core::native::variables::CompiledVariables;

use super::CompiledRuntime;
use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSize, MqttTagLink};
use crate::memory::IoArea;

/// The process-image bytes past the program's own, which the MQTT tags bind.
#[derive(Debug, Default)]
pub(crate) struct ExtraImages {
    pub(crate) inputs: Vec<u8>,
    pub(crate) outputs: Vec<u8>,
}

/// The extra images, which the process I/O and the boundaries of the scan share; only the scan
/// thread takes them.
pub(crate) type SharedExtraImages = Arc<Mutex<ExtraImages>>;

/// The extra images, also after a thread that held them panicked: they stay valid bytes.
pub(crate) fn lock(images: &Mutex<ExtraImages>) -> MutexGuard<'_, ExtraImages> {
    images
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A tag's variable, and where its value lies in the extra image of its area.
#[derive(Debug, Clone)]
pub(crate) struct TagLink {
    name: SmolStr,
    area: IoArea,
    /// The address within the extra image.
    address: IoAddress,
    codec: IoValueCodec,
}

/// The bytes of the process image area `area` of `image`.
pub(super) fn image_bytes(image: &Image<'_>, area: IoArea) -> usize {
    let id = match area {
        IoArea::Input => trust_plc_abi::AREA_INPUTS,
        IoArea::Output => trust_plc_abi::AREA_OUTPUTS,
        IoArea::Memory => trust_plc_abi::AREA_MARKERS,
    };
    image
        .area_length(id)
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
}

/// The bytes a value of `size` takes in an image.
fn width(size: IoSize) -> u32 {
    match size {
        IoSize::Bit | IoSize::Byte => 1,
        IoSize::Word => 2,
        IoSize::DWord => 4,
        IoSize::LWord => 8,
        IoSize::Bytes(bytes) => bytes,
    }
}

impl CompiledRuntime {
    /// The bytes of the program's own process image of `area`.
    #[must_use]
    pub fn process_image_bytes(&self, area: IoArea) -> usize {
        image_bytes(&self.image.image(), area)
    }

    /// The process-image addresses at which the program binds the variable whose symbol is
    /// `symbol`.
    #[must_use]
    pub fn bound_addresses(&self, symbol: u32) -> Vec<IoAddress> {
        self.bindings
            .iter()
            .filter(|binding| binding.ref_idx == symbol)
            .map(|binding| binding.address.clone())
            .collect()
    }

    /// The representation in a process image of the variable whose symbol is `symbol`, as the
    /// program's own bindings have it.
    #[must_use]
    pub fn io_codec(&self, symbol: u32) -> IoValueCodec {
        symbol_codec(&self.image.image(), symbol)
    }

    /// Binds the MQTT tags of `links`, which lie past the program's own process image: the drivers
    /// read and drive their bytes with the image from now on, and each scan writes an input tag's
    /// value into its variable after the drivers read the inputs and an output tag's variable into
    /// its bytes after the scan.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidConfig`] for a link within the program's own process image or in the
    /// marker image.
    pub fn link_mqtt_tags(&mut self, links: &[MqttTagLink]) -> Result<(), RuntimeError> {
        for link in links {
            let area = link.address.area;
            let base = u32::try_from(self.process_image_bytes(area)).unwrap_or(u32::MAX);
            let byte = link
                .address
                .byte
                .checked_sub(base)
                .filter(|_| area != IoArea::Memory)
                .ok_or_else(|| {
                    RuntimeError::InvalidConfig(
                        format!(
                            "MQTT tag '{}' lies within the program's process image",
                            link.name
                        )
                        .into(),
                    )
                })?;
            let address = IoAddress {
                byte,
                path: vec![byte],
                ..link.address.clone()
            };
            let end = usize::try_from(byte.saturating_add(width(address.size))).unwrap_or(0);
            {
                let mut images = lock(&self.extra);
                let image = match area {
                    IoArea::Input => &mut images.inputs,
                    _ => &mut images.outputs,
                };
                if image.len() < end {
                    image.resize(end, 0);
                }
            }
            self.tags.push(TagLink {
                name: link.name.clone(),
                area,
                address,
                codec: link.codec.clone(),
            });
        }
        Ok(())
    }
}

/// Writes each input tag's value, decoded from the extra input image, into its variable: after the
/// drivers read the inputs, before the program samples them.
pub(super) fn apply_input_tags(
    tags: &[TagLink],
    images: &Mutex<ExtraImages>,
    variables: &mut dyn CompiledVariables,
) -> Result<(), RuntimeError> {
    let images = lock(images);
    for tag in tags.iter().filter(|tag| tag.area == IoArea::Input) {
        let value = tag
            .codec
            .decode(read_flat_image(&images.inputs, &tag.address)?)?;
        variables.write_named(&tag.name, &value)?;
    }
    Ok(())
}

/// Writes each output tag's variable, encoded, into the extra output image: after the scan,
/// before the outputs reach the drivers.
pub(super) fn capture_output_tags(
    tags: &[TagLink],
    images: &Mutex<ExtraImages>,
    variables: &mut dyn CompiledVariables,
) -> Result<(), RuntimeError> {
    let mut images = lock(images);
    for tag in tags.iter().filter(|tag| tag.area == IoArea::Output) {
        let value = tag
            .codec
            .encode(variables.read_named(&tag.name)?, tag.address.size)?;
        write_flat_image(&mut images.outputs, &tag.address, value)?;
    }
    Ok(())
}
