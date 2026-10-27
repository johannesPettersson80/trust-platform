//! The Structured Text companion of a Ladder program (specification 16 §4 and §5, specification 17
//! §3).
//!
//! The companion is one `FUNCTION_BLOCK`. Its `VAR_EXTERNAL` names the program's globals and one
//! global per direct address the networks name; its `VAR` holds the locals, one instance per timer
//! and counter, and one result per compare block. Each network becomes, in ascending `order`, a
//! comment and its statements: a coil assigns or latches the power reaching it, a timer or counter
//! is called with that power (or its own input) and copies its outputs, a compare block stores its
//! comparison and a math block its result while power reaches them. Division is plain Structured
//! Text division, so a zero divisor faults as it does in Structured Text (specification 16 §4).
//!
//! Operands are symbols, qualified symbols (`LOCAL::Name`, `GLOBAL::Name`, …) or direct
//! addresses. Every symbol must be declared; an undeclared one makes the program untranslatable.

use crate::direct_io::{address_type, alias_name, direct_address, refused_address};
use crate::js;
use crate::json::{js_round, number_text};
use crate::ladder::power::{incoming_power, sorted_nodes};
use crate::ladder::references::{self, Scope};
use crate::naming::{
    comment_text, fb_name_for_source, is_assignable_reference, is_simple_identifier, local_name,
};
use crate::{Generated, Global, Json, Problem};

const INDENT: &str = "  ";

/// One global the runtime entry declares for a Ladder program.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeGlobal<'a> {
    pub name: String,
    pub ty: String,
    pub address: Option<String>,
    pub initial: Option<&'a Json>,
}

/// The program's globals (scope other than `local`, identifier names, first of each name), then
/// one global per direct address the networks name that no global is declared `AT`, named
/// `ld_io_<address>` (with `_2`, `_3`, … when that name is taken) and typed by its size
/// (specification 03 §5).
pub(crate) fn runtime_globals(program: &Json) -> Vec<RuntimeGlobal<'_>> {
    let mut globals: Vec<RuntimeGlobal<'_>> = Vec::new();
    for variable in array(program, "variables") {
        if variable.str_at("scope") == Some("local") {
            continue;
        }
        let name = js::trim(variable.str_at("name").unwrap_or_default());
        if !is_simple_identifier(name) || globals.iter().any(|global| global.name == name) {
            continue;
        }
        globals.push(RuntimeGlobal {
            name: name.to_owned(),
            ty: variable.str_at("type").unwrap_or("BOOL").to_owned(),
            address: variable.str_at("address").and_then(direct_address),
            initial: variable.get("initialValue"),
        });
    }
    for address in addresses_used(program) {
        if globals
            .iter()
            .any(|global| global.address.as_deref() == Some(address.as_str()))
        {
            continue;
        }
        let base = alias_name(&address);
        let mut name = base.clone();
        let mut suffix = 2;
        while globals.iter().any(|global| global.name == name) {
            name = format!("{base}_{suffix}");
            suffix += 1;
        }
        globals.push(RuntimeGlobal {
            name,
            ty: address_type(&address).to_owned(),
            address: Some(address),
            initial: None,
        });
    }
    globals
}

/// The operand members of each node kind, in the order they are read.
fn operand_members(kind: &str) -> &'static [&'static str] {
    match kind {
        "contact" | "coil" => &["variable"],
        "timer" => &["input", "qOutput", "etOutput"],
        "counter" => &["input", "qOutput", "cvOutput"],
        "compare" => &["left", "right"],
        "math" => &["left", "right", "output"],
        _ => &[],
    }
}

/// The distinct direct addresses the nodes name, canonical, in file order.
fn addresses_used(program: &Json) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for network in array(program, "networks") {
        for node in array(network, "nodes") {
            for member in operand_members(node.str_at("type").unwrap_or_default()) {
                if let Some(address) = node.str_at(member).and_then(direct_address)
                    && !found.contains(&address)
                {
                    found.push(address);
                }
            }
        }
    }
    found
}

/// The Structured Text literal of an initial value of a variable of type `ty`, or `None` when the
/// value does not fit the type. Numbers in text are read as JavaScript's `Number` reads them.
pub(crate) fn initial_literal(ty: &str, value: Option<&Json>) -> Option<String> {
    let value = value?;
    let number = || match value {
        Json::Number(number) => Some(*number),
        Json::String(text) if !js::trim(text).is_empty() => js::number_from_text(text),
        _ => None,
    };
    match ty.trim().to_ascii_uppercase().as_str() {
        "BOOL" => match value {
            Json::Bool(flag) => Some(flag_literal(*flag)),
            Json::Number(number) => Some(flag_literal(*number != 0.0)),
            Json::String(text) => match js::trim(text).to_uppercase().as_str() {
                "TRUE" | "1" => Some(flag_literal(true)),
                "FALSE" | "0" => Some(flag_literal(false)),
                _ => None,
            },
            _ => None,
        },
        "INT" | "DINT" => number()
            .filter(|number| number.is_finite())
            .map(|number| number_text(number.trunc())),
        "REAL" | "LREAL" => number()
            .filter(|number| number.is_finite())
            .map(number_text),
        "TIME" => match value {
            Json::String(text) if !js::trim(text).is_empty() => Some(js::trim(text).to_owned()),
            Json::Number(number) if number.is_finite() => {
                Some(format!("T#{}ms", number_text(number.trunc().max(0.0))))
            }
            _ => None,
        },
        _ => None,
    }
}

fn flag_literal(flag: bool) -> String {
    if flag { "TRUE" } else { "FALSE" }.to_owned()
}

/// The items of the array member `key`, or none.
pub(crate) fn array<'a>(value: &'a Json, key: &str) -> &'a [Json] {
    value
        .get(key)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// A local of the companion's `VAR` section.
struct Local {
    name: String,
    ty: String,
    initial: Option<String>,
    comment: String,
}

/// A timer or counter instance of the companion's `VAR` section.
struct Instance {
    name: String,
    block: String,
    source: String,
}

/// The state of one lowering.
struct Lowering<'a> {
    locals: Vec<Local>,
    local_names: Vec<String>,
    externals: Vec<(String, String)>,
    aliases: Vec<(String, String)>,
    declared: Vec<String>,
    timers: Vec<Instance>,
    counters: Vec<Instance>,
    problems: Vec<Problem>,
    warnings: Vec<String>,
    lines: Vec<String>,
    globals: Vec<RuntimeGlobal<'a>>,
}

/// The companion function block of the Ladder program `program` saved as
/// `<base_name>.ladder.json`, named `FB_<base>_LADDER` (`metadata.name` when there is no base
/// name), and the globals of its runtime entry.
///
/// # Errors
/// The program breaks the schema (specification 16 §2), names an undeclared symbol, an address
/// the runtime refuses, or an output that cannot be written.
pub fn generate(program: &Json, base_name: &str) -> Result<Generated, Vec<Problem>> {
    super::schema::validate(program).map_err(|message| vec![Problem::new(message)])?;
    let mut lowering = Lowering::new(program);
    let mut networks: Vec<&Json> = array(program, "networks").iter().collect();
    networks.sort_by(|left, right| {
        let order = |network: &Json| network.get("order").and_then(Json::as_f64).unwrap_or(0.0);
        order(left)
            .partial_cmp(&order(right))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for network in networks {
        lowering.network(network);
    }
    if !lowering.problems.is_empty() {
        return Err(lowering.problems);
    }
    let source = if base_name.is_empty() {
        program
            .get("metadata")
            .and_then(|metadata| metadata.str_at("name"))
            .unwrap_or_default()
    } else {
        base_name
    };
    let function_block = lowering.text(&fb_name_for_source(source, "LADDER"));
    let globals = lowering
        .globals
        .iter()
        .map(|global| Global {
            name: global.name.clone(),
            address: global.address.clone(),
            ty: global.ty.clone(),
            initial: initial_literal(&global.ty, global.initial),
        })
        .collect();
    Ok(Generated {
        function_block,
        globals,
        warnings: lowering.warnings,
    })
}

impl<'a> Lowering<'a> {
    fn new(program: &'a Json) -> Self {
        let mut lowering = Self {
            locals: Vec::new(),
            local_names: Vec::new(),
            externals: Vec::new(),
            aliases: Vec::new(),
            declared: Vec::new(),
            timers: Vec::new(),
            counters: Vec::new(),
            problems: Vec::new(),
            warnings: Vec::new(),
            lines: Vec::new(),
            globals: runtime_globals(program),
        };
        for variable in array(program, "variables") {
            let name = js::trim(variable.str_at("name").unwrap_or_default());
            if !is_simple_identifier(name) || variable.str_at("scope") != Some("local") {
                continue;
            }
            if !lowering.local_names.iter().any(|known| known == name) {
                lowering.local_names.push(name.to_owned());
                lowering.declared.push(name.to_owned());
            }
            if lowering.locals.iter().any(|local| local.name == name) {
                continue;
            }
            let ty = variable.str_at("type").unwrap_or("BOOL").to_owned();
            lowering.locals.push(Local {
                name: name.to_owned(),
                initial: initial_literal(&ty, variable.get("initialValue")),
                ty,
                comment: "Ladder local".to_owned(),
            });
        }
        for global in &lowering.globals {
            if lowering.local_names.contains(&global.name)
                || lowering
                    .externals
                    .iter()
                    .any(|(name, _)| *name == global.name)
            {
                continue;
            }
            lowering
                .externals
                .push((global.name.clone(), global.ty.clone()));
            lowering.declared.push(global.name.clone());
            if let Some(address) = &global.address {
                lowering
                    .aliases
                    .push((address.clone(), global.name.clone()));
            }
        }
        for variable in array(program, "variables") {
            if let Some(problem) = variable.str_at("address").and_then(refused_address) {
                let name = variable.str_at("name").unwrap_or_default();
                lowering
                    .problems
                    .push(Problem::new(format!("Variable '{name}': {problem}.")));
            }
        }
        lowering
    }

    /// The Structured Text an operand stands for: a direct address becomes its global, a
    /// qualified symbol its plain name, anything else is the operand trimmed.
    fn symbol(&mut self, operand: &str, element: &str) -> String {
        if let Some(address) = direct_address(operand) {
            return self
                .aliases
                .iter()
                .find(|(known, _)| *known == address)
                .map_or_else(|| js::trim(operand).to_owned(), |(_, name)| name.clone());
        }
        if let Some(problem) = refused_address(operand) {
            self.problems
                .push(Problem::at(element, format!("{problem}.")));
            return js::trim(operand).to_owned();
        }
        if let Some((scope, name)) = references::qualified(operand) {
            let is_local = self.local_names.iter().any(|local| local == name);
            let is_global = self.externals.iter().any(|(global, _)| global == name);
            let problem = match scope {
                Scope::Local if !is_local => Some(format!(
                    "'{}' names no local variable '{name}'.",
                    js::trim(operand)
                )),
                Scope::Global if is_local => Some(format!(
                    "'{}' cannot be reached: the local variable '{name}' hides the global.",
                    js::trim(operand)
                )),
                Scope::Global if !is_global => Some(format!(
                    "'{}' names no global variable '{name}'.",
                    js::trim(operand)
                )),
                _ => None,
            };
            if let Some(problem) = problem {
                self.problems.push(Problem::at(element, problem));
                return js::trim(operand).to_owned();
            }
            return name.to_owned();
        }
        js::trim(operand).to_owned()
    }

    /// Records a problem when `symbol` is an identifier no declaration names. `TRUE` and `FALSE`
    /// are literals, not symbols: a block on the left rail gets the power `TRUE`.
    fn require_declared(&mut self, symbol: &str, context: &str, element: &str) {
        let symbol = js::trim(symbol);
        let literal = symbol.eq_ignore_ascii_case("TRUE") || symbol.eq_ignore_ascii_case("FALSE");
        if !literal
            && is_simple_identifier(symbol)
            && !self.declared.iter().any(|known| known == symbol)
        {
            self.problems.push(Problem::at(
                element,
                format!(
                    "Undeclared ladder symbol '{symbol}' referenced by {context}. Declare it in variables[] or use a direct address."
                ),
            ));
        }
    }

    /// The target an output operand writes, or a problem when it cannot be written.
    fn output(&mut self, operand: &str, context: &str, element: &str) -> String {
        if !is_assignable_reference(operand) && references::qualified(operand).is_none() {
            self.problems.push(Problem::at(
                element,
                format!(
                    "{context} '{}' is not a variable or direct address that can be written.",
                    js::trim(operand)
                ),
            ));
            return js::trim(operand).to_owned();
        }
        let target = self.symbol(operand, element);
        self.require_declared(&target, context, element);
        target
    }

    fn ensure_local(&mut self, name: String, ty: &str, comment: String) -> String {
        if !self.locals.iter().any(|local| local.name == name) {
            self.locals.push(Local {
                name: name.clone(),
                ty: ty.to_owned(),
                initial: None,
                comment,
            });
        }
        name
    }

    /// The instance variable of a timer or counter, declared once per instance name (or id).
    fn instance(&mut self, node: &Json, timer: bool) -> String {
        let id = node.str_at("id").unwrap_or_default();
        let instance = js::trim(node.str_at("instance").unwrap_or_default());
        let key = if instance.is_empty() { id } else { instance };
        let (prefix, list, kind_member) = if timer {
            ("ld_timer", &mut self.timers, "timerType")
        } else {
            ("ld_counter", &mut self.counters, "counterType")
        };
        if let Some(existing) = list.iter().find(|entry| entry.source == key) {
            return existing.name.clone();
        }
        let name = local_name(prefix, key);
        list.push(Instance {
            name: name.clone(),
            block: node.str_at(kind_member).unwrap_or_default().to_owned(),
            source: key.to_owned(),
        });
        name
    }

    fn network(&mut self, network: &Json) {
        let nodes = sorted_nodes(network);
        let power = {
            let aliases = &self.aliases;
            let contact_term = |node: &Json| {
                let operand = node.str_at("variable").unwrap_or_default();
                let term = resolve_quietly(operand, aliases);
                let term = if term.is_empty() {
                    "FALSE".to_owned()
                } else {
                    term
                };
                if node.str_at("contactType") == Some("NC") {
                    format!("NOT ({term})")
                } else {
                    term
                }
            };
            incoming_power(network, &contact_term)
        };
        let power_of = |id: &str| {
            power
                .iter()
                .find(|(node, _)| node == id)
                .map_or_else(|| "FALSE".to_owned(), |(_, term)| term.clone())
        };
        let order = network.get("order").and_then(Json::as_f64).unwrap_or(0.0);
        self.lines.push(format!(
            "(* Network {}: {} *)",
            number_text(order),
            comment_text(network.str_at("id").unwrap_or_default())
        ));
        let of_kind = |kind: &str| -> Vec<&Json> {
            nodes
                .iter()
                .copied()
                .filter(|node| node.str_at("type") == Some(kind))
                .collect()
        };
        for contact in of_kind("contact") {
            let id = contact.str_at("id").unwrap_or_default();
            let symbol = self.symbol(contact.str_at("variable").unwrap_or_default(), id);
            self.require_declared(&symbol, &format!("contact '{id}'"), id);
        }
        for coil in of_kind("coil") {
            self.coil(coil, &power_of(coil.str_at("id").unwrap_or_default()));
        }
        for timer in of_kind("timer") {
            self.timer(timer, &power_of(timer.str_at("id").unwrap_or_default()));
        }
        for counter in of_kind("counter") {
            self.counter(counter, &power_of(counter.str_at("id").unwrap_or_default()));
        }
        for compare in of_kind("compare") {
            self.compare(compare, &power_of(compare.str_at("id").unwrap_or_default()));
        }
        for math in of_kind("math") {
            self.math(math, &power_of(math.str_at("id").unwrap_or_default()));
        }
        self.lines.push(String::new());
    }

    fn coil(&mut self, coil: &Json, power: &str) {
        let id = coil.str_at("id").unwrap_or_default();
        let operand = coil.str_at("variable").unwrap_or_default();
        let target = self.symbol(operand, id);
        self.require_declared(&target, &format!("coil '{id}'"), id);
        if !is_assignable_reference(&target) {
            self.lines.push(format!(
                "(* Skipped coil '{}' due to non-assignable target: {} *)",
                comment_text(id),
                comment_text(operand)
            ));
            self.warnings.push(format!(
                "Coil '{id}' is skipped: '{}' cannot be written.",
                js::trim(operand)
            ));
            return;
        }
        match coil.str_at("coilType").unwrap_or_default() {
            "NEGATED" => self.lines.push(format!("{target} := NOT ({power});")),
            "SET" | "RESET" => {
                let value = if coil.str_at("coilType") == Some("SET") {
                    "TRUE"
                } else {
                    "FALSE"
                };
                self.lines.push(format!("IF {power} THEN"));
                self.lines.push(format!("{INDENT}{target} := {value};"));
                self.lines.push("END_IF;".to_owned());
            }
            _ => self.lines.push(format!("{target} := {power};")),
        }
    }

    /// The `IN`, `CU` or `CD` input of a block: its own input when it has one, else the power.
    fn block_input(&mut self, node: &Json, power: &str, context: &str) -> String {
        let id = node.str_at("id").unwrap_or_default();
        let own = node.str_at("input").unwrap_or_default();
        let input = if js::trim(own).is_empty() {
            power.to_owned()
        } else {
            self.symbol(own, id)
        };
        self.require_declared(&input, context, id);
        input
    }

    fn timer(&mut self, timer: &Json, power: &str) {
        let id = timer.str_at("id").unwrap_or_default();
        let variable = self.instance(timer, true);
        let input = self.block_input(timer, power, &format!("timer '{id}' input"));
        let preset = timer.get("presetMs").and_then(Json::as_f64).unwrap_or(0.0);
        let preset = number_text(js_round(preset).max(0.0));
        self.lines
            .push(format!("{variable}(IN := {input}, PT := T#{preset}ms);"));
        let q = self.output(
            timer.str_at("qOutput").unwrap_or_default(),
            &format!("timer '{id}' Q output"),
            id,
        );
        let et = self.output(
            timer.str_at("etOutput").unwrap_or_default(),
            &format!("timer '{id}' ET output"),
            id,
        );
        self.lines.push(format!("{q} := {variable}.Q;"));
        self.lines.push(format!("{et} := {variable}.ET;"));
    }

    fn counter(&mut self, counter: &Json, power: &str) {
        let id = counter.str_at("id").unwrap_or_default();
        let variable = self.instance(counter, false);
        let preset = counter.get("preset").and_then(Json::as_f64).unwrap_or(0.0);
        let preset = number_text(js_round(preset).max(0.0));
        let input = self.block_input(counter, power, &format!("counter '{id}' input"));
        let call = match counter.str_at("counterType").unwrap_or_default() {
            "CTD" => format!("{variable}(CD := {input}, LD := FALSE, PV := {preset});"),
            "CTUD" => format!(
                "{variable}(CU := {input}, CD := FALSE, R := FALSE, LD := FALSE, PV := {preset});"
            ),
            _ => format!("{variable}(CU := {input}, R := FALSE, PV := {preset});"),
        };
        self.lines.push(call);
        let q = self.output(
            counter.str_at("qOutput").unwrap_or_default(),
            &format!("counter '{id}' Q output"),
            id,
        );
        let cv = self.output(
            counter.str_at("cvOutput").unwrap_or_default(),
            &format!("counter '{id}' CV output"),
            id,
        );
        self.lines.push(format!("{q} := {variable}.Q;"));
        self.lines.push(format!("{cv} := {variable}.CV;"));
    }

    /// A compare or math operand: its Structured Text, `0` when it is blank.
    fn operand(&mut self, node: &Json, member: &str, context: &str) -> String {
        let id = node.str_at("id").unwrap_or_default();
        let symbol = self.symbol(node.str_at(member).unwrap_or_default(), id);
        let symbol = if symbol.is_empty() {
            "0".to_owned()
        } else {
            symbol
        };
        self.require_declared(&symbol, context, id);
        symbol
    }

    fn compare(&mut self, compare: &Json, power: &str) {
        let id = compare.str_at("id").unwrap_or_default();
        let left = self.operand(compare, "left", &format!("compare '{id}' left operand"));
        let right = self.operand(compare, "right", &format!("compare '{id}' right operand"));
        let operator = match compare.str_at("op").unwrap_or_default() {
            "LT" => "<",
            "EQ" => "=",
            _ => ">",
        };
        let target = self.ensure_local(
            local_name("ld_compare_q", id),
            "BOOL",
            format!("Compare result for {id}"),
        );
        self.lines.push(format!("IF {power} THEN"));
        self.lines
            .push(format!("{INDENT}{target} := ({left} {operator} {right});"));
        self.lines.push("END_IF;".to_owned());
    }

    fn math(&mut self, math: &Json, power: &str) {
        let id = math.str_at("id").unwrap_or_default();
        let left = self.operand(math, "left", &format!("math '{id}' left operand"));
        let right = self.operand(math, "right", &format!("math '{id}' right operand"));
        let operator = match math.str_at("op").unwrap_or_default() {
            "SUB" => "-",
            "MUL" => "*",
            "DIV" => "/",
            _ => "+",
        };
        let target = self.output(
            math.str_at("output").unwrap_or_default(),
            &format!("math '{id}' output"),
            id,
        );
        self.lines.push(format!("IF {power} THEN"));
        self.lines.push(format!(
            "{INDENT}{target} := ({left}) {operator} ({right});"
        ));
        self.lines.push("END_IF;".to_owned());
    }

    /// The function block text.
    fn text(&self, block_name: &str) -> String {
        let mut output: Vec<String> = vec![
            "(*".to_owned(),
            "  Auto-generated Ladder Diagram companion (schema v2).".to_owned(),
            "  Source of truth remains the .ladder.json file.".to_owned(),
            "  Generated to support .st-first runtime workflows.".to_owned(),
            "*)".to_owned(),
            String::new(),
            format!("FUNCTION_BLOCK {block_name}"),
        ];
        if !self.externals.is_empty() {
            output.push("VAR_EXTERNAL".to_owned());
            output.extend(
                self.externals
                    .iter()
                    .map(|(name, ty)| format!("{INDENT}{name} : {ty};")),
            );
            output.push("END_VAR".to_owned());
            output.push(String::new());
        }
        let mut locals: Vec<String> = Vec::new();
        for instance in self.timers.iter().chain(&self.counters) {
            locals.push(format!(
                "{INDENT}{} : {}; (* {} *)",
                instance.name,
                instance.block,
                comment_text(&instance.source)
            ));
        }
        for local in &self.locals {
            let initial = local
                .initial
                .as_deref()
                .filter(|initial| !initial.is_empty())
                .map(|initial| format!(" := {initial}"))
                .unwrap_or_default();
            locals.push(format!(
                "{INDENT}{} : {}{initial}; (* {} *)",
                local.name,
                local.ty,
                comment_text(&local.comment)
            ));
        }
        if !locals.is_empty() {
            output.push("VAR".to_owned());
            output.extend(locals);
            output.push("END_VAR".to_owned());
            output.push(String::new());
        }
        output.extend(self.lines.iter().cloned());
        output.push("END_FUNCTION_BLOCK".to_owned());
        output.join("\n")
    }
}

/// A contact operand as a term of the power expression: its address global, its plain name when
/// it is qualified, or the operand trimmed. Problems are recorded by [`Lowering::symbol`].
fn resolve_quietly(operand: &str, aliases: &[(String, String)]) -> String {
    if let Some(address) = direct_address(operand) {
        return aliases
            .iter()
            .find(|(known, _)| *known == address)
            .map_or_else(|| js::trim(operand).to_owned(), |(_, name)| name.clone());
    }
    if let Some((_, name)) = references::qualified(operand) {
        return name.to_owned();
    }
    js::trim(operand).to_owned()
}
