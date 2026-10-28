//! New HMI Screen: a screen layout and its first page from a template (specification 39, §4
//! items 1, 8 and 10; row H5).
//!
//! - `templates {}` → `{ templates: [{ id, label, detail }] }`: the templates, for the quick pick.
//! - `create { template, folder }` → `{ screen: { file, text }, page: { file, text } }`: the
//!   template's screen layout, its top bar titled with the project folder's name in words
//!   (`room-heating` is "Room heating", §4 item 8), and the first page, Overview. `folder` is the
//!   name of the project folder, not its path.
//!
//! The extension keeps an existing screen layout and an existing first page, and installs the
//! libraries the screen needs.

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{
    write_screen, HmiColors, HmiNavigation, HmiNavigationPosition, HmiNavigationStyle, HmiScreen,
    HmiSymbolStyle, HmiTheme, HmiTopBar,
};

use super::decode;

/// The screen layout's file in the project.
const SCREEN_FILE: &str = "hmi/screen.toml";

/// The first page's file in the project.
const FIRST_PAGE_FILE: &str = "hmi/pages/overview.toml";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    Templates {},
    Create { template: String, folder: String },
}

/// What a template's top bar shows beside its title (§4 item 8).
struct TopBar {
    clock: bool,
    alarms: bool,
    user: bool,
    page_title: bool,
}

/// A screen layout to start from (§4 item 10).
struct Template {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    size: [u32; 2],
    top_bar: Option<TopBar>,
    navigation: HmiNavigationStyle,
}

/// The templates, in the order the quick pick offers them. Each size fits its panel together with
/// the top bar and the navigation (§4 item 7).
static TEMPLATES: [Template; 3] = [
    Template {
        id: "operator-panel",
        label: "Operator panel",
        detail: "For a 1920 × 1080 panel: top bar with title, clock, alarms and user, side bar on the left",
        size: [1688, 1024],
        top_bar: Some(TopBar {
            clock: true,
            alarms: true,
            user: true,
            page_title: true,
        }),
        navigation: HmiNavigationStyle::Side,
    },
    Template {
        id: "overview",
        label: "Overview",
        detail: "For a 1280 × 800 panel: top bar with title, clock and user, tabs",
        size: [1280, 696],
        top_bar: Some(TopBar {
            clock: true,
            alarms: false,
            user: true,
            page_title: false,
        }),
        navigation: HmiNavigationStyle::Tabs,
    },
    Template {
        id: "single-page",
        label: "Single page",
        detail: "1024 × 768, without top bar or navigation",
        size: [1024, 768],
        top_bar: None,
        navigation: HmiNavigationStyle::None,
    },
];

/// The answer of `trust-lsp.hmiNewScreen`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Templates {} => {
            let templates: Vec<Value> = TEMPLATES
                .iter()
                .map(|template| {
                    json!({ "id": template.id, "label": template.label, "detail": template.detail })
                })
                .collect();
            Ok(json!({ "ok": true, "templates": templates }))
        }
        Args::Create { template, folder } => create(&template, &folder),
    }
}

/// The screen layout a project without one gets with its first page or panel (§4 item 1): the
/// default design size of §3.4, 1920 × 1080, the dark theme, flat symbols and no navigation.
pub(super) fn default_screen() -> String {
    write_screen(&screen([1920, 1080], None, HmiNavigationStyle::None))
}

fn screen(size: [u32; 2], top_bar: Option<HmiTopBar>, navigation: HmiNavigationStyle) -> HmiScreen {
    HmiScreen {
        start_page: None,
        size,
        theme: HmiTheme::Dark,
        symbols: HmiSymbolStyle::Flat,
        languages: vec!["en".to_string()],
        top_bar,
        navigation: HmiNavigation {
            style: navigation,
            position: HmiNavigationPosition::Left,
        },
        colors: HmiColors::default(),
    }
}

fn create(id: &str, folder: &str) -> Result<Value, String> {
    let template = TEMPLATES
        .iter()
        .find(|template| template.id == id)
        .ok_or_else(|| format!("There is no screen template \"{id}\"."))?;
    let title = super::files::in_words(folder);
    let top_bar = template.top_bar.as_ref().map(|bar| HmiTopBar {
        title,
        logo: None,
        clock: bar.clock,
        alarms: bar.alarms,
        user: bar.user,
        page_title: bar.page_title,
    });
    let layout = write_screen(&screen(template.size, top_bar, template.navigation));
    // The first page: first in the navigation, with the overview icon and a heading.
    let page = super::files::written(
        "page",
        json!({
            "title": "Overview",
            "order": 0,
            "icon": "overview",
            "object": [{
                "id": "heading",
                "type": "text",
                "variant": "heading",
                "at": [40, 32],
                "size": [600, 40],
                "rotation": 0,
                "text": "Overview",
                "bind": {},
            }],
            "pipe": [],
        }),
    )?;
    Ok(json!({
        "ok": true,
        "screen": { "file": SCREEN_FILE, "text": layout },
        "page": { "file": FIRST_PAGE_FILE, "text": page },
    }))
}

#[cfg(test)]
#[path = "screens_tests.rs"]
mod tests;
