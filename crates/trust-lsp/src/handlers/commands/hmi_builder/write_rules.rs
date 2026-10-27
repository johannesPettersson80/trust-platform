//! The runtime display's write rules a page's controls need, added to `runtime.toml`
//! (specification 39, §12.2 item 3): **Allow writing** beside a write-policy problem, its quick fix,
//! Create HMI Settings Page and Create PLC code ask for the rules, and the extension writes the
//! answer into the file.
//!
//! - `forPage { file, text, point? }` → `{ root, rules }`: the rules the controls of the page,
//!   popup or panel file `file` (its text `text`) need, one per point they write, or for `point`
//!   only; `root` is the project, the folder that holds the file's `hmi/`.
//! - `add { runtimeToml, rules }` → `{ text, added }`: `runtime.toml` with the rules for the points
//!   it does not name yet appended, and those points.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use toml_edit::{value, Table};
use trust_runtime::hmi::page_files::{
    parse_faceplate, parse_page, parse_popup, HmiBindingDirection, HmiObject, HmiRole,
};

use super::decode;
use super::runtime_toml::{self, Entries};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    ForPage {
        file: String,
        text: String,
        #[serde(default)]
        point: Option<String>,
    },
    Add {
        #[serde(rename = "runtimeToml")]
        runtime_toml: String,
        rules: Vec<WriteRule>,
    },
}

/// A `[[runtime.web.hmi.write]]` rule as the editor adds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WriteRule {
    /// The point.
    pub(super) path: String,
    /// The lowest role that may write it.
    pub(super) role: RuleRole,
    /// The lowest number that may be written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) min: Option<f64>,
    /// The highest number that may be written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) max: Option<f64>,
}

/// The role a write rule names (§12.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum RuleRole {
    Operator,
    Engineer,
}

impl RuleRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Engineer => "engineer",
        }
    }
}

/// The answer of `trust-lsp.hmiWriteRules`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::ForPage { file, text, point } => {
            let (root, kind) = place(&file)
                .ok_or_else(|| "The page is not in a project's hmi folder.".to_owned())?;
            let rules = page_rules(&controls(kind, &file, &text), point.as_deref());
            Ok(json!({ "ok": true, "root": root, "rules": rules }))
        }
        Args::Add {
            runtime_toml,
            rules,
        } => {
            let (text, added) = add(&runtime_toml, &rules)?;
            Ok(json!({ "ok": true, "text": text, "added": added }))
        }
    }
}

/// The project of a page, popup or panel file, the folder that holds its `hmi/`, and the file's
/// kind; none for a file elsewhere.
fn place(file: &str) -> Option<(String, &'static str)> {
    let normalized = file.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    let hmi = parts.iter().rposition(|part| *part == "hmi")?;
    let kind = match *parts.get(hmi + 1)? {
        "pages" => "page",
        "popups" => "popup",
        "panels" => "panel",
        _ => return None,
    };
    (hmi >= 1).then(|| (parts[..hmi].join("/"), kind))
}

/// The objects of a page, popup or panel file; none when it does not load, so a broken file asks
/// for no rule but the one named.
fn controls(kind: &str, file: &str, text: &str) -> Vec<HmiObject> {
    let page = match kind {
        "popup" => parse_popup(file, text).map(|popup| popup.page),
        "panel" => parse_faceplate(file, text).map(|faceplate| faceplate.popup.page),
        _ => parse_page(file, text),
    };
    page.map(|page| page.objects).unwrap_or_default()
}

/// The rule a control needs to write `point`: its role, Engineer when it names none (§12.3), and
/// its limits.
fn rule_for(object: &HmiObject, point: &str) -> WriteRule {
    WriteRule {
        path: point.to_owned(),
        role: match object.role {
            Some(HmiRole::Operator) => RuleRole::Operator,
            Some(HmiRole::Engineer) | None => RuleRole::Engineer,
        },
        min: object.min,
        max: object.max,
    }
}

/// The rules the controls need, one per point they write, in the order they first write it, or
/// for `point` only (an Engineer rule when no control writes it). Several controls writing one
/// point give the lowest role they ask and the widest limits, a limit only when each control has
/// one, so that none of them asks more than the rule allows.
fn page_rules(objects: &[HmiObject], point: Option<&str>) -> Vec<WriteRule> {
    let wanted = point.map(str::to_lowercase);
    let mut by_point: Vec<(String, Vec<WriteRule>)> = Vec::new();
    for object in objects {
        for binding in object.bind.values() {
            let key = binding.path.to_lowercase();
            if binding.direction == HmiBindingDirection::Read
                || wanted.as_ref().is_some_and(|wanted| *wanted != key)
            {
                continue;
            }
            let rule = rule_for(object, &binding.path);
            match by_point.iter_mut().find(|(known, _)| *known == key) {
                Some((_, rules)) => rules.push(rule),
                None => by_point.push((key, vec![rule])),
            }
        }
    }
    if let Some(point) = point
        && by_point.is_empty()
    {
        return vec![WriteRule {
            path: point.to_owned(),
            role: RuleRole::Engineer,
            min: None,
            max: None,
        }];
    }
    by_point
        .into_iter()
        .map(|(_, found)| WriteRule {
            path: found[0].path.clone(),
            role: if found.iter().any(|rule| rule.role == RuleRole::Operator) {
                RuleRole::Operator
            } else {
                RuleRole::Engineer
            },
            min: found
                .iter()
                .map(|rule| rule.min)
                .collect::<Option<Vec<f64>>>()
                .map(|mins| mins.into_iter().fold(f64::INFINITY, f64::min)),
            max: found
                .iter()
                .map(|rule| rule.max)
                .collect::<Option<Vec<f64>>>()
                .map(|maxes| maxes.into_iter().fold(f64::NEG_INFINITY, f64::max)),
        })
        .collect()
}

/// `runtime.toml` with the rules for the points it does not name yet appended, each point once
/// whatever its case, and those points. A point the file names already is left as it is.
fn add(text: &str, rules: &[WriteRule]) -> Result<(String, Vec<String>), String> {
    let document = runtime_toml::parse(text)?;
    let named: Vec<String> = runtime_toml::tables(&document, Entries::WriteRules)?
        .into_iter()
        .filter_map(|table| runtime_toml::text_of(table, "path"))
        .map(str::to_lowercase)
        .collect();
    let mut added: Vec<&WriteRule> = Vec::new();
    for rule in rules {
        let key = rule.path.to_lowercase();
        if !named.contains(&key) && !added.iter().any(|other| other.path.to_lowercase() == key) {
            added.push(rule);
        }
    }
    let tables = added.iter().map(|rule| table_of(rule)).collect();
    let next = runtime_toml::appended(text, Entries::WriteRules, tables)?;
    Ok((
        next,
        added.into_iter().map(|rule| rule.path.clone()).collect(),
    ))
}

/// A rule's table: `path`, `role`, and `min` and `max` when it has them.
fn table_of(rule: &WriteRule) -> Table {
    let mut table = Table::new();
    table.insert("path", value(rule.path.as_str()));
    table.insert("role", value(rule.role.as_str()));
    if let Some(min) = rule.min {
        table.insert("min", value(min));
    }
    if let Some(max) = rule.max {
        table.insert("max", value(max));
    }
    table
}

#[cfg(test)]
#[path = "write_rules_tests.rs"]
mod tests;
