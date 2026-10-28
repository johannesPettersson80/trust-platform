//! Derived shared-panel viewports. These describe presentation only, never PLC ownership.
use super::{
    types::HmiPage,
    write::{entry, optional, string},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A viewport containing named page objects and indexed pipes in local coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiRegion {
    /// Shared panel definition name; absent for the main page content.
    #[serde(default)]
    pub panel: Option<String>,
    /// Viewport origin on the screen.
    pub at: [i64; 2],
    /// Viewport dimensions.
    pub size: [u32; 2],
    /// Objects displayed in this region, in the page's original z-order.
    pub objects: Vec<String>,
    /// Pipe indices into the page's pipe array.
    pub pipes: Vec<usize>,
}
pub(super) fn check(page: &HmiPage) -> Result<(), String> {
    if page.regions.is_empty() {
        return Ok(());
    }
    if page.regions.len() > 64 {
        return Err("a page supports at most 64 layout regions".into());
    }
    let known: BTreeSet<_> = page
        .objects
        .iter()
        .map(|object| object.id.as_str())
        .collect();
    let mut objects = BTreeSet::new();
    let mut pipes = BTreeSet::new();
    for region in &page.regions {
        if region.size.contains(&0) || region.at.iter().any(|position| *position < 0) {
            return Err("layout regions need a nonnegative origin and positive size".into());
        }
        for id in &region.objects {
            if !known.contains(id.as_str()) || !objects.insert(id.as_str()) {
                return Err("layout regions must name each page object exactly once".into());
            }
        }
        for index in &region.pipes {
            if *index >= page.pipes.len() || !pipes.insert(*index) {
                return Err("layout regions must name each page pipe exactly once".into());
            }
        }
    }
    if objects.len() != known.len() || pipes.len() != page.pipes.len() {
        return Err("layout regions leave page content unassigned".into());
    }
    Ok(())
}
pub(super) fn write(out: &mut String, regions: &[HmiRegion]) {
    for region in regions {
        out.push_str("\n[[regions]]\n");
        optional(out, "panel", region.panel.as_deref());
        entry(out, "at", &format!("[{}, {}]", region.at[0], region.at[1]));
        entry(
            out,
            "size",
            &format!("[{}, {}]", region.size[0], region.size[1]),
        );
        entry(
            out,
            "objects",
            &format!(
                "[{}]",
                region
                    .objects
                    .iter()
                    .map(|id| string(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        entry(
            out,
            "pipes",
            &format!(
                "[{}]",
                region
                    .pipes
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }
}
