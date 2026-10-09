//! The real values of an instance's settings for the builder's inspector (specification 39, §21.3
//! item 2): the design's choice in `plant.toml`, else the instance declaration's initial value,
//! else the block's declared default, shown in the setting's unit; and the conversion of an edited
//! value back to the ST literal Create PLC code writes. Declarations only: nothing is compiled or
//! run.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use trust_hir::symbols::{Symbol, SymbolId};
use trust_hir::types::{Type, TypeId};
use trust_runtime::hmi::page_files::{HmiScalar, HmiSetting};
use trust_syntax::{parser::parse, SyntaxKind, SyntaxNode};

use super::super::hmi_plant_project::DesignEntry;
use super::declarations::Declarations;

/// What kind of value a setting holds, from the member's declared type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::handlers::commands) enum Kind {
    Bool,
    Integer,
    Real,
    Time,
    Text,
    Other,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Integer => "integer",
            Self::Real => "real",
            Self::Time => "time",
            Self::Text => "text",
            Self::Other => "other",
        }
    }
}

/// Durations are read with the runtime's parser and edited in the runtime's units (§21.4).
pub(in crate::handlers::commands) use trust_runtime::hmi::setting_values::duration_nanos;
use trust_runtime::hmi::setting_values::time_unit;

/// An exact ST duration literal in the largest whole unit.
pub(in crate::handlers::commands) fn time_literal(nanos: i128) -> String {
    for (unit, size) in [
        ("h", 3_600_000_000_000_i128),
        ("m", 60_000_000_000),
        ("s", 1_000_000_000),
        ("ms", 1_000_000),
        ("us", 1_000),
    ] {
        if nanos != 0 && nanos % size == 0 {
            return format!("T#{}{unit}", nanos / size);
        }
    }
    if nanos == 0 {
        "T#0s".to_string()
    } else {
        format!("T#{nanos}ns")
    }
}

/// An ST string literal.
fn st_string(text: &str) -> String {
    format!("'{}'", text.replace('$', "$$").replace('\'', "$'"))
}

/// The text of an ST string literal.
fn unquote(literal: &str) -> String {
    let inner = literal
        .trim()
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .unwrap_or(literal);
    inner.replace("$'", "'").replace("$$", "$")
}

/// A numeric literal without its type prefix (`LREAL#1.5`) and digit separators.
fn number_text(literal: &str) -> String {
    let text = literal.trim();
    let text = text.rsplit_once('#').map_or(text, |(_, value)| value);
    text.replace('_', "")
}

/// The type of a member path below a block (`StartTimeout`, `Outer.ProportionalGain`).
fn member_type(index: &Declarations, block: &str, member: &str) -> Option<TypeId> {
    let mut owner = block.to_string();
    let mut found = None;
    for part in member.split('.') {
        let ty = index
            .block_members(&owner)?
            .into_iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(part))?
            .1;
        owner = index
            .symbols
            .type_name(ty)
            .map(|name| name.to_string())
            .unwrap_or_default();
        found = Some(ty);
    }
    found
}

/// The kind of a setting's member; a member beside a loop's PID (`{instance}_Setpoint`, §10.5)
/// has the kind of the loop's variable.
pub(in crate::handlers::commands) fn kind_of(
    index: &Declarations,
    block: &str,
    member: &str,
) -> Kind {
    if trust_runtime::hmi::page_files::beside_instance(member) {
        return super::super::hmi_generate::beside_kind(member).unwrap_or(Kind::Other);
    }
    let Some(ty) = member_type(index, block, member) else {
        return Kind::Other;
    };
    match index.registry.get(index.symbols.resolve_alias_type(ty)) {
        Some(Type::Bool) => Kind::Bool,
        Some(
            Type::SInt
            | Type::Int
            | Type::DInt
            | Type::LInt
            | Type::USInt
            | Type::UInt
            | Type::UDInt
            | Type::ULInt,
        ) => Kind::Integer,
        Some(Type::Real | Type::LReal) => Kind::Real,
        Some(Type::Time | Type::LTime) => Kind::Time,
        Some(Type::String { .. } | Type::WString { .. }) => Kind::Text,
        _ => Kind::Other,
    }
}

/// The node after `:=` in the declaration of a symbol: its initial value.
fn initializer_of(index: &Declarations, symbol: &Symbol) -> Option<SyntaxNode> {
    let origin = symbol.origin?;
    let text = index.source_texts.get(&origin.file_id)?;
    let root = parse(text).syntax();
    let declaration = root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|node| node.text_range().contains_range(symbol.range))?;
    let assign = declaration
        .children_with_tokens()
        .find(|element| element.kind() == SyntaxKind::Assign)?
        .text_range()
        .end();
    declaration
        .children()
        .find(|node| node.text_range().start() >= assign)
}

/// The `name := value` entries of an aggregate initializer, as texts.
fn aggregate_entries(list: &SyntaxNode) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let mut name: Option<String> = None;
    for child in list.children() {
        if child.kind() == SyntaxKind::Name {
            name = Some(child.text().to_string().trim().to_string());
        } else if let Some(member) = name.take() {
            entries.insert(member, child.text().to_string().trim().to_string());
        }
    }
    entries
}

/// The symbol a block declares, or inherits, for a member.
fn member_symbol<'a>(index: &'a Declarations, block: &str, member: &str) -> Option<&'a Symbol> {
    use trust_hir::semantic::SemanticOutcome;
    let mut owner: Option<SymbolId> = Some(index.symbol(block)?.id);
    let mut depth = 0;
    while let Some(id) = owner {
        if let Some(symbol) = index
            .symbols
            .iter()
            .find(|symbol| symbol.parent == Some(id) && symbol.name.eq_ignore_ascii_case(member))
        {
            return Some(symbol);
        }
        depth += 1;
        if depth > 32 {
            return None;
        }
        owner =
            index
                .symbols
                .extends_reference(id)
                .and_then(|reference| match reference.outcome() {
                    SemanticOutcome::Resolved(target) => Some(*target),
                    _ => None,
                });
    }
    None
}

/// The default a block declares for a member: its declaration's initial value, through the blocks
/// it extends; a nested member takes the default its own block declares.
pub(in crate::handlers::commands) fn block_default(
    index: &Declarations,
    block: &str,
    member: &str,
) -> Option<String> {
    // A variable beside the instance takes its start from the definition, not from the block.
    if trust_runtime::hmi::page_files::beside_instance(member) {
        return None;
    }
    let (owner, last) = match member.rsplit_once('.') {
        Some((path, last)) => {
            let ty = member_type(index, block, path)?;
            (index.symbols.type_name(ty)?.to_string(), last.to_string())
        }
        None => (block.to_string(), member.to_string()),
    };
    let symbol = member_symbol(index, &owner, &last)?;
    initializer_of(index, symbol).map(|node| node.text().to_string().trim().to_string())
}

/// The initial values an instance's own declaration sets (`P101 : FB_Pump := (A := 1)`), and
/// those of the variables beside it, named after it, keyed as members beside the instance
/// (`TIC101_Parameters := (ProportionalGain := 2.0)` gives `{instance}_Parameters.ProportionalGain`).
pub(in crate::handlers::commands) fn instance_initializers(
    index: &Declarations,
    instance: &str,
) -> BTreeMap<String, String> {
    let mut values = own_initializers(index, instance);
    values.extend(beside_initializers(index, instance));
    values
}

/// The initial values of the variables beside an instance (`TIC101_Setpoint`, `TIC101_Parameters`).
fn beside_initializers(index: &Declarations, instance: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let Some((root, name)) = instance.split_once('.') else {
        return values;
    };
    if name.contains('.') {
        return values;
    }
    let Some(program) = index
        .symbol(
            &index
                .variables
                .iter()
                .find(|node| node.path.eq_ignore_ascii_case(root))
                .map_or_else(|| root.to_string(), |node| node.type_name.clone()),
        )
        .map(|program| program.id)
    else {
        return values;
    };
    let prefix = format!("{name}_").to_ascii_lowercase();
    for symbol in index.symbols.iter().filter(|symbol| {
        symbol.parent == Some(program)
            && symbol.name.len() > prefix.len()
            && symbol.name.to_ascii_lowercase().starts_with(&prefix)
    }) {
        let member = format!(
            "{}{}",
            trust_runtime::hmi::page_files::INSTANCE_PLACEHOLDER,
            &symbol.name[name.len()..]
        );
        match initializer_of(index, symbol) {
            Some(list) if list.kind() == SyntaxKind::InitializerList => {
                for (field, value) in aggregate_entries(&list) {
                    values.insert(format!("{member}.{field}"), value);
                }
            }
            Some(value) => {
                values.insert(member, value.text().to_string().trim().to_string());
            }
            None => {}
        }
    }
    values
}

/// The initial values an instance's own declaration sets.
fn own_initializers(index: &Declarations, instance: &str) -> BTreeMap<String, String> {
    let Some((root, name)) = instance.split_once('.') else {
        return BTreeMap::new();
    };
    if name.contains('.') {
        return BTreeMap::new();
    }
    let program_type = index
        .variables
        .iter()
        .find(|node| node.path.eq_ignore_ascii_case(root))
        .map_or_else(|| root.to_string(), |node| node.type_name.clone());
    let Some(program) = index.symbol(&program_type) else {
        return BTreeMap::new();
    };
    let Some(symbol) = index
        .symbols
        .iter()
        .find(|symbol| symbol.parent == Some(program.id) && symbol.name.eq_ignore_ascii_case(name))
    else {
        return BTreeMap::new();
    };
    match initializer_of(index, symbol) {
        Some(list) if list.kind() == SyntaxKind::InitializerList => aggregate_entries(&list),
        _ => BTreeMap::new(),
    }
}

/// A literal as the inspector shows it, in the setting's unit.
fn display(kind: Kind, setting: &HmiSetting, literal: &str) -> Value {
    if !setting.choices.is_empty() {
        let chosen = setting.choices.iter().find(|choice| match &choice.value {
            HmiScalar::Bool(flag) => {
                literal
                    .trim()
                    .eq_ignore_ascii_case(if *flag { "TRUE" } else { "FALSE" })
            }
            HmiScalar::Number(number) => number_text(literal).parse::<f64>().ok() == Some(*number),
            HmiScalar::Text(text) => match kind {
                Kind::Time => duration_nanos(text) == duration_nanos(literal),
                Kind::Text => unquote(literal) == *text,
                _ => literal.trim().eq_ignore_ascii_case(text),
            },
        });
        if let Some(choice) = chosen {
            return serde_json::to_value(&choice.value).unwrap_or(Value::Null);
        }
    }
    match kind {
        Kind::Bool => json!(literal.trim().eq_ignore_ascii_case("TRUE")),
        Kind::Integer => number_text(literal)
            .parse::<i64>()
            .map_or(Value::Null, |value| json!(value)),
        Kind::Real => number_text(literal)
            .parse::<f64>()
            .map_or(Value::Null, |value| json!(value)),
        Kind::Time => duration_nanos(literal).map_or(Value::Null, |nanos| {
            json!(nanos as f64 / time_unit(setting).1)
        }),
        Kind::Text => json!(unquote(literal)),
        Kind::Other => json!(literal.trim()),
    }
}

/// The zero value of a kind, for a member no declaration initializes.
fn zero(kind: Kind) -> &'static str {
    match kind {
        Kind::Bool => "FALSE",
        Kind::Integer => "0",
        Kind::Real => "0.0",
        Kind::Time => "T#0s",
        Kind::Text => "''",
        Kind::Other => "",
    }
}

/// The ST literal of a value written in a definition, for a member of this kind.
pub(in crate::handlers::commands) fn scalar_literal(kind: Kind, value: &HmiScalar) -> String {
    match value {
        HmiScalar::Bool(flag) => if *flag { "TRUE" } else { "FALSE" }.to_string(),
        HmiScalar::Number(number) if kind == Kind::Integer => format!("{}", *number as i64),
        HmiScalar::Number(number) => real_literal(*number),
        HmiScalar::Text(text) if kind == Kind::Text => st_string(text),
        HmiScalar::Text(text) => text.clone(),
    }
}

/// The value of a member as Create PLC code initializes it, as an ST literal: the design's choice,
/// else the instance declaration's, else the definition's initial value; `None` keeps the block's
/// own default.
pub(in crate::handlers::commands) fn chosen_literal(
    index: &Declarations,
    block: &str,
    member: &str,
    design: Option<&DesignEntry>,
    declared: &BTreeMap<String, String>,
    initial: &BTreeMap<String, HmiScalar>,
) -> Option<(String, &'static str)> {
    let find = |map: &BTreeMap<String, String>| {
        map.iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(member))
            .map(|(_, value)| value.clone())
    };
    if let Some(value) = design.and_then(|entry| find(&entry.settings)) {
        return Some((value, "design"));
    }
    if let Some(value) = find(declared) {
        return Some((value, "declaration"));
    }
    initial
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(member))
        .map(|(_, value)| {
            (
                scalar_literal(kind_of(index, block, member), value),
                "design",
            )
        })
}

/// The settings of an instance for the inspector, each with its real value and where it comes
/// from (`design`, `declaration` or `default`). `initial` holds the definition's instance initial
/// values, which Create PLC code writes like design choices.
pub(in crate::handlers::commands) fn describe(
    index: &Declarations,
    instance: &str,
    block: &str,
    settings: &[HmiSetting],
    design: Option<&DesignEntry>,
    initial: &BTreeMap<String, HmiScalar>,
) -> Vec<Value> {
    let declared = instance_initializers(index, instance);
    let value_of = |member: &str| -> Option<(String, &'static str)> {
        chosen_literal(index, block, member, design, &declared, initial)
    };
    let unit_member = |member: &str| -> Option<String> {
        value_of(member)
            .map(|(literal, _)| literal)
            .or_else(|| block_default(index, block, member))
            .map(|literal| unquote(&literal))
    };
    settings
        .iter()
        .map(|setting| {
            let kind = kind_of(index, block, &setting.member);
            let (literal, source) = if let Some((value, source)) = value_of(setting.member.as_str())
            {
                (value, source)
            } else {
                (
                    block_default(index, block, &setting.member)
                        .unwrap_or_else(|| zero(kind).to_string()),
                    "default",
                )
            };
            // As the runtime's Settings dialog shows it: a duration in its editing unit.
            let unit = match setting.unit.as_deref() {
                _ if kind == Kind::Time && setting.choices.is_empty() => {
                    Some(time_unit(setting).0.to_string())
                }
                _ if kind == Kind::Time => None,
                Some(unit) if unit.starts_with('$') => unit_member(&unit[1..]),
                other => other.map(str::to_string),
            };
            json!({
                "member": setting.member,
                "title": setting.title,
                "unit": unit,
                "min": setting.min,
                "max": setting.max,
                "step": setting.step,
                "choices": setting.choices,
                "help": setting.help,
                "key": setting.key,
                "kind": kind.name(),
                "value": display(kind, setting, &literal),
                "source": source,
            })
        })
        .collect()
}

/// The ST literal for a value edited in the inspector, checked against the setting's metadata.
pub(in crate::handlers::commands) fn literal_for(
    index: &Declarations,
    block: &str,
    setting: &HmiSetting,
    value: &Value,
) -> Result<String, String> {
    let kind = kind_of(index, block, &setting.member);
    let title = &setting.title;
    if !setting.choices.is_empty() {
        let choice = setting
            .choices
            .iter()
            .find(|choice| serde_json::to_value(&choice.value).ok().as_ref() == Some(value))
            .ok_or_else(|| format!("{title}: choose one of the listed values"))?;
        return Ok(scalar_literal(kind, &choice.value));
    }
    let within = |number: f64| -> Result<(), String> {
        if !number.is_finite() {
            return Err(format!("{title} needs a number"));
        }
        if setting.min.is_some_and(|min| number < min)
            || setting.max.is_some_and(|max| number > max)
        {
            let unit = setting
                .unit
                .as_deref()
                .filter(|unit| !unit.starts_with('$'))
                .unwrap_or("");
            return Err(match (setting.min, setting.max) {
                (Some(min), Some(max)) => format!("{title} is between {min} and {max} {unit}"),
                (Some(min), None) => format!("{title} is at least {min} {unit}"),
                (None, Some(max)) => format!("{title} is at most {max} {unit}"),
                (None, None) => format!("{title} is out of range"),
            }
            .trim_end()
            .to_string());
        }
        Ok(())
    };
    match kind {
        Kind::Bool => value
            .as_bool()
            .map(|flag| if flag { "TRUE" } else { "FALSE" }.to_string())
            .ok_or_else(|| format!("{title} is on or off")),
        Kind::Integer => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{title} needs a whole number"))?;
            within(number)?;
            if number.fract() != 0.0 {
                return Err(format!("{title} needs a whole number"));
            }
            Ok(format!("{}", number as i64))
        }
        Kind::Real => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{title} needs a number"))?;
            within(number)?;
            Ok(real_literal(number))
        }
        Kind::Time => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{title} needs a number"))?;
            within(number)?;
            if number < 0.0 {
                return Err(format!("{title} cannot be negative"));
            }
            let nanos = (number * time_unit(setting).1).round() as i128;
            Ok(time_literal(nanos))
        }
        Kind::Text => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{title} needs a text"))?;
            if text.contains('\n') {
                return Err(format!("{title} is one line"));
            }
            Ok(st_string(text))
        }
        Kind::Other => Err(format!("{title} cannot be edited in the inspector")),
    }
}

/// A real literal with a decimal point, as ST requires.
fn real_literal(number: f64) -> String {
    if number.fract() == 0.0 && number.abs() < 1e15 {
        format!("{number:.1}")
    } else {
        format!("{number}")
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::LoadedSource;
    use super::*;

    const SOURCE: &str = "FUNCTION_BLOCK Base\nVAR PUBLIC RETAIN\n    StartTimeout : TIME := T#5s;\n    Gain : LREAL := 1.5;\nEND_VAR\nVAR PUBLIC\n    Unit : STRING[16] := 'bar';\nEND_VAR\nEND_FUNCTION_BLOCK\nFUNCTION_BLOCK Pump EXTENDS Base\nVAR_INPUT\n    HasFeedback : BOOL := TRUE;\nEND_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR\n    P101 : Pump := (StartTimeout := T#10s, HasFeedback := FALSE);\n    P102 : Pump;\nEND_VAR\nEND_PROGRAM\n";

    fn index() -> std::sync::Arc<Declarations> {
        Declarations::read(&[LoadedSource {
            path: "/equipment/main.st".into(),
            text: SOURCE.into(),
        }])
    }

    fn setting(member: &str, unit: Option<&str>) -> HmiSetting {
        HmiSetting {
            member: member.into(),
            title: member.into(),
            unit: unit.map(Into::into),
            min: Some(1.0),
            max: Some(600.0),
            step: None,
            choices: Vec::new(),
            key: true,
            help: None,
        }
    }

    #[test]
    fn durations_convert_exactly_both_ways() {
        assert_eq!(duration_nanos("T#1h30m"), Some(5_400_000_000_000));
        assert_eq!(duration_nanos("TIME#2.5s"), Some(2_500_000_000));
        assert_eq!(duration_nanos("LT#10ms"), Some(10_000_000));
        assert_eq!(duration_nanos("T#5"), None);
        assert_eq!(time_literal(10_000_000_000), "T#10s");
        assert_eq!(time_literal(90_000_000_000), "T#90s");
        assert_eq!(time_literal(120_000_000_000), "T#2m");
        assert_eq!(time_literal(1_500_000), "T#1500us");
    }

    #[test]
    fn the_inspector_shows_the_declaration_then_the_inherited_default() {
        let index = index();
        let settings = [setting("StartTimeout", Some("s"))];
        let none = BTreeMap::new();
        let own = describe(index.as_ref(), "Main.P101", "Pump", &settings, None, &none);
        assert_eq!(own[0]["value"], json!(10.0), "P101's declaration sets 10 s");
        assert_eq!(own[0]["source"], "declaration");
        let inherited = describe(index.as_ref(), "Main.P102", "Pump", &settings, None, &none);
        assert_eq!(inherited[0]["value"], json!(5.0), "Base declares 5 s");
        assert_eq!(inherited[0]["source"], "default");
        let design = DesignEntry {
            block: "Pump".into(),
            settings: BTreeMap::from([("StartTimeout".to_string(), "T#20s".to_string())]),
            ..DesignEntry::default()
        };
        let chosen = describe(
            index.as_ref(),
            "Main.P102",
            "Pump",
            &settings,
            Some(&design),
            &none,
        );
        assert_eq!(chosen[0]["value"], json!(20.0), "the design's choice wins");
    }

    #[test]
    fn an_edited_value_becomes_a_checked_literal() {
        let index = index();
        let timeout = setting("StartTimeout", Some("s"));
        assert_eq!(
            literal_for(index.as_ref(), "Pump", &timeout, &json!(12)).unwrap(),
            "T#12s"
        );
        assert!(literal_for(index.as_ref(), "Pump", &timeout, &json!(601)).is_err());
        let gain = HmiSetting {
            min: None,
            max: None,
            ..setting("Gain", None)
        };
        assert_eq!(
            literal_for(index.as_ref(), "Pump", &gain, &json!(2)).unwrap(),
            "2.0"
        );
    }

    #[test]
    fn an_instance_declaration_lists_its_initial_values() {
        let index = index();
        let values = instance_initializers(index.as_ref(), "Main.P101");
        assert_eq!(values["StartTimeout"], "T#10s");
        assert_eq!(values["HasFeedback"], "FALSE");
        assert!(instance_initializers(index.as_ref(), "Main.P102").is_empty());
    }
}
