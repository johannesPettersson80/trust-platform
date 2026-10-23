//! Continuous editor diagnostics over declarations. Build/deployment retain their full checks.
use super::super::{HmiBindingsCommandArgs, HMI_FILE_CHECK_COMMAND};
use super::declarations::Declarations;
use crate::handlers::context::ServerContext;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use trust_runtime::hmi::page_files::{problem_line, HmiBindingDirection};

pub(in crate::handlers::commands) fn check<C: ServerContext>(
    context: &C,
    selection: &HmiBindingsCommandArgs,
    text: &str,
    file: &str,
    remote: &BTreeMap<String, Value>,
) -> Value {
    let result = || -> Result<Value, String> {
        let mut inputs = super::super::project_inputs(context, selection, HMI_FILE_CHECK_COMMAND)?;
        if file.starts_with("hmi/")
            && std::path::Path::new(file)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            inputs.open.insert(inputs.root.join(file), text.into());
        }
        if file.contains("hmi/symbols/") {
            return Ok(super::super::hmi_symbols::check_value(
                context, text, file, selection,
            ));
        }
        if file.contains("hmi/pairs/") {
            let pair = super::pairs::parse_pair(file, text).map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            let index = Declarations::read(&inputs.sources);
            let line = |key| super::super::hmi_checks::key_line(text, None, key);
            let mut problems = vec![];
            let bundle = super::bundle::read_edited(&inputs.root, &inputs.open)
                .map_err(|e| e.to_string())?;
            match super::bundle::faceplate(&bundle, &pair.faceplate) {
                None => problems.push(
                    json!({"line":line("faceplate"),"message":format!("Panel {} is missing",pair.faceplate)}),
                ),
                Some(Err(errors)) => problems.extend(
                    errors
                        .into_iter()
                        .map(|error| json!({"line":line("faceplate"),"message":error.message})),
                ),
                // The runtime's check, over the editor's declarations (§8.6).
                Some(Ok(panel)) => {
                    for problem in trust_runtime::hmi::binding_checks::check_pair(
                        &pair,
                        &panel,
                        index.as_ref(),
                    ) {
                        let at = match (&problem.field, &problem.binding) {
                            (Some(field), _) if pair.fields.contains_key(field) => {
                                super::super::hmi_checks::key_line(text, Some("fields"), field)
                            }
                            (Some(_), _) => line("faceplate"),
                            (None, Some(property)) => {
                                super::super::hmi_checks::key_line(text, Some("bind"), property)
                            }
                            (None, None) => line("block"),
                        };
                        problems.push(json!({"line": at, "message": problem.message}));
                    }
                }
            }
            return Ok(json!({"ok":true,"problems":problems}));
        }
        let mut field_problems = Vec::new();
        let catalogue = super::super::hmi_symbols::project_catalogue(&inputs.root, &inputs.open)?;
        let page = if file.contains("hmi/panels/") {
            super::documents::faceplate(file, text).map(|panel| {
                field_problems =
                    trust_runtime::hmi::binding_checks::field_problems(&panel, &catalogue);
                panel.popup.page
            })
        } else {
            super::documents::page(file, text, file.contains("hmi/popups/")).map(|(page, _)| page)
        };
        let page = match page {
            Ok(page) => page,
            Err(errors) => return Ok(super::super::hmi_files::file_errors(&errors)),
        };
        let index = Declarations::read(&inputs.sources);
        let variables = super::design_variables::variables(&inputs, index.as_ref())?;
        let resolve = |path: &str| super::declarations::resolve_nodes(&variables, path);
        let check = |component: &trust_runtime::hmi::catalogue::ComponentDef,
                     property: &str,
                     binding: &trust_runtime::hmi::page_files::HmiBinding|
         -> Result<(), String> {
            // Panel fields are checked when an instance is selected; they are not PLC paths.
            if binding.path.starts_with('$') {
                return Ok(());
            }
            // The direction is checked by the shared page check, which knows the object's command.
            let rule = component
                .property(property)
                .ok_or_else(|| format!("{} has no property {property}", component.kind))?;
            if binding.path.contains(':') {
                let key = trust_runtime::hmi::remote_values::qualified_key(&binding.path);
                let node = remote
                    .get(&key)
                    .ok_or("The owning runtime has no fresh declaration for this binding")?;
                let kinds = node["kinds"]
                    .as_array()
                    .ok_or("The remote declaration has no value type")?;
                if !rule.accepts.iter().any(|kind| {
                    serde_json::to_value(kind)
                        .ok()
                        .is_some_and(|kind| kinds.contains(&kind))
                }) {
                    return Err("The remote value has an incompatible type".into());
                }
                if binding.direction != HmiBindingDirection::Read && node["writable"] != true {
                    return Err("The remote value is read-only".into());
                }
                return Ok(());
            }
            let Some(value) = resolve(&binding.path) else {
                // An undeclared draft is expected before Create PLC code. Report it without
                // trying to compile or synthesizing a fake running instance.
                return Err(format!(
                    "{} is not declared yet; Create PLC code after configuring the design",
                    binding.path
                ));
            };
            if !rule.accepts.iter().any(|kind| value.kinds.contains(kind)) {
                return Err(format!(
                    "{} has incompatible type {}",
                    binding.path, value.type_name
                ));
            }
            if binding.direction != HmiBindingDirection::Read && !value.writable {
                return Err(format!("{} is read-only", binding.path));
            }
            if let Some(quality) = &binding.quality
                && !resolve(quality).is_some_and(|node| {
                    node.kinds
                        .contains(&trust_runtime::hmi::catalogue::ValueKind::Boolean)
                        || node
                            .kinds
                            .contains(&trust_runtime::hmi::catalogue::ValueKind::Enumeration)
                })
            {
                return Err(format!("{quality} must name a BOOL or quality enumeration"));
            }
            Ok(())
        };
        let mut problems =
            trust_runtime::hmi::binding_checks::check_declared_page(&page, &catalogue, &check);
        problems.extend(field_problems);
        problems.extend(super::super::hmi_checks::policy_problems(
            &trust_runtime::hmi::binding_checks::without_fields(&page),
            &inputs.root,
            &catalogue,
        ));
        for object in &page.objects {
            if let Some(command) = object
                .command
                .as_ref()
                .filter(|command| !command.instance.starts_with('$'))
            {
                let target = resolve(&command.instance);
                let public_method = |kind: &str| {
                    index
                        .registry
                        .lookup(kind)
                        .and_then(|ty| {
                            index
                                .symbols
                                .resolve_member_symbol_in_type(ty, &command.method)
                        })
                        .and_then(|id| index.symbols.get(id))
                        .is_some_and(|symbol| {
                            matches!(symbol.kind, trust_hir::symbols::SymbolKind::Method { .. })
                                && symbol.visibility == trust_hir::symbols::Visibility::Public
                        })
                };
                let valid = catalogue.component(&object.kind).is_some_and(|component| {
                    component.category == trust_runtime::hmi::catalogue::ComponentCategory::Control
                }) && index.symbol(&command.interface).is_some_and(|symbol| {
                    matches!(symbol.kind, trust_hir::symbols::SymbolKind::Interface)
                }) && target.is_some_and(|node| {
                    index.implements_capability(&node.type_name, &command.interface)
                        && public_method(&node.type_name)
                }) && public_method(&command.interface);
                if !valid {
                    problems.push(trust_runtime::hmi::binding_checks::BindingProblem {
                        object: object.id.clone(),
                        property: Some("command".into()),
                        message: format!(
                            "Choose an instance exposing public {}.{}",
                            command.interface, command.method
                        ),
                    });
                }
            }
        }
        problems.sort_by_key(|problem| {
            page.objects
                .iter()
                .position(|object| object.id == problem.object)
                .unwrap_or(usize::MAX)
        });
        let problems=problems.into_iter().map(|problem|json!({"line":problem_line(text,&problem.object,problem.property.as_deref()),"object":problem.object,"property":problem.property,"message":problem.message})).collect::<Vec<_>>();
        Ok(json!({"ok":true,"problems":problems,"validation":"declarations"}))
    };
    result().unwrap_or_else(super::super::hmi_files::refused)
}
