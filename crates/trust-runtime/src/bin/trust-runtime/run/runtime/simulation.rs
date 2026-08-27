fn build_simulation_plan(
    bundle: Option<&RuntimeBundle>,
    simulation: bool,
    time_scale: u32,
) -> anyhow::Result<SimulationPlan> {
    if time_scale == 0 {
        anyhow::bail!("--time-scale must be >= 1");
    }

    let mut simulation_config = bundle
        .and_then(|bundle| bundle.simulation.clone())
        .unwrap_or_default();
    if simulation || time_scale > 1 {
        simulation_config.enabled = true;
    }
    if time_scale > 1 {
        simulation_config.time_scale = time_scale;
    }
    if simulation_config.time_scale == 0 {
        anyhow::bail!("simulation.time_scale must be >= 1");
    }

    let enabled = simulation_config.enabled;
    let time_scale = simulation_config.time_scale.max(1);
    if enabled {
        if let Some(driver) = bundle
            .into_iter()
            .flat_map(|bundle| &bundle.io.drivers)
            .find(|driver| driver.enabled && !is_nonphysical_driver(driver.name.as_str()))
        {
            anyhow::bail!(
                "simulation mode rejects enabled physical I/O driver '{}'; disable physical I/O before starting simulation",
                driver.name
            );
        }
    }
    let warning = simulation_warning_message(enabled, time_scale).unwrap_or_default();
    let active_revision = bundle
        .map(|bundle| trust_runtime::simulation::file_content_revision(bundle.root.join("simulation.toml")))
        .transpose()?;
    let controller = enabled.then(|| {
        let controller = trust_runtime::simulation::SimulationController::new(simulation_config);
        match active_revision {
            Some(revision) => controller.with_active_configuration_revision(revision),
            None => controller,
        }
    });

    Ok(SimulationPlan {
        enabled,
        time_scale,
        warning,
        controller,
    })
}

fn is_nonphysical_driver(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "simulated" | "sim" | "noop" | "loopback"
    )
}
