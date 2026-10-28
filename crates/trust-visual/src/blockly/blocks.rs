//! Blockly blocks as Structured Text (specification 25 §11.4).
//!
//! The conversion reads the workspace as the Blockly editor saved it: the declared variables
//! (`variables[]`, with `id`, `name` and `type`), and the top-level block chains
//! (`blocks.blocks[]`). A block names its kind in `type`, its settings in `fields`, the blocks
//! plugged into it in `inputs` (`{ "NAME": { "block": {…} } }`) and the block after it in `next`,
//! either inline (`{ "block": {…} }`) or by id (`"next": "<id>"`).
//!
//! Each chain becomes Structured Text statements in order: `controls_if` an `IF`, the pre-tested
//! `controls_whileUntil` a `WHILE` (its `UNTIL` mode negates the condition), `variables_set` an
//! assignment and `io_digital_write` an assignment to a direct address; the value blocks
//! (`logic_compare`, `math_arithmetic`, `variables_get`, `io_digital_read`, `logic_boolean`,
//! `text`, `math_number`) become expressions. A declared variable without a type takes the type
//! of the first value assigned to it (`BOOL` for a comparison, a Boolean or an input, `REAL` for a
//! number with a decimal point, `INT` for other numbers and for arithmetic), and `BOOL` when
//! nothing gives it one. An unknown block becomes a comment and a warning; a chain that leads back
//! to itself stops with a warning.

use std::collections::{HashMap, HashSet};

use crate::js;
use crate::js::trim;
use crate::json::number_text;
use crate::naming::{comment_text, sanitize_identifier, st_string};
use crate::{Json, Problem};

/// How deeply blocks may nest before the conversion stops with a warning. Inline blocks cannot
/// nest deeper than the JSON reader allows; only chains joined by `next` ids can.
const MAX_NESTING: usize = 256;

/// The comparison operators of `logic_compare`, by the `OP` field.
const COMPARE_OPERATORS: [(&str, &str); 6] = [
    ("EQ", "="),
    ("NEQ", "<>"),
    ("LT", "<"),
    ("LTE", "<="),
    ("GT", ">"),
    ("GTE", ">="),
];

/// The arithmetic operators of `math_arithmetic`, by the `OP` field.
const ARITHMETIC_OPERATORS: [(&str, &str); 5] = [
    ("ADD", "+"),
    ("MINUS", "-"),
    ("MULTIPLY", "*"),
    ("DIVIDE", "/"),
    ("POWER", "**"),
];

/// The Structured Text of a workspace.
pub(super) struct Conversion {
    /// The variables in declaration order with their types.
    pub variables: Vec<(String, String)>,
    /// The statements of the top-level chains, in order; a statement may span several lines.
    pub body: Vec<String>,
    /// The blocks the conversion could not express.
    pub errors: Vec<String>,
}

/// Converts the blocks of `workspace` (whose `blocks.blocks` is a list).
///
/// # Errors
/// The `variables` member is present but no list, or a variable entry is `null`.
pub(super) fn convert(workspace: &Json) -> Result<Conversion, Problem> {
    let mut converter = Converter::default();
    converter.declare(workspace.get("variables"))?;
    let roots: &[Json] = workspace
        .get("blocks")
        .and_then(|blocks| blocks.get("blocks"))
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice);
    for root in roots {
        converter.register(root);
    }
    for root in roots {
        converter.infer(root);
    }
    for (_, ty) in &mut converter.variables {
        if ty.is_empty() {
            "BOOL".clone_into(ty);
        }
    }
    let mut body = Vec::new();
    for root in roots {
        body.extend(converter.statement_chain(root));
    }
    Ok(Conversion {
        variables: converter.variables,
        body,
        errors: converter.errors,
    })
}

/// A block id as a key: ids compare as JavaScript `Map` keys do (text with text, numbers with
/// numbers, an object only with itself, and a missing id with every other missing id).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum IdKey {
    Missing,
    Value(String),
}

fn id_key(block: &Json) -> IdKey {
    match block.get("id") {
        None => IdKey::Missing,
        Some(Json::String(text)) => IdKey::Value(format!("s:{text}")),
        Some(Json::Number(number)) => IdKey::Value(format!("n:{}", number_text(*number))),
        Some(Json::Bool(value)) => IdKey::Value(format!("b:{value}")),
        Some(Json::Null) => IdKey::Value("null".to_owned()),
        Some(other) => IdKey::Value(format!("p:{:p}", std::ptr::from_ref(other))),
    }
}

/// The field `name` of a block (`block.fields?.[name]`).
fn field<'a>(block: &'a Json, name: &str) -> Option<&'a Json> {
    block.get("fields").and_then(|fields| fields.get(name))
}

/// The text of the field `name`, or `default` when the field is absent or falsy.
fn field_text(block: &Json, name: &str, default: &str) -> String {
    let value = field(block, name);
    if js::truthy(value) {
        js::string(value)
    } else {
        default.to_owned()
    }
}

/// The values plugged into a block's inputs, in input order (`Object.values(block.inputs)`).
fn input_values(block: &Json) -> Vec<&Json> {
    match block.get("inputs") {
        Some(Json::Object(members)) => js::ordered(members)
            .into_iter()
            .map(|(_, value)| value)
            .collect(),
        Some(Json::Array(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

/// A value that is a block: an object with a text `id` and a text `type`.
fn as_block(value: Option<&Json>) -> Option<&Json> {
    value.filter(|candidate| {
        matches!(candidate.get("id"), Some(Json::String(_)))
            && matches!(candidate.get("type"), Some(Json::String(_)))
    })
}

/// The block plugged into an input value `{ "block": {…} }`.
fn input_block(input: &Json) -> Option<&Json> {
    as_block(input.get("block"))
}

/// The block plugged into the first of the inputs `names` that holds one.
fn named_input_block<'a>(block: &'a Json, names: &[&str]) -> Option<&'a Json> {
    let inputs = block.get("inputs")?;
    names
        .iter()
        .find_map(|name| inputs.get(name).and_then(input_block))
}

/// The value the input `name` plugs in, without checking that it is a block
/// (`block.inputs?.[name]?.block`, when truthy).
fn plugged_value<'a>(block: &'a Json, name: &str) -> Option<&'a Json> {
    block
        .get("inputs")
        .and_then(|inputs| inputs.get(name))
        .and_then(|input| input.get("block"))
        .filter(|value| js::truthy(Some(value)))
}

/// The statements after a line break, every line indented by two spaces; nothing without
/// statements.
fn indented(statements: &[String]) -> String {
    if statements.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = statements
        .iter()
        .flat_map(|statement| statement.split('\n'))
        .map(|line| format!("  {line}"))
        .collect();
    format!("\n{}", lines.join("\n"))
}

/// The symbol of the operator `op`; an unknown operator is the default's.
fn operator_symbol(operators: &[(&str, &'static str)], op: &str, default: &str) -> &'static str {
    operators
        .iter()
        .find(|(name, _)| *name == op)
        .or_else(|| operators.iter().find(|(name, _)| *name == default))
        .map_or("", |(_, symbol)| *symbol)
}

#[derive(Default)]
struct Converter<'a> {
    /// Every block reachable from the roots, by id; the first block with an id is kept.
    blocks_by_id: HashMap<IdKey, &'a Json>,
    /// The variables in declaration order with their types (empty while unknown).
    variables: Vec<(String, String)>,
    /// The place of each variable in `variables`.
    variable_index: HashMap<String, usize>,
    /// The declared name of each variable id.
    names_by_id: HashMap<String, Option<&'a Json>>,
    /// The blocks being converted, to stop a chain that leads back into the block holding it.
    converting: HashSet<*const Json>,
    errors: Vec<String>,
}

impl<'a> Converter<'a> {
    /// Declares the workspace's variables in order; a repeated name keeps its place and takes
    /// the later type.
    fn declare(&mut self, variables: Option<&'a Json>) -> Result<(), Problem> {
        if !js::truthy(variables) {
            return Ok(());
        }
        let Some(variables) = variables.and_then(Json::as_array) else {
            return Err(Problem::new("The Blockly variables must be a list."));
        };
        for variable in variables {
            if matches!(variable, Json::Null) {
                return Err(Problem::new("A Blockly variable entry is null."));
            }
            let ty = variable.get("type");
            let ty = if js::truthy(ty) {
                js::string(ty)
            } else {
                String::new()
            };
            self.set_variable(js::string(variable.get("name")), ty);
            if let Some(Json::String(id)) = variable.get("id") {
                self.names_by_id.insert(id.clone(), variable.get("name"));
            }
        }
        Ok(())
    }

    fn set_variable(&mut self, name: String, ty: String) {
        if let Some(index) = self.variable_index.get(&name) {
            self.variables[*index].1 = ty;
        } else {
            self.variable_index
                .insert(name.clone(), self.variables.len());
            self.variables.push((name, ty));
        }
    }

    fn variable_type(&self, name: &str) -> Option<&str> {
        self.variable_index
            .get(name)
            .map(|index| self.variables[*index].1.as_str())
    }

    /// Registers `root` and the blocks inside and after it by id, in that order; the first block
    /// of an id is kept, and the blocks of a block already registered are not visited again.
    fn register(&mut self, root: &'a Json) {
        let mut pending = vec![root];
        while let Some(block) = pending.pop() {
            if !js::truthy(Some(block)) {
                continue;
            }
            let key = id_key(block);
            if self.blocks_by_id.contains_key(&key) {
                continue;
            }
            self.blocks_by_id.insert(key, block);
            if let Some(next) = self.next_block(block) {
                pending.push(next);
            }
            for input in input_values(block).into_iter().rev() {
                if let Some(nested) = input_block(input) {
                    pending.push(nested);
                }
            }
        }
    }

    /// The block after `block`: inline, or the block registered under the id `next` names.
    fn next_block(&self, block: &'a Json) -> Option<&'a Json> {
        match block.get("next") {
            Some(Json::String(id)) if !id.is_empty() => self
                .blocks_by_id
                .get(&IdKey::Value(format!("s:{id}")))
                .copied(),
            Some(next @ Json::Object(_)) => as_block(next.get("block")),
            _ => None,
        }
    }

    /// The name a `VAR` field stands for: the text itself, the variable's `name`, the declared
    /// name of its `id` (or the id made an identifier), and `temp` otherwise.
    fn variable_name(&self, field: Option<&Json>) -> String {
        match field {
            Some(Json::String(text)) if !trim(text).is_empty() => text.clone(),
            Some(candidate @ Json::Object(_)) => {
                if let Some(Json::String(name)) = candidate.get("name")
                    && !trim(name).is_empty()
                {
                    return name.clone();
                }
                if let Some(Json::String(id)) = candidate.get("id")
                    && !trim(id).is_empty()
                {
                    return match self.names_by_id.get(id) {
                        Some(name) if js::truthy(*name) => js::string(*name),
                        _ => sanitize_identifier(id, "temp"),
                    };
                }
                "temp".to_owned()
            }
            _ => "temp".to_owned(),
        }
    }

    /// Gives untyped variables the type of the values assigned to them, walking `root`, the
    /// blocks inside it and the blocks after it in that order. A block met again on its own path
    /// (a chain that leads back to itself) is not walked again.
    fn infer(&mut self, root: &'a Json) {
        enum Step<'b> {
            Enter(&'b Json),
            Leave(*const Json),
        }
        let mut path: HashSet<*const Json> = HashSet::new();
        let mut steps = vec![Step::Enter(root)];
        while let Some(step) = steps.pop() {
            let block = match step {
                Step::Leave(block) => {
                    path.remove(&block);
                    continue;
                }
                Step::Enter(block) => block,
            };
            if !js::truthy(Some(block)) {
                continue;
            }
            let identity = std::ptr::from_ref(block);
            if !path.insert(identity) {
                continue;
            }
            if block.str_at("type") == Some("variables_set") {
                let name = self.variable_name(field(block, "VAR"));
                let inferred = self.expression_type(named_input_block(block, &["VALUE"]));
                if let Some(ty) = inferred
                    && self.variable_type(&name).is_none_or(str::is_empty)
                {
                    self.set_variable(name, ty);
                }
            }
            steps.push(Step::Leave(identity));
            if let Some(next) = self.next_block(block) {
                steps.push(Step::Enter(next));
            }
            for input in input_values(block).into_iter().rev() {
                if let Some(nested) = input_block(input) {
                    steps.push(Step::Enter(nested));
                }
            }
        }
    }

    /// The type a value block gives a variable it is assigned to.
    fn expression_type(&self, block: Option<&Json>) -> Option<String> {
        let block = block?;
        let ty = match block.str_at("type")? {
            "logic_boolean" | "logic_compare" | "io_digital_read" => "BOOL",
            "math_number" => {
                let value = match field(block, "NUM") {
                    None | Some(Json::Null) => "0".to_owned(),
                    value => js::string(value),
                };
                if value.contains('.') {
                    "REAL"
                } else {
                    "INT"
                }
            }
            "math_arithmetic" => {
                let left = self.expression_type(named_input_block(block, &["A"]));
                let right = self.expression_type(named_input_block(block, &["B"]));
                if left.as_deref() == Some("REAL") || right.as_deref() == Some("REAL") {
                    "REAL"
                } else {
                    "INT"
                }
            }
            "variables_get" => {
                let name = self.variable_name(field(block, "VAR"));
                return self
                    .variable_type(&name)
                    .filter(|ty| !ty.is_empty())
                    .map(str::to_owned);
            }
            _ => return None,
        };
        Some(ty.to_owned())
    }

    /// The statements of the chain that starts at `start`, in order. A block whose id the chain
    /// already passed stops the chain with a warning.
    fn statement_chain(&mut self, start: &'a Json) -> Vec<String> {
        let mut lines = Vec::new();
        let mut visited: HashSet<IdKey> = HashSet::new();
        let mut current = Some(start);
        while let Some(block) = current {
            if !js::truthy(Some(block)) {
                break;
            }
            if !visited.insert(id_key(block)) {
                self.errors.push(format!(
                    "Detected cycle while generating block chain at: {}",
                    js::string(block.get("id"))
                ));
                break;
            }
            let code = self.block_code(block);
            if !code.is_empty() {
                lines.push(code);
            }
            current = self.next_block(block);
        }
        lines
    }

    /// The Structured Text of one block. A block met again while it is being converted (a chain
    /// that leads back into the block holding it) is a warning and no code.
    fn block_code(&mut self, block: &'a Json) -> String {
        let identity = std::ptr::from_ref(block);
        if self.converting.contains(&identity) {
            self.errors.push(format!(
                "Detected cycle while generating block chain at: {}",
                js::string(block.get("id"))
            ));
            return String::new();
        }
        if self.converting.len() >= MAX_NESTING {
            self.errors.push(format!(
                "Blocks are nested too deeply at: {}",
                js::string(block.get("id"))
            ));
            return String::new();
        }
        self.converting.insert(identity);
        let code = self.code_of(block);
        self.converting.remove(&identity);
        code
    }

    fn code_of(&mut self, block: &'a Json) -> String {
        let Some(kind) = block.str_at("type") else {
            return self.unknown(block);
        };
        match kind {
            "controls_if" => self.if_statement(block),
            "controls_whileUntil" => self.while_statement(block),
            "logic_compare" => self.binary(block, &COMPARE_OPERATORS, "EQ"),
            "math_arithmetic" => self.binary(block, &ARITHMETIC_OPERATORS, "ADD"),
            "variables_set" => {
                let name = self.variable_name(field(block, "VAR"));
                let value = self.plugged_code(block, "VALUE", "0");
                format!("{name} := {value};")
            }
            "variables_get" => self.variable_name(field(block, "VAR")),
            "io_digital_write" => {
                let address = field_text(block, "ADDRESS", "%QX0.0");
                let value = self.plugged_code(block, "VALUE", "FALSE");
                format!("{address} := {value};")
            }
            "io_digital_read" => field_text(block, "ADDRESS", "%IX0.0"),
            "logic_boolean" => field_text(block, "BOOL", "TRUE"),
            "text" => st_string(&field_text(block, "TEXT", "")),
            "math_number" => field_text(block, "NUM", "0"),
            _ => self.unknown(block),
        }
    }

    fn unknown(&mut self, block: &Json) -> String {
        let kind = js::string(block.get("type"));
        self.errors.push(format!("Unknown block type: {kind}"));
        format!("(* Unknown block: {} *)", comment_text(&kind))
    }

    /// The code of the value plugged into the input `name`, or `default`.
    fn plugged_code(&mut self, block: &'a Json, name: &str, default: &str) -> String {
        match plugged_value(block, name) {
            Some(value) => self.block_code(value),
            None => default.to_owned(),
        }
    }

    /// `(A <operator> B)`, the operator chosen by the `OP` field (`default` when absent, the
    /// default's symbol when unknown).
    fn binary(
        &mut self,
        block: &'a Json,
        operators: &[(&str, &'static str)],
        default: &str,
    ) -> String {
        let op = field(block, "OP");
        let op = if js::truthy(op) {
            js::string(op)
        } else {
            default.to_owned()
        };
        let symbol = operator_symbol(operators, &op, default);
        let left = self.plugged_code(block, "A", "0");
        let right = self.plugged_code(block, "B", "0");
        format!("({left} {symbol} {right})")
    }

    fn if_statement(&mut self, block: &'a Json) -> String {
        let condition = match named_input_block(block, &["IF0", "IF"]) {
            Some(condition) => self.block_code(condition),
            None => "FALSE".to_owned(),
        };
        let statements = match named_input_block(block, &["DO0", "DO"]) {
            Some(first) => self.statement_chain(first),
            None => Vec::new(),
        };
        format!("IF {condition} THEN{}\nEND_IF;", indented(&statements))
    }

    /// A pre-tested loop: Blockly's `UNTIL` mode is also tested before each pass, so it is a
    /// `WHILE` with the condition negated (not a `REPEAT`).
    fn while_statement(&mut self, block: &'a Json) -> String {
        let condition = match named_input_block(block, &["BOOL"]) {
            Some(condition) => self.block_code(condition),
            None => "FALSE".to_owned(),
        };
        let mode = field(block, "MODE");
        let mode = if js::truthy(mode) {
            js::string(mode).to_uppercase()
        } else {
            "WHILE".to_owned()
        };
        let condition = if mode == "UNTIL" {
            format!("NOT ({condition})")
        } else {
            condition
        };
        let statements = match named_input_block(block, &["DO"]) {
            Some(first) => self.statement_chain(first),
            None => Vec::new(),
        };
        format!("WHILE {condition} DO{}\nEND_WHILE;", indented(&statements))
    }
}
