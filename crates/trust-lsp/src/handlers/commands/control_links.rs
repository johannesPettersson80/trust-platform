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
/// call's `Setpoint` sources. No name or role is guessed; unresolved shapes are simply not reported.
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
    Some((
        instance.text().to_string().trim().to_string(),
        kind.text().to_string().trim().to_string(),
    ))
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
        .map(|name| name.text().to_string().trim().to_string())
}

/// Calls, record-field assignments and assignment targets of one POU body, in local names.
struct PouBody {
    declared: HashSet<String>,
    calls: Vec<Call>,
    fields: BTreeMap<(String, String), BTreeSet<String>>,
    writes: BTreeSet<String>,
}

struct Call {
    instance: String,
    method: String,
    argument: String,
    record: String,
    output: Option<String>,
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
                    .map(|name| name.text().to_string().trim().to_ascii_uppercase())
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut body = Self {
            declared,
            calls: Vec::new(),
            fields: BTreeMap::new(),
            writes: BTreeSet::new(),
        };
        // Nested METHOD/ACTION bodies have their own scope; only the POU body is read.
        for node in pou
            .descendants()
            .filter(|node| !inside_nested_body(node, pou))
        {
            match node.kind() {
                SyntaxKind::CallExpr => {
                    if let Some(call) = controller_call(&node) {
                        body.calls.push(call);
                    }
                }
                SyntaxKind::AssignStmt => {
                    let mut children = node.children();
                    let (Some(target), Some(value)) = (children.next(), children.next()) else {
                        continue;
                    };
                    body.writes.extend(symbol_path(&target));
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
                json!({
                    "instance": self.path(instance, &call.instance),
                    "method": call.method,
                    "argument": call.argument,
                    "record": self.path(instance, &call.record),
                    "output": call.output.as_ref().map(|output| self.path(instance, output)),
                    "fields": fields,
                })
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
            Some((
                name.text().to_string().trim().to_string(),
                symbol_path(&value)?,
            ))
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
    })
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
        .map(|name| name.text().to_string().trim().to_string())
}

/// The dotted path of a plain variable or field reference; other expressions have none.
fn symbol_path(node: &SyntaxNode) -> Option<String> {
    match node.kind() {
        SyntaxKind::NameRef => Some(node.text().to_string().trim().to_string()),
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
        const SOURCE: &str = r#"
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
                "output": null, "fields": { "Setpoint": ["P1.ManualC"] }
            }),
            "a program that initializes its controller replaces the controller state validation starts from"
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
