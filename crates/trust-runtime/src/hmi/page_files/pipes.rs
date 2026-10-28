//! Pipes in page and popup files (specification 39, §6 and §15.2): two ports, a line style, the
//! bends the user placed and a flow binding. Whether the ports exist is a check against the page
//! and the catalogue (`binding_checks`); here each end must be written `object.port`.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use super::load::{error_at, line_at, PageFileError, RawBinding};
use super::types::{HmiPipe, HmiPipeStyle};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPipe {
    from: Spanned<String>,
    to: Spanned<String>,
    #[serde(default)]
    style: HmiPipeStyle,
    #[serde(default)]
    waypoints: Vec<[i64; 2]>,
    #[serde(default)]
    bind: BTreeMap<String, RawBinding>,
}

/// Every pipe end names an object and one of its ports.
pub(super) fn check_pipes(
    file: &str,
    text: &str,
    pipes: &[RawPipe],
) -> Result<(), Vec<PageFileError>> {
    let errors: Vec<PageFileError> = pipes
        .iter()
        .flat_map(|pipe| [&pipe.from, &pipe.to])
        .filter(|end| !is_port(end.get_ref()))
        .map(|end| {
            error_at(
                file,
                line_at(text, end.span().start),
                format!(
                    "a pipe end is written object.port, for example \"p-101.outlet\", not \"{}\"",
                    end.get_ref()
                ),
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// `object.port`: an object id and a port name, neither empty, with one dot between them.
fn is_port(end: &str) -> bool {
    end.split_once('.')
        .is_some_and(|(object, port)| !object.is_empty() && !port.is_empty() && !port.contains('.'))
}

impl RawPipe {
    pub(super) fn into_pipe(self) -> HmiPipe {
        HmiPipe {
            from: self.from.into_inner(),
            to: self.to.into_inner(),
            style: self.style,
            waypoints: self.waypoints,
            bind: self
                .bind
                .into_iter()
                .map(|(name, binding)| (name, binding.0))
                .collect(),
        }
    }
}
