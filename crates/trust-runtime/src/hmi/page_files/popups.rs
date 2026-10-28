//! Popup files (specification 39, §4.9): designed like pages, with their own size and an
//! optional pin.

use serde::Deserialize;
use toml::Spanned;

use super::load::{
    check_format, check_objects, error_at, line_at, toml_error, PageFileError, RawObject,
};
use super::pipes::{check_pipes, RawPipe};
use super::types::{HmiPage, HmiPopup};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPopup {
    #[allow(dead_code)]
    format: i64,
    title: String,
    size: Spanned<[u32; 2]>,
    #[serde(default)]
    pinnable: Option<bool>,
    #[serde(default, rename = "object")]
    objects: Vec<RawObject>,
    #[serde(default, rename = "pipe")]
    pipes: Vec<RawPipe>,
}

/// Parses and validates one popup file; its objects are checked as a page's are.
pub fn parse_popup(file: &str, text: &str) -> Result<HmiPopup, Vec<PageFileError>> {
    check_format(file, text)?;
    let raw: RawPopup =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    popup_from(
        file,
        text,
        raw.title,
        raw.size,
        raw.pinnable,
        raw.objects,
        raw.pipes,
    )
}

/// A popup from its parts, checked: its size, then its objects and pipes as a page's (§4.9).
/// Faceplates are popups too (§8.7).
pub(super) fn popup_from(
    file: &str,
    text: &str,
    title: String,
    size: Spanned<[u32; 2]>,
    pinnable: Option<bool>,
    objects: Vec<RawObject>,
    pipes: Vec<RawPipe>,
) -> Result<HmiPopup, Vec<PageFileError>> {
    let design = *size.get_ref();
    if design.contains(&0) {
        return Err(vec![error_at(
            file,
            line_at(text, size.span().start),
            "size must be at least 1 × 1 pixels".to_string(),
        )]);
    }
    check_objects(file, text, &objects)?;
    check_pipes(file, text, &pipes)?;
    super::attachments::check_attachments(file, text, &objects, pipes.len())?;
    Ok(HmiPopup {
        page: HmiPage {
            regions: Vec::new(),
            title,
            order: None,
            icon: None,
            group: None,
            hidden: false,
            incomplete: false,
            pid: None,
            objects: objects.into_iter().map(RawObject::into_object).collect(),
            pipes: pipes.into_iter().map(RawPipe::into_pipe).collect(),
        },
        size: design,
        pinnable: pinnable.unwrap_or(false),
    })
}
