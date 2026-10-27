//! Implicit Boolean locals of contacts and coils (specification 16 §5, specification 25 §11.4).
//!
//! A contact or coil that names an undeclared symbol declares it: a `BOOL` local starting
//! `FALSE` (or a global when the operand is qualified `GLOBAL::`). An implicit local is one that
//! only such a declaration could have made: a local `BOOL` with an identifier name, no address and
//! no initial value other than `FALSE`. An implicit local no element names any more is removed, so
//! the names typed letter by letter into an operand (`t`, `te`, `tes`) do not stay behind.

use crate::ladder::references::{self, Scope};
use crate::naming::is_simple_identifier;
use crate::Json;

/// The scope as a variable writes it.
fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Local => "local",
        Scope::Global => "global",
    }
}

/// The variable `variable`'s name compared without case, and its scope (no scope is global).
fn identity(variable: &Json) -> (String, &str) {
    (
        references::key(variable.str_at("name").unwrap_or_default()),
        variable.str_at("scope").unwrap_or("global"),
    )
}

/// Declares the symbol of a contact or coil operand when it is not declared yet.
fn declare(variables: &mut Vec<Json>, operand: &str) {
    let Some(reference) = references::parse(operand) else {
        return;
    };
    let name = reference.name;
    if !is_simple_identifier(name) {
        return;
    }
    let key = references::key(name);
    if reference.scope.is_none() && variables.iter().any(|variable| identity(variable).0 == key) {
        return;
    }
    let scope = scope_name(reference.scope.unwrap_or(Scope::Local));
    if variables
        .iter()
        .any(|variable| identity(variable) == (key.clone(), scope))
    {
        return;
    }
    variables.push(Json::Object(vec![
        ("name".to_owned(), Json::str(name)),
        ("type".to_owned(), Json::str("BOOL")),
        ("scope".to_owned(), Json::str(scope)),
        ("initialValue".to_owned(), Json::Bool(false)),
    ]));
}

/// The members of an element that may name a symbol.
const OPERAND_MEMBERS: [&str; 8] = [
    "variable", "input", "qOutput", "etOutput", "cvOutput", "left", "right", "output",
];

/// Whether `variable` is an implicit local (see the module documentation).
fn implicit_local(variable: &Json) -> bool {
    let (_, scope) = identity(variable);
    let name = variable.str_at("name").unwrap_or_default();
    let has_address = variable
        .str_at("address")
        .is_some_and(|address| !crate::js::trim(address).is_empty());
    let default_value = match variable.get("initialValue") {
        None => true,
        Some(Json::Bool(value)) => !value,
        Some(Json::String(text)) => {
            matches!(
                crate::js::trim(text).to_uppercase().as_str(),
                "" | "FALSE" | "BOOL(FALSE)"
            )
        }
        Some(_) => false,
    };
    scope == "local"
        && variable.str_at("type") == Some("BOOL")
        && !has_address
        && is_simple_identifier(name)
        && default_value
}

/// Declares the undeclared symbols of contacts and coils and removes the implicit locals no
/// element names (see the module documentation).
pub fn reconcile_declarations(program: &mut Json) {
    let mut variables: Vec<Json> = program
        .get("variables")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();
    let networks: Vec<Json> = program
        .get("networks")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();
    let nodes: Vec<&Json> = networks
        .iter()
        .flat_map(|network| {
            network
                .get("nodes")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
        })
        .collect();
    for node in &nodes {
        if matches!(node.str_at("type"), Some("contact" | "coil")) {
            declare(&mut variables, node.str_at("variable").unwrap_or_default());
        }
    }
    let mut scoped: Vec<(String, &str)> = Vec::new();
    let mut unscoped: Vec<String> = Vec::new();
    for node in &nodes {
        for member in OPERAND_MEMBERS {
            let Some(reference) = node.str_at(member).and_then(references::parse) else {
                continue;
            };
            if !is_simple_identifier(reference.name) {
                continue;
            }
            let key = references::key(reference.name);
            match reference.scope {
                Some(scope) => scoped.push((key, scope_name(scope))),
                None => unscoped.push(key),
            }
        }
    }
    variables.retain(|variable| {
        if !implicit_local(variable) {
            return true;
        }
        let (key, scope) = identity(variable);
        scoped
            .iter()
            .any(|(name, named)| *name == key && *named == scope)
            || unscoped.contains(&key)
    });
    program.set("variables", Json::Array(variables));
}
