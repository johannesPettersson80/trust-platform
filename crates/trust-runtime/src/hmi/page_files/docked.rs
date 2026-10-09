//! Panels docked on pages (specification 39, §21.1 item 3). A panel file with `[panel]` names the
//! pages it stands on and the edge it docks to; the loader composes it into every page it applies
//! to, identically for the runtime display, `hmi check` and the editor. A docked panel binds no
//! instance.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::faceplates::HmiFaceplate;
use super::inline_faceplates::PLACED_FACEPLATE;
use super::regions::HmiRegion;
use super::types::HmiPage;

/// Which pages a docked panel stands on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiDockScope {
    /// None: the panel is placed by hand only.
    #[default]
    Manual,
    /// Every page.
    All,
    /// The pages it lists.
    Selected,
}

/// The screen edge a docked panel takes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiDockEdge {
    /// The left edge, its full height.
    #[default]
    Left,
    /// The right edge, its full height.
    Right,
    /// The top edge, its full width.
    Top,
    /// The bottom edge, its full width.
    Bottom,
}

/// A panel file's `[panel]`: where it docks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiDock {
    /// The pages it stands on.
    #[serde(default)]
    pub scope: HmiDockScope,
    /// The pages, for `selected`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<String>,
    /// Its edge.
    #[serde(default)]
    pub dock: HmiDockEdge,
    /// Its place among the panels on the same page, from the outside in.
    #[serde(default)]
    pub order: u32,
}

impl HmiDock {
    /// Checks the page list: only with `selected`, valid page names, each once.
    pub fn validate(&self) -> Result<(), String> {
        if self.scope != HmiDockScope::Selected && !self.pages.is_empty() {
            return Err("choose scope = \"selected\" before listing pages".into());
        }
        if self.pages.iter().any(|page| {
            page.is_empty()
                || page.len() > 64
                || !page
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        }) {
            return Err("a docked panel lists valid page names".into());
        }
        if self.pages.iter().collect::<BTreeSet<_>>().len() != self.pages.len() {
            return Err("a page is listed twice".into());
        }
        Ok(())
    }

    /// Whether the panel stands on the page of this name.
    #[must_use]
    pub fn applies(&self, page: &str) -> bool {
        match self.scope {
            HmiDockScope::Manual => false,
            HmiDockScope::All => true,
            HmiDockScope::Selected => self.pages.iter().any(|listed| listed == page),
        }
    }
}

/// Composes the docked panels that apply to the page `name`: each panel's objects and pipes join
/// the page under ids prefixed with the panel's name, in a region at its edge, and the page's own
/// content keeps its coordinates in the area that remains (the first region). A page no panel
/// applies to is returned unchanged.
pub fn compose_docked(
    mut page: HmiPage,
    name: &str,
    screen: [u32; 2],
    panels: &[(String, HmiFaceplate)],
) -> Result<HmiPage, String> {
    let mut selected: Vec<(&String, &HmiFaceplate, &HmiDock)> = panels
        .iter()
        .filter_map(|(panel_name, panel)| {
            let dock = panel.panel.as_ref().filter(|dock| dock.applies(name))?;
            Some((panel_name, panel, dock))
        })
        .collect();
    if selected.is_empty() {
        return Ok(page);
    }
    selected.sort_by(|a, b| a.2.order.cmp(&b.2.order).then_with(|| a.0.cmp(b.0)));
    let main_objects: Vec<String> = page
        .objects
        .iter()
        .map(|object| object.id.clone())
        .collect();
    let main_pipes: Vec<usize> = (0..page.pipes.len()).collect();
    let mut ids: BTreeSet<String> = main_objects.iter().cloned().collect();
    // The area left for the page's own content: x, y, width, height.
    let mut area = [0_i64, 0, i64::from(screen[0]), i64::from(screen[1])];
    let mut regions = Vec::new();
    for (panel_name, panel, dock) in selected {
        if !panel.fields.is_empty() || panel.settings {
            return Err(format!(
                "the docked panel {panel_name} cannot require an equipment instance"
            ));
        }
        let (at, size) = match dock.dock {
            HmiDockEdge::Left | HmiDockEdge::Right => {
                let extent = i64::from(panel.popup.size[0]);
                if extent >= area[2] {
                    return Err(format!(
                        "the docked panel {panel_name} leaves no room for the page"
                    ));
                }
                let x = if dock.dock == HmiDockEdge::Left {
                    area[0]
                } else {
                    area[0] + area[2] - extent
                };
                let region = ([x, area[1]], [extent, area[3]]);
                if dock.dock == HmiDockEdge::Left {
                    area[0] += extent;
                }
                area[2] -= extent;
                region
            }
            HmiDockEdge::Top | HmiDockEdge::Bottom => {
                let extent = i64::from(panel.popup.size[1]);
                if extent >= area[3] {
                    return Err(format!(
                        "the docked panel {panel_name} leaves no room for the page"
                    ));
                }
                let y = if dock.dock == HmiDockEdge::Top {
                    area[1]
                } else {
                    area[1] + area[3] - extent
                };
                let region = ([area[0], y], [area[2], extent]);
                if dock.dock == HmiDockEdge::Top {
                    area[1] += extent;
                }
                area[3] -= extent;
                region
            }
        };
        let prefix = format!("__panel_{}_{panel_name}__", panel_name.len());
        let first_pipe = page.pipes.len();
        let mut panel_ids = Vec::new();
        for original in &panel.popup.page.objects {
            if original.kind == PLACED_FACEPLATE {
                return Err(format!(
                    "the docked panel {panel_name} cannot contain another placed panel"
                ));
            }
            let mut object = original.clone();
            object.id = format!("{prefix}{}", original.id);
            if !ids.insert(object.id.clone()) {
                return Err(format!(
                    "an object of the docked panel {panel_name} has the id of a page object"
                ));
            }
            object.group = original
                .group
                .as_ref()
                .map(|group| format!("{prefix}{group}"));
            object.attach = original.attach.as_ref().map(|attach| {
                match attach
                    .strip_prefix("pipe:")
                    .and_then(|index| index.parse::<usize>().ok())
                {
                    Some(index) => format!("pipe:{}", first_pipe + index),
                    None => format!("{prefix}{attach}"),
                }
            });
            object.measures = original.measures.as_ref().map(|id| format!("{prefix}{id}"));
            object.acts_on = original.acts_on.as_ref().map(|id| format!("{prefix}{id}"));
            panel_ids.push(object.id.clone());
            page.objects.push(object);
        }
        for original in &panel.popup.page.pipes {
            let mut pipe = original.clone();
            pipe.from = format!("{prefix}{}", original.from);
            pipe.to = format!("{prefix}{}", original.to);
            page.pipes.push(pipe);
        }
        regions.push(HmiRegion {
            panel: Some(panel_name.clone()),
            at,
            size: [to_size(size[0]), to_size(size[1])],
            objects: panel_ids,
            pipes: (first_pipe..page.pipes.len()).collect(),
        });
    }
    regions.insert(
        0,
        HmiRegion {
            panel: None,
            at: [area[0], area[1]],
            size: [to_size(area[2]), to_size(area[3])],
            objects: main_objects,
            pipes: main_pipes,
        },
    );
    page.regions = regions;
    Ok(page)
}

fn to_size(extent: i64) -> u32 {
    u32::try_from(extent.max(0)).unwrap_or(u32::MAX)
}
