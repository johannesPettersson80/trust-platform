//! Program changes and the HMI (specification 39, §16.3, §16.4 item 6). Before a program
//! deployment through the runtime writes anything, the active bundle's bindings are checked
//! against the new program. Broken bindings refuse the deployment, unless it carries an updated
//! bundle that passes against the new program, or accepts them; then they show as "not in running
//! program".

use serde_json::{json, Value};

use super::deploy::DeployRequest;
use crate::hmi::access::config::HmiAccessConfig;
use crate::hmi::bundle::checks::{check_bundle_with_remote, BundleProblem, Program};
use crate::hmi::bundle::store::BundleStore;
use crate::hmi::bundle::HmiBundle;
use crate::hmi::catalogue::Catalogue;

/// What the check decided.
pub(super) enum ProgramCheck {
    /// Deploy the program, and the updated bundle if one came. `broken` names the bindings
    /// that stay broken because the deployment accepted them.
    Proceed {
        hmi: Option<HmiBundle>,
        broken: Vec<Value>,
    },
    /// Nothing changes; answer with this status and body.
    Refuse { status: u16, body: Value },
}

/// The check of a program deployment against the active bundle and an updated one.
pub(super) fn check_program_change(
    request: &DeployRequest,
    store: &BundleStore,
    access: &HmiAccessConfig,
    remote: &std::collections::BTreeMap<String, Value>,
) -> ProgramCheck {
    let hmi = match request.hmi.as_ref().map(HmiBundle::from_json).transpose() {
        Ok(hmi) => hmi,
        Err(error) => {
            return refuse(
                400,
                json!({ "ok": false, "error": format!("the HMI bundle: {error}") }),
            )
        }
    };
    // Without new sources there is no new program to compare.
    let Some(sources) = request
        .sources
        .as_ref()
        .filter(|sources| !sources.is_empty())
    else {
        return ProgramCheck::Proceed {
            hmi,
            broken: Vec::new(),
        };
    };
    let session = crate::harness::CompileSession::from_sources(
        sources
            .iter()
            .map(|source| {
                crate::harness::SourceFile::with_path(source.path.as_str(), source.content.clone())
            })
            .collect(),
    );
    let declarations = match session.declarations() {
        Ok(declarations) => declarations,
        Err(error) => {
            return refuse(
                422,
                json!({ "ok": false, "error": format!("the program does not compile: {error}") }),
            )
        }
    };
    let program = Program {
        metadata: declarations.metadata(),
        storage: declarations.storage(),
    };
    if let Some(updated) = &hmi {
        let problems = check_bundle_with_remote(
            updated,
            Catalogue::shipped(),
            &program,
            &access.write,
            remote,
        );
        if !problems.is_empty() {
            return refuse(
                422,
                json!({ "ok": false, "error": "the HMI bundle does not fit the new program", "problems": listed(&problems) }),
            );
        }
        return ProgramCheck::Proceed {
            hmi,
            broken: Vec::new(),
        };
    }
    let active = store.read(HmiBundle::read).unwrap_or_default();
    let broken = if active.files.is_empty() {
        Vec::new()
    } else {
        check_bundle_with_remote(
            &active,
            Catalogue::shipped(),
            &program,
            &access.write,
            remote,
        )
    };
    if broken.is_empty() {
        return ProgramCheck::Proceed {
            hmi: None,
            broken: Vec::new(),
        };
    }
    if request.accept_broken_bindings == Some(true) {
        return ProgramCheck::Proceed {
            hmi: None,
            broken: listed(&broken),
        };
    }
    refuse(
        409,
        json!({
            "ok": false,
            "error": "the program would break bindings of the active HMI; deploy it with an updated HMI bundle, or accept the broken bindings",
            "broken": listed(&broken),
        }),
    )
}

fn refuse(status: u16, body: Value) -> ProgramCheck {
    ProgramCheck::Refuse { status, body }
}

fn listed(problems: &[BundleProblem]) -> Vec<Value> {
    problems.iter().map(|problem| json!({ "file": problem.file, "line": problem.line, "message": problem.message })).collect()
}
