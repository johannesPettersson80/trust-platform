//! The I/O snapshot's view of an image (specification 44 §13.3): each binding is the address the
//! parser makes of its text, with its variable's wire type and declared name.

use crate::io_address::IoAddress;
use crate::native::image::{Image, Target};
use crate::native::program::tests::image;
use crate::stdlib::conversions::ConversionType;

/// The test image binds `MAIN.OUT`, a `USINT`, to `%QX0.1`. The binding equals the address parsed
/// from `%QX0.1`, so a force or a snapshot names it as a client does, and it decodes as a `USINT`.
/// Catches an address that prints as `%QX1` or never matches a client's force.
#[test]
fn a_binding_is_the_parsed_address_with_its_variables_type_and_name() {
    let bytes = image(0, 100, 64, Target::X86_64);
    let image = Image::parse(&bytes).expect("the image parses");
    let (bindings, names) = super::io_bindings(&image).expect("the bindings read");
    assert_eq!(bindings.len(), 1);
    assert_eq!(
        bindings[0].address,
        IoAddress::parse("%QX0.1").expect("the address parses")
    );
    assert_eq!(bindings[0].codec.wire_type, Some(ConversionType::USInt));
    assert_eq!(names, [Some("MAIN.OUT".into())]);
}
