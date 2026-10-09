//! Loading and validating page files (specification 39, §15.2).

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use toml::Spanned;

use super::arrange;
use super::colors::{is_hex_colour, COLOR_TOKENS};
use super::control_keys;
use super::page_list;
use super::pipes::{self, RawPipe};

use super::types::{
    HmiBinding, HmiBindingDirection, HmiClick, HmiColors, HmiNavigation, HmiNavigationPosition,
    HmiNavigationStyle, HmiObject, HmiPage, HmiScalar, HmiScreen, HmiSymbolStyle, HmiTheme,
    HmiTopBar,
};

/// The page-file format this truST reads.
pub const SUPPORTED_FORMAT: i64 = 1;

/// A problem in a page file, with the file and line it is on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PageFileError {
    /// Project-relative file path.
    pub file: String,
    /// One-based line number.
    pub line: usize,
    /// Plain description of the problem.
    pub message: String,
}

impl fmt::Display for PageFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

/// Parses and validates one page file.
///
/// The format is checked first, so a file of another format reports only that. Every other
/// problem is reported with its line; nothing is partially loaded.
pub fn parse_page(file: &str, text: &str) -> Result<HmiPage, Vec<PageFileError>> {
    check_format(file, text)?;
    let raw: RawPage =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    check_objects(file, text, &raw.objects)?;
    pipes::check_pipes(file, text, &raw.pipes)?;
    super::attachments::check_attachments(file, text, &raw.objects, raw.pipes.len())?;
    page_list::check(
        file,
        text,
        raw.order.as_ref(),
        raw.icon.as_ref(),
        raw.group.as_ref(),
    )?;
    page_list::check_pid(file, text, raw.pid.as_ref())?;
    let page = raw.into_page();
    super::regions::check(&page).map_err(|message| vec![error_at(file, 1, message)])?;
    Ok(page)
}

/// Every check of a file's objects, shared by pages and popups.
pub(super) fn check_objects(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    check_unique_ids(file, text, objects)?;
    check_display_ranges(file, text, objects)?;
    check_image_paths(file, text, objects)?;
    check_spans(file, text, objects)?;
    arrange::check(file, text, objects)?;
    control_keys::check(file, text, objects)?;
    super::inline_faceplates::check(file, text, objects)
}

/// Parses and validates the screen layout file.
pub fn parse_screen(file: &str, text: &str) -> Result<HmiScreen, Vec<PageFileError>> {
    check_format(file, text)?;
    let raw: RawScreen =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    let size = *raw.size.get_ref();
    if size.contains(&0) {
        return Err(vec![error_at(
            file,
            line_at(text, raw.size.span().start),
            "size must be at least 1 × 1 pixels".to_string(),
        )]);
    }
    let navigation = match raw.navigation {
        None => HmiNavigation {
            style: HmiNavigationStyle::Side,
            position: HmiNavigationPosition::Left,
        },
        Some(navigation) => navigation.into_navigation(file, text)?,
    };
    if raw.start_page.as_ref().is_some_and(|name| {
        name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    }) {
        return Err(vec![error_at(
            file,
            1,
            "start_page must name a page file".into(),
        )]);
    }
    Ok(HmiScreen {
        start_page: raw.start_page,
        size,
        theme: raw.theme,
        symbols: raw.symbols,
        languages: check_languages(file, text, raw.languages)?,
        top_bar: raw.top_bar.map(|top_bar| HmiTopBar {
            title: top_bar.title,
            logo: top_bar.logo,
            clock: top_bar.clock,
            alarms: top_bar.alarms,
            user: top_bar.user,
            page_title: top_bar.page_title,
        }),
        navigation,
        colors: check_colors(file, text, raw.colors)?,
    })
}

/// The screen's languages (§14.5): language tags, each once, at least one; English without the
/// key.
fn check_languages(
    file: &str,
    text: &str,
    languages: Option<Spanned<Vec<String>>>,
) -> Result<Vec<String>, Vec<PageFileError>> {
    let Some(languages) = languages else {
        return Ok(vec!["en".to_string()]);
    };
    let line = line_at(text, languages.span().start);
    let list = languages.into_inner();
    if let Some(bad) = list.iter().find(|tag| !super::texts::is_language_tag(tag)) {
        return Err(vec![error_at(file, line, super::texts::not_a_tag(bad))]);
    }
    if list.is_empty() {
        return Err(vec![error_at(
            file,
            line,
            "`languages` lists at least one language".to_string(),
        )]);
    }
    if let Some(twice) = list
        .iter()
        .enumerate()
        .find(|(index, tag)| list[..*index].contains(tag))
        .map(|(_, tag)| tag)
    {
        return Err(vec![error_at(
            file,
            line,
            format!("`{twice}` is listed twice in `languages`"),
        )]);
    }
    Ok(list)
}

pub(super) fn check_format(file: &str, text: &str) -> Result<(), Vec<PageFileError>> {
    #[derive(Deserialize)]
    struct FormatOnly {
        format: Option<Spanned<i64>>,
    }

    let probe: FormatOnly =
        toml::from_str(text).map_err(|error| vec![toml_error(file, text, &error)])?;
    match probe.format {
        None => Err(vec![error_at(
            file,
            1,
            "the file has no `format`; add `format = 1` at the top".to_string(),
        )]),
        Some(format) if *format.get_ref() == SUPPORTED_FORMAT => Ok(()),
        Some(format) => Err(vec![error_at(
            file,
            line_at(text, format.span().start),
            format!(
                "format {} is not supported; this truST reads format {SUPPORTED_FORMAT}",
                format.get_ref()
            ),
        )]),
    }
}

/// An image is a project path under `hmi/assets/`: relative, with `/` separators and no `.` or
/// `..` segment, so a page can never reach outside the project's assets.
fn check_image_paths(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    let errors: Vec<PageFileError> = objects
        .iter()
        .filter_map(|object| object.image.as_ref())
        .filter(|image| !is_asset_path(image.get_ref()))
        .map(|image| {
            error_at(
                file,
                line_at(text, image.span().start),
                "`image` must be a project path under hmi/assets/, such as hmi/assets/logo.svg"
                    .to_string(),
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// A trend's span is at least one second.
fn check_spans(file: &str, text: &str, objects: &[RawObject]) -> Result<(), Vec<PageFileError>> {
    let errors: Vec<PageFileError> = objects
        .iter()
        .filter_map(|object| object.span.as_ref())
        .filter(|span| *span.get_ref() == 0)
        .map(|span| {
            error_at(
                file,
                line_at(text, span.span().start),
                "`span` must be at least 1 second".to_string(),
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn is_asset_path(path: &str) -> bool {
    path.strip_prefix("hmi/assets/").is_some_and(|rest| {
        !rest.is_empty()
            && !path.contains('\\')
            && rest
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
    })
}

/// A display range is a finite number: `nan` and `inf` cannot be shown or sent to the editor.
fn check_display_ranges(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    let errors: Vec<PageFileError> = objects
        .iter()
        .flat_map(|object| [("min", &object.min), ("max", &object.max)])
        .filter_map(|(key, value)| {
            let value = value.as_ref()?;
            (!value.get_ref().is_finite()).then(|| {
                error_at(
                    file,
                    line_at(text, value.span().start),
                    format!("{key} must be a finite number"),
                )
            })
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_unique_ids(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    let mut seen = HashSet::new();
    let errors: Vec<PageFileError> = objects
        .iter()
        .filter(|object| !seen.insert(object.id.get_ref().as_str()))
        .map(|object| {
            error_at(
                file,
                line_at(text, object.id.span().start),
                format!(
                    "object id \"{}\" is used twice on this page",
                    object.id.get_ref()
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

pub(super) fn toml_error(file: &str, text: &str, error: &toml::de::Error) -> PageFileError {
    let line = error.span().map_or(1, |span| line_at(text, span.start));
    error_at(file, line, error.message().to_string())
}

pub(crate) fn error_at(file: &str, line: usize, message: String) -> PageFileError {
    PageFileError {
        file: file.to_string(),
        line,
        message,
    }
}

pub(crate) fn line_at(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    #[serde(default)]
    regions: Vec<super::regions::HmiRegion>,
    #[allow(dead_code)]
    format: i64,
    title: String,
    #[serde(default)]
    order: Option<Spanned<i64>>,
    #[serde(default)]
    icon: Option<Spanned<String>>,
    #[serde(default)]
    group: Option<Spanned<String>>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    incomplete: bool,
    #[serde(default)]
    pid: Option<Spanned<String>>,
    #[serde(default, rename = "object")]
    objects: Vec<RawObject>,
    #[serde(default, rename = "pipe")]
    pipes: Vec<RawPipe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawObject {
    pub(super) id: Spanned<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(rename = "type")]
    pub(super) kind: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    pub(super) block: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) faceplate: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) instance: Option<Spanned<String>>,
    at: [i64; 2],
    size: [u32; 2],
    #[serde(default)]
    rotation: u16,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    image: Option<Spanned<String>>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    decimals: Option<u8>,
    #[serde(default)]
    min: Option<Spanned<f64>>,
    #[serde(default)]
    max: Option<Spanned<f64>>,
    #[serde(default)]
    limits: Option<super::limits::HmiLimits>,
    #[serde(default)]
    span: Option<Spanned<u32>>,
    #[serde(default)]
    pub(super) role: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) confirm: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) command: Option<crate::hmi::commands::capability::MethodCommand>,
    #[serde(default)]
    pub(super) action: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) options: Option<Spanned<Vec<Spanned<String>>>>,
    #[serde(default)]
    pub(super) pulse: Option<Spanned<f64>>,
    #[serde(default)]
    pub(super) expect: Option<HmiScalar>,
    #[serde(default)]
    pub(super) timeout: Option<Spanned<f64>>,
    #[serde(default)]
    pub(super) attach: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) measures: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) acts_on: Option<Spanned<String>>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    locked: Option<bool>,
    #[serde(default)]
    pub(super) arrange: Option<Spanned<String>>,
    #[serde(default)]
    pub(super) gap: Option<Spanned<u32>>,
    #[serde(default)]
    bind: BTreeMap<String, RawBinding>,
    #[serde(default)]
    click: Option<RawClick>,
}

impl RawPage {
    fn into_page(self) -> HmiPage {
        HmiPage {
            regions: self.regions,
            title: self.title,
            order: self
                .order
                .and_then(|order| u32::try_from(order.into_inner()).ok()),
            icon: self.icon.map(Spanned::into_inner),
            group: self.group.map(Spanned::into_inner),
            hidden: self.hidden.unwrap_or(false),
            incomplete: self.incomplete,
            pid: self.pid.map(Spanned::into_inner),
            objects: self
                .objects
                .into_iter()
                .map(RawObject::into_object)
                .collect(),
            pipes: self.pipes.into_iter().map(RawPipe::into_pipe).collect(),
        }
    }
}

impl RawObject {
    pub(super) fn into_object(self) -> HmiObject {
        HmiObject {
            id: self.id.into_inner(),
            tag: self.tag,
            kind: self.kind,
            variant: self.variant,
            block: self.block.map(Spanned::into_inner),
            faceplate: self.faceplate.map(Spanned::into_inner),
            instance: self.instance.map(Spanned::into_inner),
            at: self.at,
            size: self.size,
            rotation: self.rotation,
            label: self.label,
            text: self.text,
            image: self.image.map(Spanned::into_inner),
            unit: self.unit,
            decimals: self.decimals,
            min: self.min.map(Spanned::into_inner),
            max: self.max.map(Spanned::into_inner),
            limits: self.limits,
            span: self.span.map(Spanned::into_inner),
            role: self
                .role
                .as_ref()
                .and_then(|role| control_keys::role(role.get_ref())),
            confirm: self.confirm.map(Spanned::into_inner),
            command: self.command,
            action: self
                .action
                .as_ref()
                .and_then(|action| control_keys::action(action.get_ref())),
            options: self.options.map(|options| {
                options
                    .into_inner()
                    .into_iter()
                    .map(Spanned::into_inner)
                    .collect()
            }),
            pulse: self.pulse.map(Spanned::into_inner),
            expect: self.expect,
            timeout: self.timeout.map(Spanned::into_inner),
            attach: self.attach.map(Spanned::into_inner),
            measures: self.measures.map(Spanned::into_inner),
            acts_on: self.acts_on.map(Spanned::into_inner),
            group: self.group,
            locked: self.locked.unwrap_or(false),
            arrange: self
                .arrange
                .as_ref()
                .and_then(|arrange| arrange::arrange(arrange.get_ref())),
            gap: self.gap.map(Spanned::into_inner),
            bind: self
                .bind
                .into_iter()
                .map(|(name, binding)| (name, binding.0))
                .collect(),
            click: self.click.map(|click| click.0),
        }
    }
}

/// A binding written either as a path string or as a table with `path`.
pub(super) struct RawBinding(pub(super) HmiBinding);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingTable {
    #[serde(default)]
    quality: Option<String>,
    path: String,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    range: Option<[f64; 2]>,
    #[serde(default)]
    direction: Option<HmiBindingDirection>,
}

impl<'de> Deserialize<'de> for RawBinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BindingVisitor;

        impl<'de> Visitor<'de> for BindingVisitor {
            type Value = RawBinding;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a variable path, or a table with `path`")
            }

            fn visit_str<E: de::Error>(self, path: &str) -> Result<RawBinding, E> {
                Ok(RawBinding(HmiBinding {
                    quality: None,
                    path: path.to_string(),
                    unit: None,
                    range: None,
                    direction: HmiBindingDirection::Read,
                }))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<RawBinding, A::Error> {
                let table = BindingTable::deserialize(de::value::MapAccessDeserializer::new(map))?;
                if let Some(quality) = &table.quality {
                    crate::hmi::member_paths::MemberPath::parse(
                        quality.strip_prefix('$').unwrap_or(quality),
                    )
                    .map_err(de::Error::custom)?;
                }
                if table
                    .range
                    .is_some_and(|[low, high]| !low.is_finite() || !high.is_finite() || high <= low)
                {
                    return Err(de::Error::custom(
                        "range must have finite, increasing bounds",
                    ));
                }
                Ok(RawBinding(HmiBinding {
                    quality: table.quality,
                    path: table.path,
                    unit: table.unit,
                    range: table.range,
                    direction: table.direction.unwrap_or(HmiBindingDirection::Read),
                }))
            }
        }

        deserializer.deserialize_any(BindingVisitor)
    }
}

/// A click table with exactly one target.
struct RawClick(HmiClick);

impl<'de> Deserialize<'de> for RawClick {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        HmiClick::deserialize(deserializer).map(RawClick)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScreen {
    #[serde(default)]
    start_page: Option<String>,
    #[allow(dead_code)]
    format: i64,
    size: Spanned<[u32; 2]>,
    theme: HmiTheme,
    symbols: HmiSymbolStyle,
    #[serde(default)]
    languages: Option<Spanned<Vec<String>>>,
    #[serde(default)]
    top_bar: Option<RawTopBar>,
    #[serde(default)]
    navigation: Option<RawNavigation>,
    #[serde(default)]
    colors: Option<RawColors>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColors {
    #[serde(default)]
    dark: BTreeMap<String, Spanned<String>>,
    #[serde(default)]
    light: BTreeMap<String, Spanned<String>>,
}

/// Project colours name known tokens and hold hex colours (§14.1); every problem is reported.
fn check_colors(
    file: &str,
    text: &str,
    raw: Option<RawColors>,
) -> Result<HmiColors, Vec<PageFileError>> {
    let Some(raw) = raw else {
        return Ok(HmiColors::default());
    };
    let mut errors = Vec::new();
    let mut check = |entries: BTreeMap<String, Spanned<String>>| {
        let mut colors = BTreeMap::new();
        for (name, value) in entries {
            let line = line_at(text, value.span().start);
            if !COLOR_TOKENS.contains(&name.as_str()) {
                errors.push(error_at(
                    file,
                    line,
                    format!(
                        "`{name}` is not a colour token; the tokens are {}",
                        COLOR_TOKENS.join(", ")
                    ),
                ));
            } else if !is_hex_colour(value.get_ref()) {
                errors.push(error_at(
                    file,
                    line,
                    format!("`{name}` must be a hex colour such as #22d3ee"),
                ));
            } else {
                colors.insert(name, value.into_inner());
            }
        }
        colors
    };
    let colors = HmiColors {
        dark: check(raw.dark),
        light: check(raw.light),
    };
    if errors.is_empty() {
        Ok(colors)
    } else {
        Err(errors)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTopBar {
    title: String,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    clock: bool,
    #[serde(default)]
    alarms: bool,
    #[serde(default)]
    user: bool,
    #[serde(default)]
    page_title: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNavigation {
    style: HmiNavigationStyle,
    #[serde(default)]
    position: Option<Spanned<HmiNavigationPosition>>,
}

impl RawNavigation {
    fn into_navigation(self, file: &str, text: &str) -> Result<HmiNavigation, Vec<PageFileError>> {
        match (self.style, self.position) {
            (HmiNavigationStyle::Side, position) => Ok(HmiNavigation {
                style: HmiNavigationStyle::Side,
                position: position.map_or(HmiNavigationPosition::Left, Spanned::into_inner),
            }),
            (style, None) => Ok(HmiNavigation {
                style,
                position: HmiNavigationPosition::Left,
            }),
            (_, Some(position)) => Err(vec![error_at(
                file,
                line_at(text, position.span().start),
                "`position` applies to the side bar only; remove it or use `style = \"side\"`"
                    .to_string(),
            )]),
        }
    }
}
