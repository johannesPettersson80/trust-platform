use super::{
    coerce_binding_from_io, coerce_binding_to_io, expected_size_for_type,
    validate_process_image_address, IoEnumBinding,
};
use crate::{
    error::RuntimeError,
    io_address::{IoAddress, IoSize},
    stdlib::conversions::ConversionType,
    value::Value,
};

/// Reusable declared-type conversion for an image binding; not an execution admission token.
#[derive(Debug, Clone, Default)]
pub struct IoValueCodec {
    /// Scalar wire category, or no conversion for an untyped binding.
    pub wire_type: Option<ConversionType>,
    /// Declared enum identity and admitted variants, when applicable.
    pub enum_type: Option<IoEnumBinding>,
    /// Maximum narrow-string payload length, when applicable.
    pub string_capacity: Option<u16>,
}
impl IoValueCodec {
    /// Validate the binding address and apply declared string capacity.
    pub fn address(&self, mut address: IoAddress) -> Result<IoAddress, RuntimeError> {
        if let Some(capacity) = self.string_capacity {
            address.size = IoSize::Bytes(u32::from(capacity));
        } else if let Some(ty) = self.wire_type
            && expected_size_for_type(ty) != Some(address.size)
        {
            return Err(RuntimeError::TypeMismatch);
        }
        validate_process_image_address(&address)?;
        Ok(address)
    }
    /// Convert a sampled image value to the declared IEC value.
    pub fn decode(&self, value: Value) -> Result<Value, RuntimeError> {
        coerce_binding_from_io(value, self.wire_type, self.enum_type.as_ref())
    }
    /// Convert a declared IEC value to its image representation.
    pub fn encode(&self, value: Value, size: IoSize) -> Result<Value, RuntimeError> {
        coerce_binding_to_io(value, self.wire_type, self.enum_type.as_ref(), size)
    }
}

/// One binding of a program image's process image (specification 44 §5.6): its address, the
/// variable it binds and the codec of the variable's type.
#[derive(Debug, Clone)]
pub struct PreparedIoBinding {
    /// Validated physical or hierarchical image address.
    pub address: IoAddress,
    /// The symbol of the bound variable in the image's symbol table.
    pub symbol: u32,
    /// Prepared value conversion for this binding.
    pub codec: IoValueCodec,
}
