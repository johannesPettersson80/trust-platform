//! The identity that a `modbus-tcp` driver expects of its device (specification 11, "Compare with the
//! project"): three optional texts of the driver's `params`, which only `Compare with project` reads.

use serde_json::json;

use super::{advanced, field, optional, CommFieldSchema};

/// The expected `vendor_name`, `product_code` and `revision`: advanced, optional and empty by default, so that
/// the form of a driver that states none is unchanged, and a Save of the form keeps what a driver states.
pub(super) fn expected_identity_fields() -> Vec<CommFieldSchema> {
    vec![
        advanced(optional(field(
            "vendor_name",
            "Expected vendor name",
            "string",
            json!(""),
            false,
            "Vendor name that the device should report in its Modbus device identification. Leave empty to accept any. Only Compare with project uses it.",
        ))),
        advanced(optional(field(
            "product_code",
            "Expected product code",
            "string",
            json!(""),
            false,
            "Product code that the device should report in its Modbus device identification. Leave empty to accept any. Only Compare with project uses it.",
        ))),
        advanced(optional(field(
            "revision",
            "Expected revision",
            "string",
            json!(""),
            false,
            "Revision that the device should report in its Modbus device identification. Leave empty to accept any. Only Compare with project uses it.",
        ))),
    ]
}
