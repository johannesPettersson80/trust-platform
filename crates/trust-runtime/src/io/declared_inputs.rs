//! The bytes of the input image that a driver of `io.toml` fills, from its parsed parameters
//! (specification 42, §5 item 4). The drivers declare these bytes while they run, and `validate`,
//! which constructs no driver, reads them from the parameters to warn of composed drivers that
//! fill the same bytes, as the start does (specification 11 §6.5).

use std::ops::Range;

use trust_runtime_services::io_drivers::analog::AnalogParams;
use trust_runtime_services::io_drivers::can::CanParams;
use trust_runtime_services::io_drivers::counter::CounterParams;
use trust_runtime_services::io_drivers::modbus::ModbusTcpParams;
use trust_runtime_services::io_drivers::mqtt::MqttParams;

/// The input image from its first byte: a driver that fills it from its start.
fn from_the_start() -> Vec<Range<usize>> {
    std::iter::once(0..usize::MAX).collect()
}

/// The bytes from `offset` for `len` bytes, saturating where they would pass the end of memory.
fn span(offset: u32, len: u32) -> Range<usize> {
    let start = usize::try_from(offset).unwrap_or(usize::MAX);
    let len = usize::try_from(len).unwrap_or(usize::MAX);
    start..start.saturating_add(len)
}

/// A Modbus/TCP driver's: each input point its own bytes, and without input points the block of
/// registers, which fills the image from its start.
pub(crate) fn modbus_tcp(params: &ModbusTcpParams) -> Vec<Range<usize>> {
    if params.input_points.is_empty() {
        return from_the_start();
    }
    super::joined_bytes(
        params
            .input_points
            .iter()
            .map(|point| span(point.image_offset, point.data_type.image_bytes()))
            .collect(),
    )
}

/// An MQTT driver's: each input point its own bytes, and a raw input topic the image from its
/// start. A driver that subscribes to no raw input topic and has no input points, such as one
/// without input mappings or one with the Sparkplug B profile, which takes only its node's
/// commands (specification 32 §4, §9), fills none.
pub(crate) fn mqtt(params: &MqttParams) -> Vec<Range<usize>> {
    if !params.input_points.is_empty() {
        return super::joined_bytes(
            params
                .input_points
                .iter()
                .map(|point| span(point.image_offset, point.data_type.image_bytes()))
                .collect(),
        );
    }
    if params.input_enabled() && params.sparkplug.is_none() {
        from_the_start()
    } else {
        Vec::new()
    }
}

/// An analog driver's: each input channel its `%IW` word.
pub(crate) fn analog(params: &AnalogParams) -> Vec<Range<usize>> {
    super::joined_bytes(
        params
            .inputs
            .iter()
            .map(|input| span(input.address.byte, 2))
            .collect(),
    )
}

/// A counter driver's: each count its `%ID` double word.
pub(crate) fn counter(params: &CounterParams) -> Vec<Range<usize>> {
    super::joined_bytes(
        params
            .inputs
            .iter()
            .map(|input| span(input.address.byte, 4))
            .collect(),
    )
}

/// A CAN driver's: each receive its `length` bytes from its `%IB` address.
pub(crate) fn can(params: &CanParams) -> Vec<Range<usize>> {
    super::joined_bytes(
        params
            .receive
            .iter()
            .map(|receive| span(receive.address.byte, u32::from(receive.length)))
            .collect(),
    )
}

/// An EtherCAT driver's: the modules' input image of `expected_input_bytes`, from the image's start.
pub(crate) fn ethercat(expected_input_bytes: usize) -> Vec<Range<usize>> {
    (expected_input_bytes > 0)
        .then_some(0..expected_input_bytes)
        .into_iter()
        .collect()
}

/// The loopback driver's, which copies the output image over the input image: from the image's
/// start as far as an output image of `outputs` bytes reaches, and nothing without one.
pub(crate) fn loopback(outputs: usize) -> Vec<Range<usize>> {
    (outputs > 0).then_some(0..outputs).into_iter().collect()
}
