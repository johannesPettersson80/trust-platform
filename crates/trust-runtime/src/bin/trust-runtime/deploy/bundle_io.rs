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
    // The program image is admitted as the computer admits it at its start (specification 44
    // §10.2, §8.8).
    trust_runtime::native::admit_on_this_computer(&bundle.image)?;
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
