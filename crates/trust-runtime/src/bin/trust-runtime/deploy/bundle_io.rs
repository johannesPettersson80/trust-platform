fn validate_bundle(bundle: &RuntimeBundle) -> anyhow::Result<()> {
    let registry = IoDriverRegistry::default_registry();
    for driver in &bundle.io.drivers {
        if !driver.enabled {
            continue;
        }
        registry
            .validate(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?;
    }
    let sources = if bundle.root.join("src").is_dir() {
        trust_runtime::bundle_builder::collect_project_source_files(&bundle.root, None)?
    } else {
        Vec::new()
    };
    let module = trust_runtime::bytecode::BytecodeModule::decode(&bundle.bytecode)?;
    trust_runtime::harness::CompileSession::from_sources(sources)
        .build_runtime_from_bytecode_with_limits(
            &module,
            Some(&bundle.runtime.resource_name),
            bundle.preparation_limits,
        )?;
    Ok(())
}

#[cfg(test)]
fn update_symlink(link: &Path, target: &Path) -> anyhow::Result<()> {
    let target = target.canonicalize().map_err(|error| {
        anyhow::anyhow!("resolve deployment target '{}': {error}", target.display())
    })?;
    replace_symlink(link, &target)?;
    Ok(())
}
