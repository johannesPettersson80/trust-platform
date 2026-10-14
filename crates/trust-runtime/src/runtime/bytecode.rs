//! Regression corpus for admitted resource metadata replacement.
#[cfg(test)]
use {crate::task::TaskConfig, smol_str::SmolStr};
#[cfg(test)]
fn legacy_placeholder_resource(
    metadata: &crate::bytecode::BytecodeMetadata,
) -> Option<&crate::bytecode::ResourceMetadata> {
    let resource = metadata.resources.first()?;
    (metadata.resources.len() == 1 && resource.name.eq_ignore_ascii_case("RESOURCE"))
        .then_some(resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytecode::{
        BytecodeMetadata, BytecodeVersion, ProcessImageConfig, ResourceMetadata,
        SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION,
    };
    use crate::value::Duration;

    #[test]
    fn legacy_placeholder_requires_one_resource_named_resource() {
        let placeholder = metadata(vec![resource("rEsOuRcE")]);
        assert_eq!(
            legacy_placeholder_resource(&placeholder).map(|resource| resource.name.as_str()),
            Some("rEsOuRcE")
        );

        for resources in [
            Vec::new(),
            vec![resource("Plant")],
            vec![resource("RESOURCE"), resource("Other")],
        ] {
            assert!(legacy_placeholder_resource(&metadata(resources)).is_none());
        }
    }

    #[test]
    fn resource_metadata_accepts_case_only_program_reference_and_preserves_task_spelling() {
        let mut builder = crate::RuntimeBuilder::new();
        builder.register_program(crate::task::ProgramDef {
            name: "Main".into(),
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            body: Vec::new(),
        });
        let mut runtime = builder.build().unwrap();
        let mut selected = resource("Plant");
        selected.tasks.push(TaskConfig {
            name: "Fast".into(),
            interval: Duration::from_millis(10),
            single: None,
            priority: 1,
            programs: vec!["mAiN".into()],
            fb_instances: Vec::new(),
        });

        runtime.apply_resource_metadata(&selected).unwrap();

        assert_eq!(runtime.resource_name(), "Plant");
        assert_eq!(runtime.tasks().len(), 1);
        assert_eq!(runtime.tasks()[0].programs, vec![SmolStr::new("mAiN")]);
    }

    fn metadata(resources: Vec<ResourceMetadata>) -> BytecodeMetadata {
        BytecodeMetadata {
            version: BytecodeVersion::new(SUPPORTED_MAJOR_VERSION, SUPPORTED_MINOR_VERSION),
            resources,
        }
    }

    fn resource(name: &str) -> ResourceMetadata {
        ResourceMetadata {
            name: name.into(),
            process_image: ProcessImageConfig::default(),
            tasks: Vec::new(),
        }
    }
}
