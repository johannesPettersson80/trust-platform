use serde_json::{json, Map, Value as JsonValue};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

/// Compiled data links between controller calls for Control Studio setup (Specification 35 §17.1).
///
/// For every `<output> := <instance>.<method>(<argument> := <record>)` call in a PROGRAM or
/// FUNCTION_BLOCK body, reports the record's field sources: every symbol read on the right-hand
/// side of an assignment to `<record>.<field>` in the same body. Local names are prefixed with each
/// compiled instance of that POU, so a cascade appears as the outer call's output among the inner
/// call's `Setpoint` sources. A call also reports what its code shape states: `guardedBy`, the BOOL
/// of an enclosing `IF NOT <flag> THEN` (a controller initialized once at startup); `mode`, the BOOL
/// an `IF` tests to set `<record>.Mode` and the mode it chooses while TRUE; and `copiesOfOutput`,
/// the variables assigned the call's output unchanged (a manual output that follows the output).
/// No name or role is guessed; unresolved shapes are simply not reported.
pub(super) fn facts(sources: &[&str], instances: &[(String, String)]) -> ControlFacts {
    let roots = sources
        .iter()
        .map(|source| parse(source).syntax())
        .collect::<Vec<_>>();
    // Compiled storage names a PROGRAM instance after itself; its type comes from `PROGRAM P1 ... : Main`.
    let mut instances = instances.to_vec();
    instances.extend(
        roots
            .iter()
            .flat_map(|root| root.descendants())
            .filter_map(|node| program_configuration(&node)),
    );
    // A caller may already report a program instance with its type; each pair counts once.
    let mut seen = HashSet::new();
    instances.retain(|entry| seen.insert(entry.clone()));
    let mut found = ControlFacts::default();
    for root in &roots {
        for pou in root
            .descendants()
            .filter(|node| matches!(node.kind(), SyntaxKind::Program | SyntaxKind::FunctionBlock))
        {
            let Some(name) = qualified_pou_name(&pou) else {
                continue;
            };
            let body = PouBody::read(&pou);
            for (path, _) in instances
                .iter()
                .filter(|(_, kind)| kind.eq_ignore_ascii_case(&name))
            {
                found.links.extend(body.links(path));
                found
                    .writes
                    .extend(body.writes.iter().map(|local| body.path(path, local)));
            }
        }
    }
    found
}

/// The controller calls of every compiled instance, and every symbol its code assigns.
#[derive(Debug, Default)]
pub(super) struct ControlFacts {
    pub(super) links: Vec<JsonValue>,
    /// Absolute paths, sorted: validation seeds a program's state, and code that assigns a seeded
    /// symbol (for example in a startup branch) replaces the seeded value on the first scan.
    pub(super) writes: BTreeSet<String>,
}

/// `(instance, program type)` of every `PROGRAM <instance> [WITH <task>] : <type>` configuration entry.
pub(super) fn program_types(sources: &[&str]) -> Vec<(String, String)> {
    sources
        .iter()
        .flat_map(|source| {
            parse(source)
                .syntax()
                .descendants()
                .filter_map(|node| program_configuration(&node))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `(instance, program type)` of one `PROGRAM <instance> [WITH <task>] : <type>` configuration entry.
fn program_configuration(node: &SyntaxNode) -> Option<(String, String)> {
    if node.kind() != SyntaxKind::ProgramConfig {
        return None;
    }
    let instance = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let kind = node.children().find(|child| {
        matches!(
            child.kind(),
            SyntaxKind::QualifiedName | SyntaxKind::TypeRef
        )
    })?;
    Some((code_text(&instance), code_text(&kind)))
}

/// A node's text without its trivia. The parser keeps the comments and whitespace that precede a
/// token inside the innermost node started before it, so the node of a name written after a
/// comment (`(* note *) Level`, a first statement after `THEN`) holds that comment too.
pub(super) fn code_text(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

/// A POU name qualified by its enclosing namespaces, matching compiled type names.
fn qualified_pou_name(pou: &SyntaxNode) -> Option<String> {
    let own = direct_name(pou)?;
    let mut parts = vec![own];
    for ancestor in pou.ancestors().skip(1) {
        if ancestor.kind() == SyntaxKind::Namespace {
            parts.push(direct_name(&ancestor)?);
        }
    }
    parts.reverse();
    Some(parts.join("."))
}

fn direct_name(node: &SyntaxNode) -> Option<String> {
    node.children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
        .map(|name| code_text(&name))
}

/// Calls, record-field assignments and assignment targets of one POU body, in local names.
struct PouBody {
    declared: HashSet<String>,
    calls: Vec<Call>,
    fields: BTreeMap<(String, String), BTreeSet<String>>,
    writes: BTreeSet<String>,
    /// `<target> := <source>` assignments of one variable to another.
    copies: Vec<(String, String)>,
    /// By upper-case record: the BOOL an `IF` tests to set `<record>.Mode`, and the mode it
    /// chooses while that BOOL is TRUE.
    modes: BTreeMap<String, (String, &'static str)>,
}

struct Call {
    instance: String,
    method: String,
    argument: String,
    record: String,
    output: Option<String>,
    /// The BOOL whose FALSE value the call needs: it stands in the THEN branch of `IF NOT <flag>`.
    guard: Option<String>,
}

impl PouBody {
    fn read(pou: &SyntaxNode) -> Self {
        let declared = pou
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::VarDecl && !inside_nested_body(node, pou))
            .flat_map(|declaration| {
                // `a, b : T` lists its names in a VarList; the type's own names sit inside TypeRef.
                let owner = declaration
                    .children()
                    .find(|child| child.kind() == SyntaxKind::VarList)
                    .unwrap_or_else(|| declaration.clone());
                owner
                    .children()
                    .filter(|child| child.kind() == SyntaxKind::Name)
                    .map(|name| code_text(&name).to_ascii_uppercase())
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut body = Self {
            declared,
            calls: Vec::new(),
            fields: BTreeMap::new(),
            writes: BTreeSet::new(),
            copies: Vec::new(),
            modes: BTreeMap::new(),
        };
        // Nested METHOD/ACTION bodies have their own scope; only the POU body is read.
        for node in pou
            .descendants()
            .filter(|node| !inside_nested_body(node, pou))
        {
            match node.kind() {
                SyntaxKind::CallExpr => {
                    if let Some(mut call) = controller_call(&node) {
                        call.guard = negated_guard(&node, pou);
                        body.calls.push(call);
                    }
                }
                SyntaxKind::IfStmt => {
                    if let Some((record, symbol, mode)) = mode_choice(&node) {
                        body.modes
                            .entry(record.to_ascii_uppercase())
                            .or_insert((symbol, mode));
                    }
                }
                SyntaxKind::AssignStmt => {
                    let mut children = node.children();
                    let (Some(target), Some(value)) = (children.next(), children.next()) else {
                        continue;
                    };
                    body.writes.extend(symbol_path(&target));
                    if let (Some(target), Some(source)) =
                        (symbol_path(&target), symbol_path(&value))
                    {
                        body.copies.push((target, source));
                    }
                    if target.kind() != SyntaxKind::FieldExpr {
                        continue;
                    }
                    let (Some(record), Some(field)) = (field_base(&target), field_name(&target))
                    else {
                        continue;
                    };
                    let sources = body.fields.entry((record, field)).or_default();
                    sources.extend(read_symbols(&value));
                }
                _ => {}
            }
        }
        body
    }

    fn links(&self, instance: &str) -> Vec<JsonValue> {
        self.calls
            .iter()
            .map(|call| {
                let mut fields = Map::new();
                for ((record, field), sources) in &self.fields {
                    if record.eq_ignore_ascii_case(&call.record) {
                        let paths = sources
                            .iter()
                            .map(|source| self.path(instance, source))
                            .collect::<Vec<_>>();
                        fields.insert(field.clone(), json!(paths));
                    }
                }
                let mut link = json!({
                    "instance": self.path(instance, &call.instance),
                    "method": call.method,
                    "argument": call.argument,
                    "record": self.path(instance, &call.record),
                    "output": call.output.as_ref().map(|output| self.path(instance, output)),
                    "fields": fields,
                });
                if let Some(guard) = &call.guard {
                    link["guardedBy"] = json!(self.path(instance, guard));
                }
                if let Some((symbol, mode)) = self.modes.get(&call.record.to_ascii_uppercase()) {
                    link["mode"] =
                        json!({ "symbol": self.path(instance, symbol), "whenTrue": mode });
                }
                if let Some(output) = &call.output {
                    let copies = self
                        .copies
                        .iter()
                        .filter(|(_, source)| source.eq_ignore_ascii_case(output))
                        .map(|(target, _)| self.path(instance, target))
                        .collect::<Vec<_>>();
                    if !copies.is_empty() {
                        link["copiesOfOutput"] = json!(copies);
                    }
                }
                link
            })
            .collect()
    }

    /// Prefixes a local name with the instance path; undeclared roots are global paths.
    fn path(&self, instance: &str, local: &str) -> String {
        let root = local
            .split(['.', '['])
            .next()
            .unwrap_or(local)
            .to_ascii_uppercase();
        if self.declared.contains(&root) {
            format!("{instance}.{local}")
        } else {
            local.to_string()
        }
    }
}

fn inside_nested_body(node: &SyntaxNode, pou: &SyntaxNode) -> bool {
    node.ancestors()
        .take_while(|ancestor| ancestor != pou)
        .any(|ancestor| {
            matches!(
                ancestor.kind(),
                SyntaxKind::Method | SyntaxKind::Action | SyntaxKind::Property
            )
        })
}

/// `<instance>.<method>(<argument> := <record>)`, with its assignment target when assigned.
fn controller_call(call: &SyntaxNode) -> Option<Call> {
    let mut children = call.children();
    let callee = children.next()?;
    if callee.kind() != SyntaxKind::FieldExpr {
        return None;
    }
    let instance = symbol_path(&field_base_node(&callee)?)?;
    let method = field_name(&callee)?;
    let arguments = children.find(|child| child.kind() == SyntaxKind::ArgList)?;
    let (argument, record) = arguments
        .children()
        .filter(|arg| arg.kind() == SyntaxKind::Arg)
        .find_map(|arg| {
            let name = arg
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)?;
            let value = arg
                .children()
                .find(|child| child.kind() != SyntaxKind::Name)?;
            Some((code_text(&name), symbol_path(&value)?))
        })?;
    let output = call
        .parent()
        .filter(|parent| parent.kind() == SyntaxKind::AssignStmt)
        .and_then(|assign| {
            let target = assign.children().next()?;
            (target != *call).then(|| symbol_path(&target)).flatten()
        });
    Some(Call {
        instance,
        method,
        argument,
        record,
        output,
        guard: None,
    })
}

/// The BOOL whose FALSE value `call` needs: the nearest enclosing `IF` has the call in its THEN
/// branch and tests `NOT <flag>`.
fn negated_guard(call: &SyntaxNode, pou: &SyntaxNode) -> Option<String> {
    let mut child = call.clone();
    for ancestor in call.ancestors().skip(1) {
        if ancestor == *pou {
            return None;
        }
        if ancestor.kind() == SyntaxKind::IfStmt {
            let condition = ancestor.children().next()?;
            if child == condition
                || matches!(
                    child.kind(),
                    SyntaxKind::ElsifBranch | SyntaxKind::ElseBranch
                )
                || condition.kind() != SyntaxKind::UnaryExpr
                || !is_not(&condition)
            {
                return None;
            }
            return symbol_path(&condition.children().next()?);
        }
        child = ancestor;
    }
    None
}

/// `IF <condition> THEN <record>.Mode := <type>#Auto|Manual; …`: the record, the BOOL the condition
/// starts with and the mode chosen while that BOOL is TRUE. `IF AutoMode THEN … Auto` and
/// `IF Manual OR NOT Good THEN … Manual` both name their Auto/Manual signal.
fn mode_choice(if_stmt: &SyntaxNode) -> Option<(String, String, &'static str)> {
    let condition = if_stmt.children().next()?;
    let (symbol, negated) = leading_symbol(&condition)?;
    // The THEN branch's statements are the IF's own children; ELSIF and ELSE are branch nodes.
    let (record, mode) = if_stmt
        .children()
        .skip(1)
        .filter(|node| node.kind() == SyntaxKind::AssignStmt)
        .find_map(|assign| {
            let mut parts = assign.children();
            let (target, value) = (parts.next()?, parts.next()?);
            if target.kind() != SyntaxKind::FieldExpr
                || !field_name(&target)?.eq_ignore_ascii_case("Mode")
            {
                return None;
            }
            Some((field_base(&target)?, mode_literal(&value)?))
        })?;
    let chosen = match (mode, negated) {
        ("Auto", true) => "Manual",
        ("Manual", true) => "Auto",
        (mode, false) => mode,
        _ => return None,
    };
    Some((record, symbol, chosen))
}

/// `Auto` or `Manual` of an enumeration value such as `PIDMode#Manual`.
fn mode_literal(value: &SyntaxNode) -> Option<&'static str> {
    let text = code_text(value);
    let word = text.rsplit('#').next()?;
    if word.eq_ignore_ascii_case("Auto") {
        Some("Auto")
    } else if word.eq_ignore_ascii_case("Manual") {
        Some("Manual")
    } else {
        None
    }
}

/// The variable a condition starts with, and whether it is negated: `A`, `NOT A`, `A OR B`.
fn leading_symbol(condition: &SyntaxNode) -> Option<(String, bool)> {
    if let Some(path) = symbol_path(condition) {
        return Some((path, false));
    }
    match condition.kind() {
        SyntaxKind::UnaryExpr if is_not(condition) => {
            let (symbol, negated) = leading_symbol(&condition.children().next()?)?;
            Some((symbol, !negated))
        }
        SyntaxKind::BinaryExpr | SyntaxKind::ParenExpr => {
            leading_symbol(&condition.children().next()?)
        }
        _ => None,
    }
}

fn is_not(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == SyntaxKind::KwNot)
}

fn field_base_node(field: &SyntaxNode) -> Option<SyntaxNode> {
    field.children().next()
}

fn field_base(field: &SyntaxNode) -> Option<String> {
    symbol_path(&field_base_node(field)?)
}

fn field_name(field: &SyntaxNode) -> Option<String> {
    field
        .children()
        .filter(|child| child.kind() == SyntaxKind::Name)
        .last()
        .map(|name| code_text(&name))
}

/// The dotted path of a plain variable or field reference; other expressions have none.
fn symbol_path(node: &SyntaxNode) -> Option<String> {
    match node.kind() {
        SyntaxKind::NameRef => Some(code_text(node)),
        SyntaxKind::FieldExpr => Some(format!(
            "{}.{}",
            symbol_path(&field_base_node(node)?)?,
            field_name(node)?
        )),
        SyntaxKind::ParenExpr => symbol_path(&node.children().next()?),
        _ => None,
    }
}

/// Every symbol read by an expression. A function call contributes its arguments, not its name;
/// a method call on an instance contributes that instance, whose call produces the value.
fn read_symbols(value: &SyntaxNode) -> Vec<String> {
    if let Some(path) = symbol_path(value) {
        return vec![path];
    }
    match value.kind() {
        SyntaxKind::CallExpr => {
            let Some(callee) = value.children().next() else {
                return Vec::new();
            };
            if callee.kind() == SyntaxKind::FieldExpr {
                return field_base(&callee).into_iter().collect();
            }
            value
                .children()
                .filter(|node| node.kind() == SyntaxKind::ArgList)
                .flat_map(|arguments| {
                    arguments
                        .children()
                        .filter(|node| node.kind() == SyntaxKind::Arg)
                        .collect::<Vec<_>>()
                })
                .filter_map(|argument| {
                    argument
                        .children()
                        .find(|node| node.kind() != SyntaxKind::Name)
                })
                .flat_map(|expression| read_symbols(&expression))
                .collect()
        }
        _ => value
            .children()
            .flat_map(|child| read_symbols(&child))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::facts;
    use serde_json::json;

    fn links(sources: &[&str], instances: &[(String, String)]) -> Vec<serde_json::Value> {
        facts(sources, instances).links
    }

    const CASCADE: &str = r#"
PROGRAM Main
VAR
    Outer : Control.PID;
    Inner : Control.PID;
    OuterInput : Control.PIDInput;
    InnerInput : Control.PIDInput;
    VesselSetpointC : LREAL;
    VesselC : LREAL;
    JacketC : LREAL;
    JacketSetpointC : LREAL;
    HeaterPct : LREAL;
END_VAR
OuterInput.Setpoint := VesselSetpointC;
OuterInput.ProcessValue := VesselC;
JacketSetpointC := Outer.Update(Input := OuterInput);
InnerInput.Setpoint := LIMIT(LREAL#0.0, JacketSetpointC, LREAL#80.0);
IF VesselC > LREAL#50.0 THEN
    InnerInput.ProcessValue := JacketC;
ELSE
    InnerInput.ProcessValue := VesselC;
END_IF;
HeaterPct := Inner.Update(Input := InnerInput);
END_PROGRAM
"#;

    fn instances(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(path, kind)| (path.to_string(), kind.to_string()))
            .collect()
    }

    #[test]
    fn reports_each_controller_call_with_its_output_and_record_field_sources() {
        let found = links(
            &[CASCADE],
            &instances(&[
                ("P1", "Main"),
                ("P1.Outer", "Control.PID"),
                ("P1.Inner", "Control.PID"),
            ]),
        );
        assert_eq!(
            found,
            vec![
                json!({
                    "instance": "P1.Outer", "method": "Update", "argument": "Input",
                    "record": "P1.OuterInput", "output": "P1.JacketSetpointC",
                    "fields": { "ProcessValue": ["P1.VesselC"], "Setpoint": ["P1.VesselSetpointC"] }
                }),
                json!({
                    "instance": "P1.Inner", "method": "Update", "argument": "Input",
                    "record": "P1.InnerInput", "output": "P1.HeaterPct",
                    "fields": { "ProcessValue": ["P1.JacketC", "P1.VesselC"], "Setpoint": ["P1.JacketSetpointC"] }
                }),
            ],
            "the inner setpoint's sources include the outer call's output: a cascade"
        );
    }

    #[test]
    fn prefixes_every_compiled_instance_of_a_function_block_body() {
        const SOURCE: &str = r#"
FUNCTION_BLOCK Station
VAR
    Loop : Control.PID;
    Request : Control.PIDInput;
    Level : LREAL;
    Target : LREAL;
END_VAR
Request.Setpoint := Target;
Request.ProcessValue := Level;
Loop.Update(Input := Request);
END_FUNCTION_BLOCK
"#;
        let found = links(
            &[SOURCE],
            &instances(&[("P1.North", "Station"), ("P1.South", "Station")]),
        );
        let identities = found
            .iter()
            .map(|link| {
                (
                    link["instance"].clone(),
                    link["output"].clone(),
                    link["fields"]["Setpoint"].clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            identities,
            vec![
                (
                    json!("P1.North.Loop"),
                    json!(null),
                    json!(["P1.North.Target"])
                ),
                (
                    json!("P1.South.Loop"),
                    json!(null),
                    json!(["P1.South.Target"])
                ),
            ],
            "a call statement without an assignment has no output"
        );
    }

    #[test]
    fn maps_each_configured_program_instance_to_its_program_type() {
        const SOURCE: &str = r#"
PROGRAM Main
VAR
    Loop : Control.PID;
    Request : Control.PIDInput;
    Output : LREAL;
    Target : LREAL;
END_VAR
Request.Setpoint := Target;
Output := Loop.Update(Input := Request);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;
        let found = links(&[SOURCE], &[]);
        let tree = trust_syntax::parser::parse(SOURCE)
            .syntax()
            .descendants()
            .find(|node| node.kind() == trust_syntax::syntax::SyntaxKind::ProgramConfig)
            .map(|node| format!("{node:#?}"))
            .unwrap_or_default();
        assert_eq!(
            found.first().map(|link| link["instance"].clone()),
            Some(serde_json::json!("P1.Loop")),
            "compiled storage names P1 after itself; the configuration gives its type. ProgramConfig: {tree}"
        );
    }

    #[test]
    fn reports_every_symbol_each_instance_assigns_and_its_controller_initialization() {
        const SOURCE: &str = r#"USING Plant;
USING Alarm;
USING Equipment;
USING Control;

PROGRAM Main
VAR
    Loop : Control.PID;
    Request : Control.PIDInput;
    State : Control.PIDInitialState;
    ManualC : LREAL;
    Output : LREAL;
    Initialized : BOOL;
END_VAR
IF NOT Initialized THEN
    ManualC := LREAL#38.0;
    State.Setpoint := ManualC;
    Loop.Initialize(State := State);
    Initialized := TRUE;
END_IF;
Request.ManualOutput := ManualC;
Output := Loop.Update(Input := Request);
Plant.Demand := Output;
END_PROGRAM
"#;
        let found = facts(&[SOURCE], &instances(&[("P1", "Main")]));
        assert_eq!(
            found.writes.iter().cloned().collect::<Vec<_>>(),
            vec!["P1.Initialized", "P1.ManualC", "P1.Output", "P1.Request.ManualOutput", "P1.State.Setpoint", "Plant.Demand"],
            "every assignment target, local names under the instance and undeclared names as global paths"
        );
        assert_eq!(
            found.links[0],
            json!({
                "instance": "P1.Loop", "method": "Initialize", "argument": "State", "record": "P1.State",
                "output": null, "fields": { "Setpoint": ["P1.ManualC"] }, "guardedBy": "P1.Initialized"
            }),
            "a program that initializes its controller replaces the controller state validation starts from, \
             and the flag it tests marks that startup as done"
        );
    }

    /// The loop Create PLC code writes (specification 39 §10.2), with its region markers and its
    /// comments after THEN and ELSE: the parser keeps a comment inside the node of the name it
    /// precedes, so names are read without it.
    const GENERATED_LOOP: &str = r#"USING Control;
USING Plant;

PROGRAM Main
VAR
    (* truST plant: declarations sha256:0 *)
    EH101 : Plant.FB_Heater;
    TIC101 : Control.PID;
    TIC101_Input : Control.PIDInput;
    TIC101_Start : Control.PIDInitialState;
    TIC101_Initialized : BOOL;
    TT101 : Plant.FB_TemperatureInput;
    (* end of truST plant declarations *)
END_VAR
VAR RETAIN
    (* truST plant: retained declarations sha256:0 *)
    TIC101_Parameters : Control.PIDParameters;
    TIC101_Manual : BOOL := TRUE;
    TIC101_Setpoint : LREAL;
    TIC101_ManualOutput : LREAL;
    TIC101_Output : LREAL;
    (* end of truST plant retained declarations *)
END_VAR
    (* truST plant: calls sha256:0 *)
    (* A PID loop: Control.PID holds its measurement at its setpoint. *)
    TIC101.Configure(Parameters := TIC101_Parameters);
    TIC101_Input.Setpoint := TIC101_Setpoint;
    TIC101_Input.ManualOutput := TIC101_ManualOutput;
    IF TT101.GetQuality() = E_Quality#Good THEN
        TIC101_Input.ProcessValue := TT101.GetValue();
        IF NOT TIC101_Initialized THEN
            (* It starts from the measurement and the output as they are: no bump. *)
            TIC101_Start.Setpoint := TIC101_Setpoint;
            TIC101_Start.NominalOutput := TIC101_Output;
            TIC101.Initialize(State := TIC101_Start);
            TIC101_Initialized := TRUE;
        END_IF;
        IF TIC101_Manual THEN
            TIC101_Input.Mode := PIDMode#Manual;
        ELSE
            TIC101_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        (* A bad measurement holds the output; the loop starts again from the next good one. *)
        TIC101_Input.Mode := PIDMode#Manual;
        TIC101_Initialized := FALSE;
    END_IF;
    TIC101_Output := TIC101.Update(Input := TIC101_Input);
    IF TIC101_Input.Mode = PIDMode#Auto THEN
        (* Manual takes over from the output as it is. *)
        TIC101_ManualOutput := TIC101_Output;
    END_IF;
    EH101.AutoPower := TIC101_Output;
    (* end of truST plant calls *)
END_PROGRAM
"#;

    #[test]
    fn reports_the_startup_flag_mode_signal_and_output_copies_of_a_generated_loop() {
        let found = links(&[GENERATED_LOOP], &instances(&[("Main", "Main")]));
        let call = |method: &str| {
            found
                .iter()
                .find(|link| link["method"] == method)
                .cloned()
                .unwrap_or_default()
        };
        assert_eq!(call("Configure")["record"], json!("Main.TIC101_Parameters"));
        assert_eq!(
            call("Initialize")["guardedBy"],
            json!("Main.TIC101_Initialized"),
            "the flag tested in IF NOT … around Initialize is the startup-done flag"
        );
        let update = call("Update");
        assert_eq!(update["output"], json!("Main.TIC101_Output"));
        assert_eq!(
            update["fields"]["Setpoint"],
            json!(["Main.TIC101_Setpoint"])
        );
        assert_eq!(
            update["fields"]["ManualOutput"],
            json!(["Main.TIC101_ManualOutput"])
        );
        assert_eq!(
            call("Initialize")["fields"]["Setpoint"],
            json!(["Main.TIC101_Setpoint"]),
            "a field assigned after a comment belongs to its own record"
        );
        let writes = facts(&[GENERATED_LOOP], &instances(&[("Main", "Main")])).writes;
        assert!(
            writes.contains("Main.TIC101_ManualOutput")
                && !writes.contains("Main.TIC101_Parameters")
                && writes.iter().all(|write| write.starts_with("Main.")),
            "every assigned name is a declared one, the first of each region too: {writes:?}"
        );
        assert_eq!(
            update["mode"],
            json!({ "symbol": "Main.TIC101_Manual", "whenTrue": "Manual" }),
            "IF TIC101_Manual THEN Mode := Manual: TIC101_Manual is the Auto/Manual signal, TRUE in Manual"
        );
        assert_eq!(
            update["copiesOfOutput"],
            json!(["Main.TIC101_Start.NominalOutput", "Main.TIC101_ManualOutput", "Main.EH101.AutoPower"]),
            "in source order: the start state, the manual output that follows the output, and the actuator"
        );
        assert!(call("Configure").get("guardedBy").is_none() && update.get("guardedBy").is_none());
    }

    #[test]
    fn a_mode_signal_true_in_auto_and_a_negated_one_name_the_mode_they_choose() {
        const SOURCE: &str = r#"
PROGRAM Main
VAR
    Loop : Control.PID;
    Request : Control.PIDInput;
    Other : Control.PID;
    OtherRequest : Control.PIDInput;
    AutoMode : BOOL;
    Hand : BOOL;
    Output : LREAL;
    OtherOutput : LREAL;
END_VAR
IF AutoMode THEN
    Request.Mode := PIDMode#Auto;
ELSE
    Request.Mode := PIDMode#Manual;
END_IF;
Output := Loop.Update(Input := Request);
IF NOT Hand THEN
    OtherRequest.Mode := PIDMode#Auto;
END_IF;
OtherOutput := Other.Update(Input := OtherRequest);
END_PROGRAM
"#;
        let found = links(&[SOURCE], &instances(&[("P1", "Main")]));
        assert_eq!(
            found[0]["mode"],
            json!({ "symbol": "P1.AutoMode", "whenTrue": "Auto" })
        );
        assert_eq!(
            found[1]["mode"],
            json!({ "symbol": "P1.Hand", "whenTrue": "Manual" }),
            "NOT Hand chooses Auto, so Hand TRUE means Manual"
        );
        assert!(
            found
                .iter()
                .all(|link| link.get("copiesOfOutput").is_none()),
            "no variable takes either output unchanged"
        );
    }

    #[test]
    fn keeps_undeclared_names_as_global_paths() {
        const SOURCE: &str = r#"
PROGRAM Main
VAR
    Loop : Control.PID;
    Request : Control.PIDInput;
    Output : LREAL;
END_VAR
Request.Setpoint := PlantSetpoint;
Output := Loop.Update(Input := Request);
END_PROGRAM
"#;
        let found = links(&[SOURCE], &instances(&[("P1", "Main")]));
        assert_eq!(found[0]["fields"]["Setpoint"], json!(["PlantSetpoint"]));
    }
}
