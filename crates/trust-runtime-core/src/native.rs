//! Compiled programs (specification 44): the program image a device loads and runs where it lies.
//!
//! The engineering computer compiles a project into a native module for the device's CPU and puts
//! it into a program image with every table the kernel and the tools need
//! ([`image`](crate::native::image)). A device
//! checks the image and calls the module's entry points; it never decodes, resolves or lays out a
//! program. The image's deployment record ([`DeploymentRecord::for_image`](crate::deployment::DeploymentRecord::for_image))
//! comes from its own tables.

pub mod image;
pub mod live;
pub mod program;
mod record;
