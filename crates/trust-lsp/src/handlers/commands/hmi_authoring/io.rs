//! Hardware terminals of equipment (specification 39, §21.3): the addressed PLC signals they may be
//! connected to and whether they are, from declarations only, independent of runtime I/O drivers.
use super::super::hmi_plant_project::PlantDesign;
use super::declarations::Declarations;
use super::equipment_settings;
use super::pairs::{HmiTerminal, HmiTerminalDirection};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use trust_runtime::runtime_core::io_address::IoSize;
use trust_runtime::runtime_core::io_image::expected_size_for_type;
use trust_runtime::runtime_core::stdlib::conversions::ConversionType;
use trust_runtime::{io::IoAddress, memory::IoArea};
use trust_syntax::{parser::parse, SyntaxKind as K};

pub(in crate::handlers::commands) struct Binding {
    pub path: String,
    pub address: IoAddress,
    pub type_name: String,
}
pub(in crate::handlers::commands) fn bindings(
    index: &Declarations,
    sources: &[super::super::LoadedSource],
) -> Vec<Binding> {
    let mut found = BTreeMap::new();
    let mut nodes = Vec::new();
    fn walk<'a>(
        nodes: &'a [trust_runtime::hmi::variable_tree::VariableNode],
        out: &mut Vec<&'a trust_runtime::hmi::variable_tree::VariableNode>,
    ) {
        for node in nodes {
            out.push(node);
            walk(&node.children, out);
        }
    }
    walk(&index.variables, &mut nodes);
    for symbol in index.symbols.iter() {
        let Some(address) = symbol
            .direct_address
            .as_deref()
            .and_then(|text| IoAddress::parse(text).ok())
        else {
            continue;
        };
        let paths: Vec<_> = if let Some(parent) = symbol.parent {
            let name = index.name(parent);
            nodes
                .iter()
                .filter(|node| node.type_name.eq_ignore_ascii_case(&name))
                .map(|node| format!("{}.{}", node.path, symbol.name))
                .collect()
        } else {
            vec![index.name(symbol.id)]
        };
        for path in paths {
            if let Some(node) = index.resolve(&path) {
                found.insert(
                    path.clone(),
                    Binding {
                        path,
                        address: address.clone(),
                        type_name: node.type_name.clone(),
                    },
                );
            }
        }
    }
    for source in sources {
        let root = parse(&source.text).syntax();
        for config in root
            .descendants()
            .filter(|node| node.kind() == K::ConfigInit)
        {
            let Some(path) = config.children().find(|node| node.kind() == K::AccessPath) else {
                continue;
            };
            let mut path = path
                .text()
                .to_string()
                .split_whitespace()
                .collect::<String>();
            let prefixes: Vec<_> = config
                .ancestors()
                .filter(|node| matches!(node.kind(), K::Configuration | K::Resource))
                .filter_map(|node| {
                    node.children()
                        .find(|node| node.kind() == K::Name)
                        .map(|node| node.text().to_string().trim().to_string())
                })
                .collect();
            for prefix in prefixes.iter().rev() {
                if path
                    .get(..prefix.len())
                    .is_some_and(|part| part.eq_ignore_ascii_case(prefix))
                    && path.as_bytes().get(prefix.len()) == Some(&b'.')
                {
                    path = path[prefix.len() + 1..].into();
                }
            }
            let address = config
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| token.kind() == K::DirectAddress)
                .and_then(|token| IoAddress::parse(token.text()).ok());
            if let (Some(address), Some(node)) = (address, index.resolve(&path)) {
                found.insert(
                    path.clone(),
                    Binding {
                        path,
                        address,
                        type_name: node.type_name.clone(),
                    },
                );
            }
        }
    }
    found.into_values().collect()
}
fn selected<'a>(bindings: &'a [Binding], path: &str) -> Option<&'a Binding> {
    bindings
        .iter()
        .find(|binding| binding.path.eq_ignore_ascii_case(path))
}
pub(in crate::handlers::commands) fn validate(
    index: &Declarations,
    bindings: &[Binding],
    manifest: &PlantDesign,
    instance: &str,
    block: &str,
    terminal: &HmiTerminal,
    target: &str,
) -> Result<(), String> {
    let binding = selected(bindings, target).ok_or("Select a declared hardware I/O signal")?;
    let area = if terminal.direction == HmiTerminalDirection::Input {
        IoArea::Input
    } else {
        IoArea::Output
    };
    if binding.address.wildcard || binding.address.area != area {
        return Err(format!(
            "{} needs a concrete {:?} channel",
            terminal.title, terminal.direction
        ));
    }
    let member = index
        .block_members(block)
        .and_then(|members| {
            members
                .into_iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&terminal.member))
        })
        .ok_or("The equipment terminal is not declared")?
        .1;
    let target_type = index
        .registry
        .lookup(&binding.type_name)
        .ok_or("The I/O type is unresolved")?;
    if index.symbols.resolve_alias_type(member) != index.symbols.resolve_alias_type(target_type) {
        return Err(
            "The equipment and I/O signal have different types; use an explicit conversion".into(),
        );
    }
    if area == IoArea::Output {
        if bindings.iter().any(|other| {
            !std::ptr::eq(other, binding)
                && super::super::hmi_equipment_io::overlaps(&other.address, &binding.address)
        }) {
            return Err("This output overlaps another hardware binding".into());
        }
        for (path, entry) in &manifest.equipment {
            for (member, signal) in &entry.signals {
                if path.eq_ignore_ascii_case(instance)
                    && member.eq_ignore_ascii_case(&terminal.member)
                {
                    continue;
                }
                if selected(bindings, signal).is_some_and(|other| {
                    super::super::hmi_equipment_io::overlaps(&other.address, &binding.address)
                }) {
                    return Err(format!("This output already belongs to {path}.{member}"));
                }
            }
        }
    }
    Ok(())
}
/// `known` with the member each terminal's `required_when` names, when it lacks it, at the block's
/// default: its initial value, or FALSE for a BOOL declared without one. A terminal needed only
/// with feedback is then optional while the block has its feedback off by default.
pub(in crate::handlers::commands) fn with_block_conditions(
    index: &Declarations,
    block: &str,
    terminals: &[HmiTerminal],
    known: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut all = known.clone();
    for condition in terminals
        .iter()
        .filter_map(|terminal| terminal.required_when.as_deref())
    {
        if all
            .keys()
            .any(|member| member.eq_ignore_ascii_case(condition))
        {
            continue;
        }
        let default = equipment_settings::block_default(index, block, condition).or_else(|| {
            matches!(
                equipment_settings::kind_of(index, block, condition),
                equipment_settings::Kind::Bool
            )
            .then(|| "FALSE".to_string())
        });
        if let Some(default) = default {
            all.insert(condition.to_string(), default);
        }
    }
    all
}

/// Whether a terminal is needed: always, unless it has `required_when` naming a BOOL input or
/// setting that is known to be FALSE for this instance (`HasFeedback := FALSE`).
pub(in crate::handlers::commands) fn required(
    terminal: &HmiTerminal,
    known: &BTreeMap<String, String>,
) -> bool {
    match &terminal.required_when {
        Some(condition) => !known.iter().any(|(member, value)| {
            member.eq_ignore_ascii_case(condition) && value.trim().eq_ignore_ascii_case("FALSE")
        }),
        None => terminal.required,
    }
}

/// Each terminal with its connected signal (the design's choice, or the instance's own addressed
/// member), a problem with it, and the signals it may be connected to. `known` holds the member
/// values known for the instance as ST literals.
pub(in crate::handlers::commands) fn describe(
    index: &Declarations,
    bindings: &[Binding],
    manifest: &PlantDesign,
    instance: &str,
    block: &str,
    terminals: &[HmiTerminal],
    known: &BTreeMap<String, String>,
) -> Vec<Value> {
    let known = &with_block_conditions(index, block, terminals, known);
    let entry = manifest.entry(instance);
    terminals
        .iter()
        .map(|terminal| {
            let value = entry
                .and_then(|entry| entry.signals.get(&terminal.member))
                .cloned()
                .or_else(|| {
                    let path = format!("{instance}.{}", terminal.member);
                    selected(bindings, &path).map(|_| path)
                });
            let error = value.as_ref().and_then(|target| {
                validate(index, bindings, manifest, instance, block, terminal, target).err()
            });
            let choices: Vec<_> = bindings
                .iter()
                .filter(|binding| {
                    validate(
                        index,
                        bindings,
                        manifest,
                        instance,
                        block,
                        terminal,
                        &binding.path,
                    )
                    .is_ok()
                })
                .map(binding_json)
                .collect();
            // The member's type, so the editor can show a declaration that would fit.
            let type_name = index
                .block_members(block)
                .and_then(|members| {
                    members
                        .into_iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(&terminal.member))
                })
                .and_then(|(_, ty)| index.symbols.type_name(ty))
                .map(|name| name.to_string());
            let help = choices
                .is_empty()
                .then(|| missing_signal_help(instance, terminal, type_name.as_deref()));
            json!({
                "member": terminal.member,
                "title": terminal.title,
                "direction": terminal.direction,
                "type": type_name,
                "required": required(terminal, known),
                "value": value,
                "error": error,
                "choices": choices,
                "help": help,
            })
        })
        .collect()
}

/// What a terminal that no declared PLC signal fits tells the user (HMI component catalogue §8.1):
/// declare one with an address in the program, for example one named after the instance's last
/// name and the member, in the terminal's direction, at the first address of the size the runtime
/// binds the type to (IEC 61131-3 §6.5.5; `expected_size_for_type`; a `STRING` is bound as the
/// byte window at a byte address). A type that the runtime binds at no size gets the advice
/// without an example; a member whose type is unknown is taken as `BOOL`.
fn missing_signal_help(instance: &str, terminal: &HmiTerminal, type_name: Option<&str>) -> String {
    let type_name = type_name.unwrap_or("BOOL");
    let (direction, area) = match terminal.direction {
        HmiTerminalDirection::Input => ("input", 'I'),
        HmiTerminalDirection::Output => ("output", 'Q'),
    };
    let advice = format!(
        "No {direction} of type {type_name} is declared yet. Declare one with an address in your program"
    );
    let size = ConversionType::from_builtin_name(type_name).and_then(|ty| match ty {
        ConversionType::String => Some(IoSize::Byte),
        ty => expected_size_for_type(ty),
    });
    let Some(size) = size else {
        return format!("{advice}, then press Refresh.");
    };
    let location = match size {
        IoSize::Bit => "X0.0",
        IoSize::Byte | IoSize::Bytes(_) => "B0",
        IoSize::Word => "W0",
        IoSize::DWord => "D0",
        IoSize::LWord => "L0",
    };
    let name = instance.rsplit('.').next().unwrap_or(instance);
    format!(
        "{advice}, for example {name}_{} AT %{area}{location} : {type_name};, then press Refresh.",
        terminal.member
    )
}

/// The hardware status shown in the inspector (specification 39, §21.3 item 2; catalogue §8.1):
/// `none` without terminals, `check` when a connection has a problem, `not-connected` while no
/// terminal is connected, `connected` when every needed terminal is, else `partly`.
pub(in crate::handlers::commands) fn status(rows: &[Value]) -> &'static str {
    let connected = |row: &Value| !row["value"].is_null();
    if rows.is_empty() {
        "none"
    } else if rows.iter().any(|row| !row["error"].is_null()) {
        "check"
    } else if !rows.iter().any(connected) {
        "not-connected"
    } else if rows
        .iter()
        .filter(|row| row["required"] == true)
        .all(connected)
    {
        "connected"
    } else {
        "partly"
    }
}

pub(in crate::handlers::commands) fn binding_json(binding: &Binding) -> Value {
    json!({"path":binding.path,"address":binding.address.to_string(),"type":binding.type_name,"direction":match binding.address.area{IoArea::Input=>"input",IoArea::Output=>"output",_=>"memory"}})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(member: &str, required_when: Option<&str>) -> HmiTerminal {
        HmiTerminal {
            member: member.into(),
            title: member.into(),
            direction: HmiTerminalDirection::Input,
            required: required_when.is_none(),
            required_when: required_when.map(Into::into),
        }
    }

    #[test]
    fn a_terminal_needed_only_with_feedback_is_optional_when_feedback_is_off() {
        let feedback = terminal("RunningFeedback", Some("HasFeedback"));
        assert!(
            required(&feedback, &BTreeMap::new()),
            "needed while HasFeedback is unknown"
        );
        let off = BTreeMap::from([("HasFeedback".to_string(), "FALSE".to_string())]);
        assert!(!required(&feedback, &off));
        assert!(required(&terminal("RunCommand", None), &off));
    }

    /// A terminal that no signal fits names a declaration the runtime accepts: its address is in
    /// the terminal's direction at the size the runtime binds the type to (IEC 61131-3 §6.5.5:
    /// X a bit, B a byte, W a word, D a double word, L a long word; a STRING's byte window at a
    /// byte address). Catches the long-word address the editor suggested for TIME, WCHAR and
    /// STRING, which the runtime refuses, and an output declared as an input.
    #[test]
    fn a_terminal_no_signal_fits_names_a_declaration_the_runtime_binds() {
        let raw = terminal("Raw", None);
        assert_eq!(
            missing_signal_help("Main.TT101", &raw, Some("LREAL")),
            "No input of type LREAL is declared yet. Declare one with an address in your program, \
             for example TT101_Raw AT %IL0 : LREAL;, then press Refresh."
        );
        let run = HmiTerminal {
            direction: HmiTerminalDirection::Output,
            ..terminal("RunCommand", None)
        };
        assert_eq!(
            missing_signal_help("Main.P101", &run, Some("BOOL")),
            "No output of type BOOL is declared yet. Declare one with an address in your program, \
             for example P101_RunCommand AT %QX0.0 : BOOL;, then press Refresh."
        );
        for (type_name, address) in [
            ("BOOL", "%IX0.0"),
            ("BYTE", "%IB0"),
            ("SINT", "%IB0"),
            ("CHAR", "%IB0"),
            ("STRING", "%IB0"),
            ("INT", "%IW0"),
            ("WORD", "%IW0"),
            ("WCHAR", "%IW0"),
            ("DINT", "%ID0"),
            ("REAL", "%ID0"),
            ("TIME", "%ID0"),
            ("LINT", "%IL0"),
            ("LREAL", "%IL0"),
            ("LTIME", "%IL0"),
            ("int", "%IW0"),
        ] {
            let help = missing_signal_help("Main.FT1", &raw, Some(type_name));
            assert!(
                help.contains(&format!("FT1_Raw AT {address} : {type_name};")),
                "{type_name}: {help}"
            );
        }
    }

    /// A type that the runtime binds at no address size gets the advice without an example, and a
    /// member whose type is unknown is taken as BOOL. Catches an address invented for a DATE, a
    /// WSTRING or a structure.
    #[test]
    fn a_type_without_an_address_size_gets_no_example() {
        let raw = terminal("Raw", None);
        for type_name in ["DATE", "TOD", "WSTRING", "Control.PIDInput"] {
            assert_eq!(
                missing_signal_help("Main.FT1", &raw, Some(type_name)),
                format!(
                    "No input of type {type_name} is declared yet. Declare one with an address in \
                     your program, then press Refresh."
                ),
                "{type_name}"
            );
        }
        assert!(
            missing_signal_help("FT1", &raw, None).contains("FT1_Raw AT %IX0.0 : BOOL;"),
            "an unknown type is taken as BOOL"
        );
    }

    #[test]
    fn the_hardware_status_says_connected_only_when_every_needed_terminal_is() {
        let row = |required: bool, value: Option<&str>, error: Option<&str>| json!({"required": required, "value": value, "error": error});
        assert_eq!(status(&[]), "none");
        assert_eq!(status(&[row(true, None, None)]), "not-connected");
        assert_eq!(
            status(&[row(true, Some("Main.Run"), None), row(true, None, None)]),
            "partly"
        );
        assert_eq!(
            status(&[row(true, Some("Main.Run"), None), row(false, None, None)]),
            "connected"
        );
        assert_eq!(
            status(&[row(true, Some("Main.Run"), Some("wrong type"))]),
            "check"
        );
    }
}
