use std::path::Path;

use anyhow::{bail, Result};

use crate::software_map::{SoftwareMap, ToolStatus};

pub fn architecture_doctor_full_map(root: &Path) -> Result<()> {
    let scaffold_map = SoftwareMap::new(root.display().to_string());
    let _json = scaffold_map.to_stable_json()?;
    let _known_statuses = ToolStatus::ALL;

    bail!(
        "architecture-doctor --full-map is not implemented yet; scaffold only covers FULLMAP-P1 data model wiring"
    )
}
