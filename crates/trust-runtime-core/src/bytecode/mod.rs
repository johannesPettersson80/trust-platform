//! Portable bytecode metadata records.

use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::task::TaskConfig;

/// Bytecode format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytecodeVersion {
    /// Major version. Incompatible changes increment this field.
    pub major: u16,
    /// Minor version. Compatible section extensions increment this field.
    pub minor: u16,
}

impl BytecodeVersion {
    /// Construct a bytecode version pair.
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

/// Supported major bytecode version.
pub const SUPPORTED_MAJOR_VERSION: u16 = 1;
/// Supported minor bytecode version.
pub const SUPPORTED_MINOR_VERSION: u16 = 1;

/// Process image sizing derived from bytecode metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessImageConfig {
    /// Input image byte length.
    pub inputs: usize,
    /// Output image byte length.
    pub outputs: usize,
    /// Marker memory image byte length.
    pub memory: usize,
}

/// Resource metadata captured in a bytecode module.
#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    /// Resource name.
    pub name: SmolStr,
    /// Process image sizing for the resource.
    pub process_image: ProcessImageConfig,
    /// Task definitions associated with the resource.
    pub tasks: Vec<TaskConfig>,
}

/// Bytecode metadata for a configuration.
#[derive(Debug, Clone)]
pub struct BytecodeMetadata {
    /// Bytecode format version.
    pub version: BytecodeVersion,
    /// Resources encoded by the bytecode module.
    pub resources: Vec<ResourceMetadata>,
}

impl BytecodeMetadata {
    /// Lookup a resource by name.
    #[must_use]
    pub fn resource(&self, name: &str) -> Option<&ResourceMetadata> {
        self.resources
            .iter()
            .find(|resource| resource.name.eq_ignore_ascii_case(name))
    }

    /// Return the first resource, if any.
    #[must_use]
    pub fn primary_resource(&self) -> Option<&ResourceMetadata> {
        self.resources.first()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BytecodeMetadata, BytecodeVersion, ProcessImageConfig, ResourceMetadata,
        SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
    };
    use crate::task::TaskConfig;
    use crate::value::Duration;
    use alloc::vec;
    use smol_str::SmolStr;

    #[test]
    fn bytecode_metadata_resource_lookup_is_case_insensitive() {
        let metadata = BytecodeMetadata {
            version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION),
            resources: vec![ResourceMetadata {
                name: SmolStr::new("ResourceA"),
                process_image: ProcessImageConfig {
                    inputs: 1,
                    outputs: 2,
                    memory: 3,
                },
                tasks: vec![TaskConfig {
                    name: SmolStr::new("MainTask"),
                    interval: Duration::from_millis(20),
                    single: None,
                    priority: 1,
                    programs: vec![SmolStr::new("Main")],
                    fb_instances: Vec::new(),
                }],
            }],
        };

        let resource = metadata.resource("resourcea").expect("resource");
        assert_eq!(
            metadata.primary_resource().map(|entry| &entry.name),
            Some(&resource.name)
        );
        assert_eq!(resource.process_image.outputs, 2);
        assert_eq!(resource.tasks[0].interval, Duration::from_millis(20));
    }
}
