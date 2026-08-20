pub struct DeployResult {
    pub current_bundle: PathBuf,
}

pub fn run_deploy(
    bundle: PathBuf,
    root: Option<PathBuf>,
    label: Option<String>,
) -> anyhow::Result<DeployResult> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner().template("{spinner} {msg}")?);
    spinner.enable_steady_tick(std::time::Duration::from_millis(120));
    spinner.set_message("Deploying project...");
    let source_bundle = RuntimeBundle::load(&bundle)?;
    validate_bundle(&source_bundle)?;
    let root = root.unwrap_or(std::env::current_dir()?);
    let bundles_dir = root.join("bundles");
    let deployments_dir = root.join("deployments");
    fs::create_dir_all(&bundles_dir)?;
    fs::create_dir_all(&deployments_dir)?;

    let current_link = root.join("current");
    let previous_link = root.join("previous");
    validate_pointer_slot(&current_link)?;
    validate_pointer_slot(&previous_link)?;
    let current_target = resolve_existing_bundle_link(&bundles_dir, &current_link)?;

    let bundle_name = label.unwrap_or_else(default_bundle_label);
    validate_bundle_label(&bundle_name)?;
    let dest = bundles_dir.join(&bundle_name);
    if dest.exists() {
        anyhow::bail!("deployment already exists: {}", dest.display());
    }
    validate_summary_slots(&deployments_dir, &bundle_name)?;

    let summary = match (|| -> anyhow::Result<BundleChangeSummary> {
        copy_bundle(&source_bundle, &dest)?;
        let bundle = RuntimeBundle::load(&dest)?;
        validate_bundle(&bundle)?;
        let previous_bundle = current_target
            .as_ref()
            .map(RuntimeBundle::load)
            .transpose()?;
        BundleChangeSummary::new(previous_bundle.as_ref(), &bundle)
    })() {
        Ok(summary) => summary,
        Err(error) => {
            let _ = fs::remove_dir_all(&dest);
            return Err(error);
        }
    };

    let (old_current_state, old_previous_state) = match update_deployment_pointers(
        &current_link,
        &previous_link,
        &dest,
        current_target.as_ref(),
    ) {
        Ok(states) => states,
        Err(error) => {
            if let Err(cleanup_error) = fs::remove_dir_all(&dest) {
                return Err(error.context(format!(
                    "also failed to remove rejected bundle '{}': {cleanup_error}",
                    dest.display()
                )));
            }
            return Err(error);
        }
    };

    if let Err(error) = write_summary(&deployments_dir, &bundle_name, &summary) {
        let pointer_restore = restore_pointer_pair(
            &current_link,
            &old_current_state,
            &previous_link,
            &old_previous_state,
        );
        let candidate_cleanup = fs::remove_dir_all(&dest);
        if let Err(restore_error) = pointer_restore {
            return Err(error.context(format!(
                "also failed to restore deployment pointers: {restore_error:#}"
            )));
        }
        if let Err(cleanup_error) = candidate_cleanup {
            return Err(error.context(format!(
                "also failed to remove rejected bundle '{}': {cleanup_error}",
                dest.display()
            )));
        }
        return Err(error);
    }

    summary.print();
    prune_bundles(
        &bundles_dir,
        &bundle_targets(&dest, current_target.as_ref()),
    )?;

    spinner.finish_and_clear();
    println!(
        "{}",
        style::success(format!(
            "Deployed project {} -> {}",
            bundle_name,
            dest.display()
        ))
    );
    println!("Current project version: {}", current_link.display());
    Ok(DeployResult {
        current_bundle: current_link,
    })
}

pub fn run_rollback(root: Option<PathBuf>) -> anyhow::Result<()> {
    let root = root.unwrap_or(std::env::current_dir()?);
    let bundles_dir = root.join("bundles");
    let current_link = root.join("current");
    let previous_link = root.join("previous");
    let current_target = resolve_existing_bundle_link(&bundles_dir, &current_link)?
        .ok_or_else(|| anyhow::anyhow!("no current project link at {}", current_link.display()))?;
    let previous_target =
        resolve_existing_bundle_link(&bundles_dir, &previous_link)?.ok_or_else(|| {
            anyhow::anyhow!(
                "no previous project link at {} (nothing to rollback)",
                previous_link.display()
            )
        })?;

    swap_deployment_pointers(
        &current_link,
        &previous_link,
        &current_target,
        &previous_target,
    )?;

    println!(
        "{}",
        style::success(format!(
            "Rolled back to project {}",
            previous_target.display()
        ))
    );
    println!("Current project version: {}", current_link.display());
    Ok(())
}
