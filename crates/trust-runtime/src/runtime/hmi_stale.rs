//! Values of links without data (specification 42, §5 item 4): at each call of `Hmi.Display`, a page
//! value located at an input a driver fills is stale while that driver's link exchanges no data,
//! with the link's time since its last data as its age. A value the program computes is not marked;
//! a driver that declares no bytes marks nothing.

use std::ops::Range;

use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use crate::hmi::member_paths::{MemberPath, PathSegment};
use crate::io::{IoAddress, IoSize};
use crate::memory::IoArea;

use super::installed::Runtime;
use super::link_subsystem::io_reading;

/// When each `io.toml` link last exchanged data, or else when the display first saw it.
#[derive(Debug, Default)]
pub(super) struct LinkWatch {
    last_data_ms: FxHashMap<SmolStr, i64>,
}

/// A variable located at the inputs of a link without data: its path in lower case, and the link's
/// time since its last data.
#[derive(Debug, Clone)]
pub(crate) struct SilentInput {
    pub(crate) path: String,
    pub(crate) age_ms: u64,
}

impl Runtime {
    /// The variables at the inputs of the links that exchange no data now.
    pub(super) fn silent_inputs(&mut self) -> Vec<SilentInput> {
        let now_ms = self.current_time().as_millis();
        let mut silent: Vec<(Vec<Range<usize>>, u64)> = Vec::new();
        for io in self.io.link_ios() {
            let last = self
                .hmi
                .links
                .last_data_ms
                .entry(io.link.clone())
                .or_insert(now_ms);
            if io_reading(&io).state.exchanges() {
                *last = now_ms;
            } else if !io.input_bytes.is_empty() {
                let age = u64::try_from(now_ms.saturating_sub(*last)).unwrap_or(0);
                silent.push((io.input_bytes, age));
            }
        }
        if silent.is_empty() {
            return Vec::new();
        }
        self.io()
            .bindings()
            .iter()
            .filter(|binding| binding.address.area == IoArea::Input)
            .filter_map(|binding| {
                let name = binding.display_name.as_ref()?;
                let bytes = address_bytes(&binding.address);
                let age = silent
                    .iter()
                    .filter(|(ranges, _)| {
                        ranges
                            .iter()
                            .any(|range| range.start < bytes.end && bytes.start < range.end)
                    })
                    .map(|(_, age)| *age)
                    .max()?;
                Some(SilentInput {
                    path: name.to_ascii_lowercase(),
                    age_ms: age,
                })
            })
            .collect()
    }
}

/// The bytes of the input image an address takes.
fn address_bytes(address: &IoAddress) -> Range<usize> {
    let start = usize::try_from(address.byte).unwrap_or(usize::MAX);
    let width = match address.size {
        IoSize::Bit | IoSize::Byte => 1,
        IoSize::Word => 2,
        IoSize::DWord => 4,
        IoSize::LWord => 8,
        IoSize::Bytes(count) => usize::try_from(count).unwrap_or(usize::MAX),
    };
    start..start.saturating_add(width)
}

/// The age of a path's value when it is, or lies within, a variable at a silent link's inputs.
pub(crate) fn silent_age(path: &MemberPath, silent: &[SilentInput]) -> Option<u64> {
    if silent.is_empty() {
        return None;
    }
    let text = path_text(path);
    silent
        .iter()
        .filter(|input| {
            text == input.path
                || text
                    .strip_prefix(input.path.as_str())
                    .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
        })
        .map(|input| input.age_ms)
        .max()
}

/// A path as text in lower case: `main.level`, `main.levels[2]`.
fn path_text(path: &MemberPath) -> String {
    let mut text = path.root.to_ascii_lowercase();
    for segment in &path.segments {
        match segment {
            PathSegment::Field(name) => {
                text.push('.');
                text.push_str(&name.to_ascii_lowercase());
            }
            PathSegment::Index(indices) => {
                let joined: Vec<String> = indices.iter().map(i64::to_string).collect();
                text.push('[');
                text.push_str(&joined.join(","));
                text.push(']');
            }
        }
    }
    text
}
