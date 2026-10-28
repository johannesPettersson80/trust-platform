//! The HMI editor's quality warnings (specification 39, §14.1 items 3 and 4, §14.5 item 6, §18 phase 5): a touch target smaller
//! than 44 × 44 px, a REAL value shown without its unit, an object bound to no variable, a text the drawing cut to fit, a project
//! colour that gives a text too little contrast or that looks like the warning's or an alarm's colour, and a program that does not
//! serve its screens. They never stop a deployment.
//!
//! The editor's drawing reports what only the drawing knows: each text it cut to fit, in each of the project's languages, and what
//! each text of the component specimen is drawn on. Every rule, threshold and sentence is here; the editor shows the warnings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::catalogue::{Catalogue, ComponentCategory, VISIBLE};

use super::hmi_authoring::declarations::Declarations;
use super::hmi_authoring::design_variables;
use super::hmi_files::{refused, single_argument};
use super::{project_inputs, HmiBindingsCommandArgs, HMI_QUALITY_COMMAND};
use crate::handlers::context::ServerContext;

/// The least width and height of a touch target (§3.5 item 2).
const TOUCH: u32 = 44;
/// The components that show a number, which needs its unit.
const NUMBERS: [&str; 7] = [
    "value",
    "gauge",
    "bar",
    "split-bar",
    "slider",
    "numeric-input",
    "setpoint",
];
/// The warning and alarm colours, which data colours must not resemble (§14.1 item 3).
const ALARM_TOKENS: [&str; 5] = [
    "warning",
    "alarm",
    "alarm-high",
    "alarm-medium",
    "alarm-low",
];
/// Closer than this in OKLab, two colours look alike.
const ALIKE: f64 = 0.1;
/// The block a program calls in a task so that the screens get their values (§18 phase 5), as the variable tree names its type.
const DISPLAY_BLOCK: &str = "hmi.display";
/// The sentence of a program without that block.
const NO_DISPLAY: &str =
    "No program calls Hmi.Display: call it in a task to show live values on the screens.";
/// The themes, in the order their warnings are given.
const THEMES: [&str; 2] = ["dark", "light"];

/// `trust-lsp.hmiQuality`: `{ root_uri, open?, page?, colours? }` to `{ ok, objects, colours, display? }`. `page` gives the warnings
/// about a page's objects (`objects`), `colours` those about the project's colours (`colours`) and whether a program serves the
/// screens (`display`). The program's variables are read from the edited declarations, as the variable picker reads them.
pub(super) fn quality_value<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    let args: QualityArgs = match single_argument(args, HMI_QUALITY_COMMAND) {
        Ok(args) => args,
        Err(message) => return refused(message),
    };
    let selection = HmiBindingsCommandArgs {
        open: args.open.clone(),
        root_uri: Some(args.root_uri.clone()),
        text_document: None,
    };
    // A program that does not compile gives no variables: the unit checks then pass, and no display warning is given.
    let variables = project_inputs(context, &selection, HMI_QUALITY_COMMAND)
        .ok()
        .and_then(|inputs| {
            let declarations = Declarations::read(&inputs.sources);
            design_variables::json(&inputs, declarations.as_ref()).ok()
        })
        .and_then(|tree| tree.as_array().cloned())
        .unwrap_or_default();
    let objects = args
        .page
        .as_ref()
        .map(|page| object_warnings(page, Catalogue::shipped(), &variables))
        .unwrap_or_default();
    let (colours, display) = match &args.colours {
        Some(colours) => (colour_warnings(colours), display_warning(&variables)),
        None => (Vec::new(), None),
    };
    json!({ "ok": true, "objects": objects, "colours": colours, "display": display })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QualityArgs {
    root_uri: Url,
    /// Unsaved files, read in memory only, keyed by project-relative path.
    #[serde(default)]
    open: BTreeMap<String, String>,
    #[serde(default)]
    page: Option<PageQuality>,
    #[serde(default)]
    colours: Option<ColourQuality>,
}

/// One page's objects, as its file's model has them, and the texts the drawing cut to fit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PageQuality {
    pub(super) objects: Vec<QualityObject>,
    /// A P&ID is a drawing that never runs: only its cut texts are warned of (§18 phase 4 item 1).
    #[serde(default)]
    pub(super) pid: bool,
    /// The texts the drawing cut, once for each of the project's languages, or once as written when the project has none
    /// (§14.5 item 6).
    #[serde(default)]
    pub(super) cuts: Vec<LanguageCuts>,
}

/// The fields of a page object that the checks read; the model's other fields are ignored.
#[derive(Deserialize)]
pub(super) struct QualityObject {
    pub(super) id: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) size: [u32; 2],
    #[serde(default)]
    pub(super) tag: Option<String>,
    #[serde(default)]
    pub(super) instance: Option<String>,
    #[serde(default)]
    pub(super) block: Option<String>,
    #[serde(default)]
    pub(super) label: Option<String>,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) unit: Option<String>,
    #[serde(default)]
    pub(super) bind: BTreeMap<String, QualityBinding>,
    #[serde(default)]
    pub(super) click: Option<Value>,
}

/// The fields of a binding that the checks read.
#[derive(Deserialize)]
pub(super) struct QualityBinding {
    pub(super) path: String,
    #[serde(default)]
    pub(super) unit: Option<String>,
}

/// The texts the drawing cut while it drew the page in one language.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LanguageCuts {
    /// The language, with its text table; none for a project without languages.
    #[serde(default)]
    pub(super) language: Option<LanguageTable>,
    pub(super) cuts: Vec<CutText>,
}

/// A project language with its text table: the project's texts over truST's words (§14.5). Its other fields, such as its tag, are
/// not read.
#[derive(Deserialize)]
pub(super) struct LanguageTable {
    pub(super) name: String,
    #[serde(default)]
    pub(super) texts: BTreeMap<String, String>,
}

/// A text the drawing cut to fit, before it was cut, and the object it belongs to.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CutText {
    pub(super) object: String,
    pub(super) text: String,
}

/// The project's colours and the drawing's own, and each text of the component specimen with what lies behind it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ColourQuality {
    /// The drawing's colours of each theme (its design tokens), which the project's override.
    pub(super) defaults: BTreeMap<String, BTreeMap<String, String>>,
    /// The project's colours of each theme, in the order `[colors.<theme>]` holds them.
    #[serde(default)]
    pub(super) own: BTreeMap<String, Vec<(String, String)>>,
    /// The specimen's texts, the inactive controls of a read-only page left out: they need no contrast (WCAG 2.1, 1.4.3).
    pub(super) texts: Vec<DrawnText>,
}

/// A text of the drawing with what lies behind it. Its other fields, such as the object it belongs to, are not read.
#[derive(Deserialize)]
pub(super) struct DrawnText {
    pub(super) text: String,
    /// The text's colour token.
    pub(super) colour: String,
    /// The shapes under the text's middle, topmost first; a gradient gives each of its stops.
    pub(super) behind: Vec<Vec<Backing>>,
    pub(super) size: f64,
    pub(super) bold: bool,
    /// A text of symbols only, such as "?" or "!": a graphic, not words or numbers.
    pub(super) sign: bool,
}

/// One colour behind a text: a token with the opacity it is drawn at, or part of the way toward a second token.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Backing {
    pub(super) token: String,
    pub(super) opacity: f64,
    #[serde(default)]
    pub(super) mix: Option<Mix>,
}

/// Part of the way toward a second token, as a gradient between two stops.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Mix {
    pub(super) token: String,
    pub(super) share: f64,
}

/// One quality warning: about an object, or about a project colour at its theme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct QualityWarning {
    pub(super) kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) object: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) colour: Option<ColourName>,
    pub(super) message: String,
}

/// A project colour by its theme and token.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct ColourName {
    pub(super) theme: String,
    pub(super) token: String,
}

/// The warnings about a page's objects, in the page's order, each object's cut texts after its other warnings.
pub(super) fn object_warnings(
    page: &PageQuality,
    catalogue: &Catalogue,
    variables: &[Value],
) -> Vec<QualityWarning> {
    let cuts = cut_warnings(&page.objects, &page.cuts);
    let mut warnings = Vec::new();
    for object in &page.objects {
        if !page.pid {
            warnings.extend(own_warnings(object, catalogue, variables));
        }
        warnings.extend(cuts.get(&object.id).cloned().unwrap_or_default());
    }
    warnings
}

/// An object's touch target, unit and binding warnings.
fn own_warnings(
    object: &QualityObject,
    catalogue: &Catalogue,
    variables: &[Value],
) -> Vec<QualityWarning> {
    let mut warnings = Vec::new();
    let component = catalogue
        .components
        .iter()
        .find(|component| component.kind == object.kind);
    let category = component.map(|component| component.category);
    let [width, height] = object.size;
    let name = name_of(object);
    let warn = |kind: &'static str, message: String| QualityWarning {
        kind,
        object: Some(object.id.clone()),
        colour: None,
        message,
    };
    let touch = category == Some(ComponentCategory::Control)
        || object.kind == "page-button"
        || object.click.is_some();
    if touch && (width < TOUCH || height < TOUCH) {
        warnings.push(warn(
            "touch-target",
            format!(
                "{name} is {width} × {height} px; a touch target needs at least {TOUCH} × {TOUCH} px."
            ),
        ));
    }
    let unit = object.unit.is_some() || object.bind.values().any(|binding| binding.unit.is_some());
    let real = object.bind.values().any(|binding| {
        variable_type(variables, &binding.path).is_some_and(|kind| {
            kind.eq_ignore_ascii_case("REAL") || kind.eq_ignore_ascii_case("LREAL")
        })
    });
    if NUMBERS.contains(&object.kind.as_str()) && !unit && real {
        warnings.push(warn(
            "unit",
            format!(
                "{name} shows a REAL value without a unit. Add its unit, or choose No unit if the number has none."
            ),
        ));
    }
    if let Some(component) = component {
        // The visibility binding every object may have shows no value, so it alone does not make a component take bindings.
        let takes = component.properties.keys().any(|name| name != VISIBLE)
            || component.additional.is_some();
        let shown_text = category == Some(ComponentCategory::Display) && filled(&object.text);
        // A process symbol without bindings is a drawing, as a tank or a hand valve often is; a display or control exists to show
        // or send a value.
        let drawing = category == Some(ComponentCategory::Equipment) && object.instance.is_none();
        if takes && object.bind.is_empty() && !shown_text && !drawing {
            warnings.push(warn(
                "unbound",
                format!("{name} is not bound to a variable, so it shows nothing in the runtime."),
            ));
        }
    }
    warnings
}

/// The warnings about the texts the drawing cut, by object: one per object and language, naming the language when the project has
/// more than one (§14.1 item 4, §14.5 item 6). A cut text is the object's label or text as the language shows it, else one of truST's
/// words.
fn cut_warnings(
    objects: &[QualityObject],
    languages: &[LanguageCuts],
) -> BTreeMap<String, Vec<QualityWarning>> {
    let mut warned: BTreeMap<String, Vec<QualityWarning>> = BTreeMap::new();
    for drawn in languages {
        let place = match &drawn.language {
            Some(language) if languages.len() > 1 => format!(" in {}", language.name),
            _ => String::new(),
        };
        let marker = format!("cut to fit{place}:");
        for cut in &drawn.cuts {
            let known = warned.entry(cut.object.clone()).or_default();
            if known
                .iter()
                .any(|warning| warning.message.contains(&marker))
            {
                continue;
            }
            let object = objects.iter().find(|object| object.id == cut.object);
            // Whether the cut text is these words as the language shows them.
            let cut_from = |words: Option<&String>| {
                words.is_some_and(|words| {
                    let shown = drawn
                        .language
                        .as_ref()
                        .and_then(|language| language.texts.get(words))
                        .unwrap_or(words);
                    *shown == cut.text
                })
            };
            let field = match object {
                Some(object) if cut_from(object.label.as_ref()) => "label",
                Some(object) if cut_from(object.text.as_ref()) => "text",
                _ => "word",
            };
            known.push(QualityWarning {
                kind: "cut-text",
                object: Some(cut.object.clone()),
                colour: None,
                message: format!(
                    "The {field} \"{}\" is cut to fit{place}: make the object wider or the {field} shorter.",
                    cut.text
                ),
            });
        }
    }
    warned
}

/// How a warning names an object: its label or text, else the name it goes by (`object_name`).
fn name_of(object: &QualityObject) -> String {
    [&object.label, &object.text]
        .into_iter()
        .find(|words| filled(words))
        .and_then(Clone::clone)
        .unwrap_or_else(|| object_name(object))
}

/// The name an object goes by (§3.3 item 8): its tag, its instance, its equipment's block in words (`HeatingSequence` is "Heating
/// sequence"), its label or text, else its id.
fn object_name(object: &QualityObject) -> String {
    if let Some(tag) = object.tag.as_ref().filter(|tag| !tag.is_empty()) {
        return tag.clone();
    }
    if let Some(instance) = object
        .instance
        .as_deref()
        .and_then(|instance| instance.rsplit('.').next())
        .filter(|last| !last.is_empty())
    {
        return instance.to_owned();
    }
    if let Some(block) = object
        .block
        .as_deref()
        .and_then(|block| block.rsplit('.').next())
        .map(|last| last.strip_prefix("FB_").unwrap_or(last))
        .filter(|block| !block.is_empty())
    {
        return words_of(block);
    }
    [&object.label, &object.text]
        .into_iter()
        .find(|words| filled(words))
        .and_then(Clone::clone)
        .unwrap_or_else(|| object.id.clone())
}

/// An identifier in words: `HeatingSequence` is "Heating sequence", `PowerOutput` "Power output".
fn words_of(name: &str) -> String {
    let letters: Vec<char> = name.chars().collect();
    let mut words = String::new();
    for (index, &letter) in letters.iter().enumerate() {
        let after_lower = index > 0
            && (letters[index - 1].is_ascii_lowercase() || letters[index - 1].is_ascii_digit());
        if letter.is_ascii_uppercase() && after_lower {
            words.push(' ');
            // A capital that starts a word in lower case after the space, as `PowerOutput` gives "Power output".
            let starts_word = letters.get(index + 1).is_some_and(char::is_ascii_lowercase);
            words.push(if starts_word {
                letter.to_ascii_lowercase()
            } else {
                letter
            });
        } else {
            words.push(letter);
        }
    }
    words
}

/// Whether optional words are present and not empty.
fn filled(words: &Option<String>) -> bool {
    words.as_deref().is_some_and(|words| !words.is_empty())
}

/// The declared type of a variable path in the variable picker's tree (§7.1): a remote `runtime:member` by its member, any other
/// path segment by segment, an index going into the array's element.
pub(super) fn variable_type(variables: &[Value], path: &str) -> Option<String> {
    if path.contains(':') {
        let mut parts = path.split(':');
        let runtime = parts.next().unwrap_or_default();
        let member = parts.next().unwrap_or_default();
        return variables
            .iter()
            .find(|node| {
                let mut parts = node["path"].as_str().unwrap_or_default().split(':');
                parts.next() == Some(runtime)
                    && parts
                        .next()
                        .is_some_and(|local| local.eq_ignore_ascii_case(member))
            })
            .and_then(|node| node["type"].as_str())
            .map(str::to_owned);
    }
    let mut level: &[Value] = variables;
    let mut node: Option<&Value> = None;
    for segment in path.split('.') {
        let (name, indexes) = segment_parts(segment.trim())?;
        let mut found = level.iter().find(|candidate| {
            candidate["name"]
                .as_str()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
        });
        for _ in 0..indexes {
            found = found.and_then(|array| array.get("array")?.get("element"));
        }
        let current = found?;
        node = Some(current);
        level = current["children"]
            .as_array()
            .map_or(&[][..], Vec::as_slice);
    }
    node?["type"].as_str().map(str::to_owned)
}

/// A path segment's name and how many indexes follow it (`Tanks[2]` is `Tanks` and one): a name without brackets, then bracketed
/// indexes only. Any other segment names no variable.
fn segment_parts(segment: &str) -> Option<(&str, usize)> {
    let start = segment.find('[').unwrap_or(segment.len());
    let (name, mut rest) = segment.split_at(start);
    if name.is_empty() || name.contains(']') {
        return None;
    }
    let mut indexes = 0;
    while !rest.is_empty() {
        let inside = rest.strip_prefix('[')?;
        let end = inside.find(']')?;
        indexes += 1 + inside[..end].matches('[').count();
        rest = &inside[end + 1..];
    }
    Some((name, indexes))
}

/// The warning of a program that has no `Hmi.Display` block, so the screens would show no values; none while the program has no
/// variables, as when it does not compile.
pub(super) fn display_warning(variables: &[Value]) -> Option<&'static str> {
    fn found(nodes: &[Value]) -> bool {
        nodes.iter().any(|node| {
            node["type"]
                .as_str()
                .is_some_and(|kind| kind.to_lowercase() == DISPLAY_BLOCK)
                || node["children"]
                    .as_array()
                    .is_some_and(|children| found(children))
        })
    }
    (!variables.is_empty() && !found(variables)).then_some(NO_DISPLAY)
}

/// The warnings about the project's own colours, theme by theme: the worst text each colour gives too little contrast, then each data
/// colour that looks like the warning's or an alarm's.
pub(super) fn colour_warnings(colours: &ColourQuality) -> Vec<QualityWarning> {
    let mut warnings = Vec::new();
    for theme in THEMES {
        let Some(defaults) = colours.defaults.get(theme) else {
            continue;
        };
        let own: Vec<(&str, &str)> = colours
            .own
            .get(theme)
            .into_iter()
            .flatten()
            .filter(|(token, colour)| {
                token != "font" && defaults.contains_key(token) && is_hex(colour)
            })
            .map(|(token, colour)| (token.as_str(), colour.as_str()))
            .collect();
        if own.is_empty() {
            continue;
        }
        let mut palette = defaults.clone();
        for (token, colour) in &own {
            palette.insert((*token).to_owned(), (*colour).to_owned());
        }
        warnings.extend(contrast_warnings(theme, &own, &palette, &colours.texts));
        warnings.extend(look_alike_warnings(theme, &own, &palette));
    }
    warnings
}

/// The worst text each project colour takes part in that has too little contrast, in the order the colours were first found.
fn contrast_warnings(
    theme: &str,
    own: &[(&str, &str)],
    palette: &BTreeMap<String, String>,
    texts: &[DrawnText],
) -> Vec<QualityWarning> {
    let mut worst: Vec<(&str, &DrawnText, f64, f64)> = Vec::new();
    for drawn in texts {
        let (ratio, needed) = text_contrast(drawn, palette);
        if ratio >= needed {
            continue;
        }
        let mut involved: Vec<&str> = vec![drawn.colour.as_str()];
        for backing in drawn.behind.iter().flatten() {
            for token in
                std::iter::once(&backing.token).chain(backing.mix.as_ref().map(|mix| &mix.token))
            {
                if !involved.contains(&token.as_str()) {
                    involved.push(token.as_str());
                }
            }
        }
        for token in involved {
            if !own.iter().any(|(own, _)| *own == token) {
                continue;
            }
            match worst.iter_mut().find(|(known, ..)| *known == token) {
                Some(known) if ratio / needed < known.2 / known.3 => {
                    *known = (token, drawn, ratio, needed)
                }
                Some(_) => {}
                None => worst.push((token, drawn, ratio, needed)),
            }
        }
    }
    worst
        .into_iter()
        .map(|(token, drawn, ratio, needed)| {
            let what = if drawn.sign {
                format!("the sign \"{}\" in \"{}\"", drawn.text, drawn.colour)
            } else {
                format!("text in \"{}\"", drawn.colour)
            };
            let behind = drawn
                .behind
                .first()
                .and_then(|layer| layer.first())
                .map_or("page", |backing| backing.token.as_str());
            QualityWarning {
                kind: "contrast",
                object: None,
                colour: Some(ColourName {
                    theme: theme.to_owned(),
                    token: token.to_owned(),
                }),
                message: format!(
                    "With this colour, {what} on \"{behind}\" has a contrast of {ratio:.2}:1 in the {theme} theme; it needs {needed}:1."
                ),
            }
        })
        .collect()
}

/// Each project colour that is not an alarm's or the stale colour and lies within [`ALIKE`] of the warning's or an alarm's colour.
fn look_alike_warnings(
    theme: &str,
    own: &[(&str, &str)],
    palette: &BTreeMap<String, String>,
) -> Vec<QualityWarning> {
    let mut warnings = Vec::new();
    for (token, colour) in own {
        if ALARM_TOKENS.contains(token) || *token == "stale" {
            continue;
        }
        let nearest = ALARM_TOKENS
            .iter()
            .map(|alarm| {
                let distance = palette
                    .get(*alarm)
                    .map_or(f64::INFINITY, |shade| oklab_distance(colour, shade));
                (*alarm, distance)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((alarm, distance)) = nearest.filter(|(_, distance)| *distance < ALIKE) else {
            continue;
        };
        let named = match alarm {
            "warning" => "warning".to_owned(),
            "alarm" => "alarm".to_owned(),
            other => format!("{}-priority alarm", other.trim_start_matches("alarm-")),
        };
        warnings.push(QualityWarning {
            kind: "look-alike",
            object: None,
            colour: Some(ColourName {
                theme: theme.to_owned(),
                token: (*token).to_owned(),
            }),
            message: format!(
                "{token} looks like the {named} colour in the {theme} theme ({distance:.2} apart), so data could be taken for an alarm."
            ),
        });
    }
    warnings
}

/// Red, green, blue and alpha, each from 0 to 1.
type Rgba = [f64; 4];

/// Whether a project colour is a hex colour: `#rgb`, `#rrggbb` or `#rrggbbaa`.
fn is_hex(colour: &str) -> bool {
    colour.strip_prefix('#').is_some_and(|digits| {
        matches!(digits.len(), 3 | 6 | 8) && digits.chars().all(|digit| digit.is_ascii_hexdigit())
    })
}

/// A hex colour (`#rgb`, `#rrggbb` or `#rrggbbaa`, the `#` optional) as red, green, blue and alpha.
fn parse_hex(hex: &str) -> Option<Rgba> {
    let digits = hex.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    let full: String = if digits.len() == 3 {
        digits.chars().flat_map(|digit| [digit, digit]).collect()
    } else {
        digits.to_owned()
    };
    if !matches!(full.len(), 6 | 8) || !full.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |index: usize| {
        u8::from_str_radix(&full[index..index + 2], 16)
            .map_or(0.0, |value| f64::from(value) / 255.0)
    };
    Some([
        channel(0),
        channel(2),
        channel(4),
        if full.len() == 8 { channel(6) } else { 1.0 },
    ])
}

/// A channel of sRGB in linear light.
fn linear(channel: f64) -> f64 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// The relative luminance of a colour (WCAG 2.1).
fn luminance(colour: Rgba) -> f64 {
    0.2126 * linear(colour[0]) + 0.7152 * linear(colour[1]) + 0.0722 * linear(colour[2])
}

/// The WCAG 2.1 contrast ratio of two opaque colours, from 1 to 21.
fn ratio_of(a: Rgba, b: Rgba) -> f64 {
    let (first, second) = (luminance(a), luminance(b));
    let (light, dark) = if first >= second {
        (first, second)
    } else {
        (second, first)
    };
    (light + 0.05) / (dark + 0.05)
}

/// `top` drawn at `alpha` over `bottom`, which comes out opaque.
fn over(top: Rgba, alpha: f64, bottom: Rgba) -> Rgba {
    [
        top[0] * alpha + bottom[0] * (1.0 - alpha),
        top[1] * alpha + bottom[1] * (1.0 - alpha),
        top[2] * alpha + bottom[2] * (1.0 - alpha),
        1.0,
    ]
}

/// A number rounded to six decimals, as every ratio and distance is compared.
fn rounded(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

/// A text's contrast with a palette at its worst over what lies behind it, and the contrast it needs: 4.5:1, or 3:1 for text of at
/// least 24 px, or 18.66 px and bold (WCAG 2.1, 1.4.3), and 3:1 for a sign, which is a graphic (1.4.11).
pub(super) fn text_contrast(drawn: &DrawnText, palette: &BTreeMap<String, String>) -> (f64, f64) {
    let needed = if drawn.sign || drawn.size >= 24.0 || (drawn.bold && drawn.size >= 18.66) {
        3.0
    } else {
        4.5
    };
    let plain = |token: &str| palette.get(token).and_then(|colour| parse_hex(colour));
    let colour = |token: &str, mix: Option<&Mix>| {
        let from = plain(token)?;
        let Some((to, share)) = mix.and_then(|mix| Some((plain(&mix.token)?, mix.share))) else {
            return Some(from);
        };
        Some([0, 1, 2, 3].map(|index| from[index] + (to[index] - from[index]) * share))
    };
    // The colours behind the text, built up from the bottom; each gradient stop is a possibility.
    let mut below: Vec<Rgba> = vec![[0.0, 0.0, 0.0, 1.0]];
    for layer in drawn.behind.iter().rev() {
        let mut next = Vec::new();
        for base in &below {
            for backing in layer {
                if let Some(paint) = colour(&backing.token, backing.mix.as_ref()) {
                    next.push(over(paint, paint[3] * backing.opacity, *base));
                }
            }
        }
        if !next.is_empty() {
            next.truncate(64);
            below = next;
        }
    }
    let Some(ink) = colour(&drawn.colour, None) else {
        return (1.0, needed);
    };
    let ratio = below
        .iter()
        .map(|background| ratio_of(over(ink, ink[3], *background), *background))
        .fold(f64::INFINITY, f64::min);
    (rounded(ratio), needed)
}

/// A colour in OKLab.
fn oklab(colour: Rgba) -> [f64; 3] {
    let (r, g, b) = (linear(colour[0]), linear(colour[1]), linear(colour[2]));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

/// The distance of two colours in OKLab, where 1 is black to white; infinite when either is no hex colour.
pub(super) fn oklab_distance(a: &str, b: &str) -> f64 {
    let (Some(x), Some(y)) = (parse_hex(a), parse_hex(b)) else {
        return f64::INFINITY;
    };
    let (p, q) = (oklab(x), oklab(y));
    rounded(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt())
}

#[cfg(test)]
#[path = "hmi_quality_tests.rs"]
mod tests;
