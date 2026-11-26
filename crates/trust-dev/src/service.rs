//! `trust-dev service`: the services of the project (specification 48 §13).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::native::rust_pous::{declared_services, service_writes};
use trust_runtime::native::service_bindings::service_bindings;
use trust_runtime::native::PROGRAM_IMAGE_FILE;
use trust_runtime::runtime_core::native::image::Image;

/// `trust-dev service bindings <name> --out <file>`: writes the bindings of the service `name`
/// (specification 48 RU-62) from the image the project's build wrote.
pub fn run_bindings(project: Option<PathBuf>, name: &str, out: &Path) -> anyhow::Result<()> {
    let project = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let declared =
        declared_services(&project).map_err(|message| anyhow::anyhow!("trust.toml: {message}"))?;
    if !declared.iter().any(|service| service == name) {
        bail!("trust.toml declares no [services.{name}]");
    }
    let writes = service_writes(&project)
        .map_err(|message| anyhow::anyhow!("trust.toml: {message}"))?
        .into_iter()
        .find(|service| service.name == name)
        .map(|service| service.writes)
        .unwrap_or_default();
    let image_path = project.join(PROGRAM_IMAGE_FILE);
    let bytes = std::fs::read(&image_path).with_context(|| {
        format!(
            "read {}: build the project first (`trust-runtime build`)",
            image_path.display()
        )
    })?;
    let image = Image::parse(&bytes)
        .map_err(|error| anyhow::anyhow!("{}: {error}", image_path.display()))?;
    let text =
        service_bindings(&image, name, &writes).map_err(|message| anyhow::anyhow!(message))?;
    if let Some(folder) = out.parent().filter(|folder| !folder.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    }
    std::fs::write(out, text).with_context(|| format!("write {}", out.display()))?;
    outln!("Wrote {}: the bindings of service {name}.", out.display());
    Ok(())
}
