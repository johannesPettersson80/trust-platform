//! The palette's equipment, from the component definitions of the project and its libraries
//! (specification 39, §8.5, §21.3).
use super::pairs::{parse_pair, HmiDependency, HmiSetting, HmiTerminal, Pair as HmiPair};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use trust_runtime::hmi::{
    bundle::HmiBundle,
    catalogue::Catalogue,
    page_files::{HmiCallPosition, PageFileError},
};

/// One equipment definition as the editor uses it: the palette entry, the symbol's bindings, the
/// inspector's settings and hardware terminals, and what Create PLC code needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EquipmentDefinition {
    /// Qualified PLC FB type.
    pub block: String,
    /// Customer-facing component title.
    pub title: String,
    /// Reusable symbol kind.
    pub symbol: String,
    /// Reusable equipment panel.
    pub panel: String,
    /// Allowed visual variants of the symbol.
    pub variants: Vec<String>,
    /// Explicit relative member mappings; compiler metadata validates implicit defaults.
    pub bindings: BTreeMap<String, String>,
    /// Relative quality member per bound symbol property.
    pub quality: BTreeMap<String, String>,
    /// Relative PLC members used by the panel fields.
    pub fields: BTreeMap<String, String>,
    /// The settings' metadata, in dialog order.
    pub settings: Vec<HmiSetting>,
    /// The hardware terminals.
    pub io: Vec<HmiTerminal>,
    /// The instances it depends on.
    pub dependencies: Vec<HmiDependency>,
    /// Where Create PLC code puts its call.
    pub position: Option<HmiCallPosition>,
}

/// Whether a name is a bounded dotted IEC identifier, never an expression.
pub(crate) fn qualified_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 256
        && text.split('.').all(|part| {
            let mut bytes = part.bytes();
            bytes
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
                && bytes.all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
        })
}

fn definition(pair: HmiPair) -> EquipmentDefinition {
    let title = pair.title.unwrap_or_else(|| {
        pair.block
            .rsplit('.')
            .next()
            .unwrap_or(&pair.block)
            .trim_start_matches("FB_")
            .to_string()
    });
    EquipmentDefinition {
        block: pair.block,
        title,
        symbol: pair.symbol,
        panel: pair.faceplate,
        variants: pair.variants,
        bindings: pair.bind,
        quality: pair.quality,
        fields: pair.fields,
        settings: pair.settings,
        io: pair.io,
        dependencies: pair.dependencies,
        position: pair.program.and_then(|program| program.position),
    }
}

/// Uses the editor snapshot including unsaved library metadata. Invalid definitions are
/// diagnostics, not silently replaced with a hardcoded equipment implementation.
pub(crate) fn equipment_definitions(
    bundle: &HmiBundle,
    catalogue: &Catalogue,
) -> (Vec<EquipmentDefinition>, Vec<PageFileError>) {
    let mut entries = BTreeMap::new();
    let mut errors = Vec::new();
    for (path, content) in &bundle.files {
        if !path.starts_with("hmi/pairs/") || !path.ends_with(".toml") {
            continue;
        }
        match parse_pair(path, &String::from_utf8_lossy(content)) {
            Err(found) => errors.extend(found),
            Ok(mut pair) => {
                let panel = match super::bundle::faceplate(bundle, &pair.faceplate) {
                    Some(Ok(panel)) => panel,
                    Some(Err(found)) => {
                        errors.extend(found);
                        continue;
                    }
                    None => {
                        errors.push(PageFileError {
                            file: path.clone(),
                            line: 1,
                            message: format!("Equipment panel {} is missing", pair.faceplate),
                        });
                        continue;
                    }
                };
                {
                    if let Some(symbol) = catalogue.component(&pair.symbol) {
                        for property in symbol.properties.keys() {
                            let member = property
                                .split('-')
                                .map(|word| {
                                    let mut chars = word.chars();
                                    chars
                                        .next()
                                        .map(|first| {
                                            first.to_uppercase().collect::<String>()
                                                + chars.as_str()
                                        })
                                        .unwrap_or_default()
                                })
                                .collect::<String>();
                            if panel.fields.contains_key(&member) {
                                let path = pair.fields.get(&member).cloned().unwrap_or(member);
                                pair.bind.entry(property.clone()).or_insert(path);
                            }
                        }
                    }
                }
                if !qualified_name(&pair.block) || catalogue.component(&pair.symbol).is_none() {
                    errors.push(PageFileError {
                        file: path.clone(),
                        line: 1,
                        message: "Component needs a valid FB type and a known drawing symbol"
                            .into(),
                    });
                    continue;
                }
                let key = pair.block.to_ascii_lowercase();
                if entries.contains_key(&key) {
                    errors.push(PageFileError {
                        file: path.clone(),
                        line: 1,
                        message: format!("More than one component defines {}", pair.block),
                    });
                    continue;
                }
                entries.insert(key, definition(pair));
            }
        }
    }
    (entries.into_values().collect(), errors)
}

#[cfg(test)]
mod tests;
