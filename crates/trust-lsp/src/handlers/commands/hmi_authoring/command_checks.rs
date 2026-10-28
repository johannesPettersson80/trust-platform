//! Customer and supplied panels' method commands are checked with the runtime's capability checks,
//! against the block each command reaches: the definition's block, or for `$instance.<member>` the
//! type of that member (a cascade loop's `Outer`).
use super::{commands::MethodCommand, pairs::Pair};
use serde_json::Value;
use smol_str::SmolStr;
use trust_runtime::program_model::FunctionBlockBase;
use trust_runtime::RuntimeMetadata;

pub(crate) fn check(
    metadata: &RuntimeMetadata,
    pair: &Pair,
    panel: &Value,
) -> Vec<(String, String)> {
    let Some(objects) = panel["object"].as_array() else {
        return vec![];
    };
    objects
        .iter()
        .filter_map(|object| {
            let raw = object.get("command")?;
            let result = serde_json::from_value::<MethodCommand>(raw.clone())
                .map_err(|e| e.to_string())
                .and_then(|command| {
                    let block = match command.instance.strip_prefix("$instance.") {
                        Some(member) => member_type(metadata, &pair.block, member)
                            .ok_or_else(|| format!("{} has no member {member}", pair.block))?,
                        None => pair.block.clone(),
                    };
                    trust_runtime::hmi::commands::capability_checks::check(
                        metadata, &block, &command,
                    )
                });
            result.err().map(|message| {
                (
                    object["id"].as_str().unwrap_or("(unnamed)").to_owned(),
                    message,
                )
            })
        })
        .collect()
}

/// The type of a variable, input or output a block declares or inherits.
fn member_type(metadata: &RuntimeMetadata, block: &str, member: &str) -> Option<String> {
    let blocks = metadata.function_blocks();
    let mut current = blocks.get(&SmolStr::new(block.to_ascii_uppercase()));
    let mut depth = 0;
    while let Some(definition) = current {
        let declared = definition
            .vars
            .iter()
            .find(|var| var.name.eq_ignore_ascii_case(member))
            .map(|var| var.type_id)
            .or_else(|| {
                definition
                    .params
                    .iter()
                    .find(|param| param.name.eq_ignore_ascii_case(member))
                    .map(|param| param.type_id)
            });
        if let Some(type_id) = declared {
            return metadata
                .registry()
                .type_name(type_id)
                .map(|name| name.to_string());
        }
        depth += 1;
        if depth > 32 {
            return None;
        }
        current = match &definition.base {
            Some(FunctionBlockBase::FunctionBlock(base)) => {
                blocks.get(&SmolStr::new(base.to_ascii_uppercase()))
            }
            _ => None,
        };
    }
    None
}
