//! Compiled programs (specification 44): the program image a device loads and runs where it lies.
//!
//! The engineering computer compiles a project into a native module for the device's CPU and puts
//! it into a program image with every table the kernel and the tools need
//! ([`image`](crate::native::image)). A device
//! checks the image and calls the module's entry points; it never decodes, resolves or lays out a
//! program. The image's deployment record ([`DeploymentRecord::for_image`](crate::deployment::DeploymentRecord::for_image))
//! comes from its own tables.

pub mod admission;
pub mod change;
pub mod fault_place;
mod functions;
pub mod image;
pub mod library;
mod literal;
pub mod live;
pub mod memory;
pub mod program;
mod record;
pub mod reflect;
pub mod retain;
pub mod signature;
mod start;
pub mod stop;
pub mod tool;
mod values;

/// The type table's reader and the decoder of a value's bytes, which a trace's replay reads its
/// located variables with (specification 48 RU-60).
pub(crate) use values::{read_value, Types};
pub mod variables;

pub use functions::{RuntimeReals, KERNEL_API, KERNEL_FUNCTIONS};
