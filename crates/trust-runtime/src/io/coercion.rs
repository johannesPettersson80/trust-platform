fn portable_io_type(
    value_type: TypeId,
) -> Option<trust_runtime_core::stdlib::conversions::ConversionType> {
    value_type
        .builtin_name()
        .and_then(trust_runtime_core::stdlib::conversions::ConversionType::from_builtin_name)
}
pub fn io_value_type_name(value_type: TypeId) -> Option<&'static str> {
    trust_runtime_core::io_image::io_value_type_name(portable_io_type(value_type)?)
}
fn coerce_binding_from_io(
    value: Value,
    binding: &IoBinding,
    codec: &IoBindingCodec,
) -> Result<Value, RuntimeError> {
    let target = codec
        .wire_type
        .or(binding.value_type)
        .map(|ty| portable_io_type(ty).ok_or(RuntimeError::TypeMismatch))
        .transpose()?;
    trust_runtime_core::io_image::coerce_binding_from_io(value, target, codec.enum_type.as_ref())
}
fn coerce_binding_to_io(
    value: Value,
    binding: &IoBinding,
    codec: &IoBindingCodec,
) -> Result<Value, RuntimeError> {
    let target = codec
        .wire_type
        .or(binding.value_type)
        .map(|ty| portable_io_type(ty).ok_or(RuntimeError::TypeMismatch))
        .transpose()?;
    trust_runtime_core::io_image::coerce_binding_to_io(
        value,
        target,
        codec.enum_type.as_ref(),
        binding.address.size,
    )
}
