//! My equipment (specification 39, §21.9): the project's own function blocks in the builder's
//! palette with a working definition, before any file is written, and the files the builder
//! writes when one is first placed. A block that already has a definition is not proposed.
//!
//! `trust-lsp.hmiOwnEquipment` takes `{ root_uri, open? }` and gives `{ ok, equipment, mine }`:
//! the proposed definitions as the palette reads definitions, and the own blocks the project has
//! defined itself; with `block`, it gives `{ ok, creates }`,
//! the files to write for that block, by path below the project; with `block` and `symbol` (and
//! `bind`, chosen members by input), `{ ok, edits }`: the definition and the pages showing the
//! block, with that picture.

mod blocks;
mod definition;
pub(super) mod picture;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::my_symbols::symbols_in;
use trust_runtime::hmi::page_files::{parse_pair, HmiPair};

use super::hmi_authoring::declarations::Declarations;
use super::hmi_authoring::{bundle, components, sources};
use super::hmi_files::{refused, single_argument};
use super::{HmiBindingsCommandArgs, HMI_OWN_EQUIPMENT_COMMAND};
use crate::handlers::context::ServerContext;
use blocks::{own_blocks, OwnBlock};
use definition::{propose, Proposal};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    root_uri: Option<Url>,
    /// Unsaved texts by project-relative path.
    #[serde(default)]
    open: BTreeMap<String, String>,
    /// The block whose files to create, or whose picture to change.
    #[serde(default)]
    block: Option<String>,
    /// The picture to give the block's definition.
    #[serde(default)]
    symbol: Option<String>,
    /// The members chosen for the picture's inputs, by input.
    #[serde(default)]
    bind: BTreeMap<String, String>,
}

pub(super) fn own_equipment<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    own_equipment_result(context, args).unwrap_or_else(refused)
}

fn own_equipment_result<C: ServerContext>(context: &C, args: Vec<Value>) -> Result<Value, String> {
    let args: Args = single_argument(args, HMI_OWN_EQUIPMENT_COMMAND)?;
    let selection = HmiBindingsCommandArgs {
        open: args.open,
        root_uri: args.root_uri,
        text_document: None,
    };
    let inputs = super::project_inputs(context, &selection, HMI_OWN_EQUIPMENT_COMMAND)?;
    let index = Declarations::read(&inputs.sources);
    let libraries = sources::dependency_roots(&inputs.root, &inputs.open)?;
    let edited =
        bundle::read_edited(&inputs.root, &inputs.open).map_err(|error| error.to_string())?;
    if let (Some(block), Some(symbol)) = (&args.block, &args.symbol) {
        let (symbols, _) = symbols_in(&edited.files);
        let catalogue = Catalogue::shipped().with_symbols(&symbols);
        return picture::change(
            &inputs.root,
            &inputs.open,
            index.as_ref(),
            &catalogue,
            block,
            symbol,
            &args.bind,
        );
    }
    let proposals = proposals(index.as_ref(), &inputs.root, &libraries, &edited);
    match args.block {
        Some(block) => creates(&proposals, &edited, &block),
        None => {
            // The own blocks the project already defined itself, listed with the proposals.
            let mine: Vec<String> = own_blocks(index.as_ref(), &inputs.root, &libraries)
                .into_iter()
                .filter(|block| {
                    picture::own_pair(&inputs.root, &inputs.open, &block.name).is_some()
                })
                .map(|block| block.name)
                .collect();
            Ok(
                json!({ "ok": true, "equipment": proposed_definitions(&proposals, &edited), "mine": mine }),
            )
        }
    }
}

/// Each own block without a definition, with the definition proposed for it.
fn proposals(
    index: &Declarations,
    root: &Path,
    libraries: &[std::path::PathBuf],
    edited: &HmiBundle,
) -> Vec<(OwnBlock, Proposal)> {
    let pairs: Vec<HmiPair> = edited
        .files
        .iter()
        .filter(|(path, _)| path.starts_with("hmi/pairs/") && path.ends_with(".toml"))
        .filter_map(|(path, bytes)| parse_pair(path, &String::from_utf8_lossy(bytes)).ok())
        .collect();
    let defined = |block: &str| {
        pairs
            .iter()
            .find(|pair| pair.block.eq_ignore_ascii_case(block))
    };
    own_blocks(index, root, libraries)
        .into_iter()
        .filter(|block| defined(&block.name).is_none())
        .map(|block| {
            // The nearest base with a definition lends it, so a block extending a Plant pump is
            // a pump with its panel (§21.9 item 3).
            let mut base = block.base.clone();
            let mut steps = 0;
            let lent = loop {
                match base {
                    Some(name) if steps < 32 => {
                        if let Some(pair) = defined(&name) {
                            break Some(pair);
                        }
                        base = index.base_of(&name);
                        steps += 1;
                    }
                    _ => break None,
                }
            };
            let proposal = propose(&block, lent);
            (block, proposal)
        })
        .collect()
}

/// The proposed definitions as the palette reads definitions: the proposals' files read with the
/// project's, by the same reader as the project's definitions, each marked `proposed`.
fn proposed_definitions(proposals: &[(OwnBlock, Proposal)], edited: &HmiBundle) -> Vec<Value> {
    let mut bundle = edited.clone();
    for (_, proposal) in proposals {
        bundle.files.insert(
            proposal.pair_path.clone(),
            proposal.pair.clone().into_bytes(),
        );
        if let Some((path, text)) = &proposal.panel {
            bundle.files.insert(path.clone(), text.clone().into_bytes());
        }
    }
    let (symbols, _) = symbols_in(&bundle.files);
    let catalogue = Catalogue::shipped().with_symbols(&symbols);
    let (definitions, _) = components::equipment_definitions(&bundle, &catalogue);
    definitions
        .into_iter()
        .filter(|definition| {
            proposals
                .iter()
                .any(|(block, _)| block.name.eq_ignore_ascii_case(&definition.block))
        })
        .filter_map(|definition| {
            let mut value = serde_json::to_value(definition).ok()?;
            value["proposed"] = json!(true);
            Some(value)
        })
        .collect()
}

/// The files to write for one proposed block; a file that already exists is refused.
fn creates(
    proposals: &[(OwnBlock, Proposal)],
    edited: &HmiBundle,
    block: &str,
) -> Result<Value, String> {
    let (_, proposal) = proposals
        .iter()
        .find(|(candidate, _)| candidate.name.eq_ignore_ascii_case(block))
        .ok_or_else(|| {
            format!("{block} is not one of the project's own function blocks without a definition")
        })?;
    let mut files = vec![(proposal.pair_path.clone(), proposal.pair.clone())];
    files.extend(proposal.panel.clone());
    if let Some((path, _)) = files
        .iter()
        .find(|(path, _)| edited.files.contains_key(path))
    {
        return Err(format!(
            "{path} already exists: rename it or choose its block in it"
        ));
    }
    Ok(json!({
        "ok": true,
        "creates": files.into_iter().map(|(path, text)| json!({ "path": path, "text": text })).collect::<Vec<_>>(),
    }))
}
