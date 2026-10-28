//! The checks of an HMI bundle before it is deployed (specification 39, §16.2.1–2): its files
//! load, the assets its pages use are in it, its bindings resolve in the program with a
//! compatible type and direction, and its controls are within the write policy. The editor runs
//! them against the edited program, the runtime against the running one.

use crate::hmi::access::config::WriteRule;
use crate::hmi::access::page_policy::check_page_policy;
use crate::hmi::binding_checks::{check_page_with_remote, check_placements};
use crate::hmi::catalogue::Catalogue;
use crate::hmi::my_symbols::symbols_in;
use crate::hmi::page_files::{
    click_line, compose_docked, object_key_line, parse_page, parse_popup, parse_screen,
    problem_line, HmiClick, HmiPage, PageFileError,
};
use crate::memory::VariableStorage;
use crate::runtime::RuntimeMetadata;

use super::HmiBundle;

/// One problem with a bundle, on a line of one of its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleProblem {
    /// The file, `hmi/pages/overview.toml`.
    pub file: String,
    /// The one-based line.
    pub line: usize,
    /// Plain description.
    pub message: String,
}

/// The program a bundle is checked against.
pub struct Program<'a> {
    /// What the program declares.
    pub metadata: &'a RuntimeMetadata,
    /// Its variables.
    pub storage: &'a VariableStorage,
}

const SCREEN: &str = "hmi/screen.toml";

/// Every problem of a bundle.
pub fn check_bundle(
    bundle: &HmiBundle,
    catalogue: &Catalogue,
    program: &Program<'_>,
    policy: &[WriteRule],
) -> Vec<BundleProblem> {
    check_bundle_with_remote(
        bundle,
        catalogue,
        program,
        policy,
        &std::collections::BTreeMap::new(),
    )
}

/// Deployment additionally checks remote paths against fresh owning-runtime declarations.
pub fn check_bundle_with_remote(
    bundle: &HmiBundle,
    catalogue: &Catalogue,
    program: &Program<'_>,
    policy: &[WriteRule],
    remote: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Vec<BundleProblem> {
    let mut problems = Vec::new();
    // The project's own symbols are components of its pages (§14.4).
    let (symbols, symbol_errors) = symbols_in(&bundle.files);
    problems.extend(symbol_errors.into_iter().map(from_file_error));
    let catalogue = &catalogue.with_symbols(&symbols);
    match bundle.files.get(SCREEN) {
        None => problems.push(BundleProblem {
            file: SCREEN.to_string(),
            line: 1,
            message: "the bundle has no screen layout (hmi/screen.toml)".to_string(),
        }),
        Some(bytes) => match parse_screen(SCREEN, &String::from_utf8_lossy(bytes)) {
            Err(errors) => problems.extend(errors.into_iter().map(from_file_error)),
            Ok(screen) => {
                if let Some(start) = screen.start_page
                    && !bundle
                        .files
                        .contains_key(&format!("hmi/pages/{start}.toml"))
                {
                    problems.push(BundleProblem {
                        file: SCREEN.into(),
                        line: 1,
                        message: format!("start page {start} is missing"),
                    });
                }
            }
        },
    }
    let screen_size = bundle
        .files
        .get(SCREEN)
        .and_then(|bytes| parse_screen(SCREEN, &String::from_utf8_lossy(bytes)).ok())
        .map(|screen| screen.size);
    let docked = bundle.docked_panels();
    for (path, bytes) in &bundle.files {
        if !path.ends_with(".toml") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        // Pages and popups (§4.9) are checked alike; a docked panel as the page it joins.
        let loaded = if path.starts_with("hmi/pages/") {
            parse_page(path, &text)
        } else if path.starts_with("hmi/popups/") {
            parse_popup(path, &text).map(|popup| popup.page)
        } else if let Some((_, panel)) = docked.iter().find(|(name, _)| {
            path.strip_prefix("hmi/panels/")
                .and_then(|file| file.strip_suffix(".toml"))
                == Some(name.as_str())
        }) {
            Ok(panel.popup.page.clone())
        } else {
            continue;
        };
        match loaded {
            Ok(page) => {
                if let (Some(name), Some(size)) = (
                    path.strip_prefix("hmi/pages/")
                        .and_then(|file| file.strip_suffix(".toml")),
                    screen_size,
                ) {
                    // The docked panels must fit beside the page's own content (§21.1 item 3).
                    if let Err(message) = compose_docked(page.clone(), name, size, &docked) {
                        problems.push(BundleProblem {
                            file: path.clone(),
                            line: 1,
                            message,
                        });
                    }
                }
                problems.extend(check_file(
                    bundle, path, &text, &page, catalogue, program, policy, remote,
                ))
            }
            Err(errors) => problems.extend(errors.into_iter().map(from_file_error)),
        }
    }
    problems
}

/// The problems of one page or popup: images and click targets the bundle does not have,
/// bindings and permissions.
#[allow(clippy::too_many_arguments)]
fn check_file(
    bundle: &HmiBundle,
    path: &str,
    text: &str,
    page: &HmiPage,
    catalogue: &Catalogue,
    program: &Program<'_>,
    policy: &[WriteRule],
    remote: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Vec<BundleProblem> {
    let mut problems = Vec::new();
    if page.incomplete {
        problems.push(BundleProblem {
            file: path.to_string(),
            line: 1,
            message: "this page is marked incomplete; finish and validate it before deployment"
                .to_string(),
        });
    }
    for object in &page.objects {
        let target = match &object.click {
            Some(HmiClick::Page(name)) => Some(("page", format!("hmi/pages/{name}.toml"), name)),
            Some(HmiClick::Popup(name)) => Some(("popup", format!("hmi/popups/{name}.toml"), name)),
            _ => None,
        };
        if let Some((kind, file, name)) =
            target.filter(|(_, file, _)| !bundle.files.contains_key(file))
        {
            problems.push(BundleProblem {
                file: path.to_string(),
                line: click_line(text, &object.id),
                message: format!("the {kind} \"{name}\" is not in the bundle ({file})"),
            });
        }
        if let Some(image) = object
            .image
            .as_ref()
            .filter(|image| !bundle.files.contains_key(*image))
        {
            problems.push(BundleProblem {
                file: path.to_string(),
                line: object_key_line(text, &object.id, "image"),
                message: format!("the image \"{image}\" is not in the bundle"),
            });
        }
    }
    // A placed faceplate's faceplate and its fit to the instance are checked against the bundle's
    // faceplates and pairs (§8.9).
    let mut bindings =
        check_page_with_remote(page, catalogue, program.metadata, program.storage, remote);
    bindings.extend(check_placements(page, bundle, program.storage));
    let permissions = check_page_policy(page, catalogue, policy);
    problems.extend(super::panel_checks::check(
        bundle, path, text, page, catalogue, program, policy, remote,
    ));
    problems.extend(super::panel_checks::check_blocks(path, text, page, program));
    problems.extend(
        bindings
            .into_iter()
            .chain(permissions)
            .map(|problem| BundleProblem {
                file: path.to_string(),
                line: problem_line(text, &problem.object, problem.property.as_deref()),
                message: problem.message,
            }),
    );
    problems
}

fn from_file_error(error: PageFileError) -> BundleProblem {
    BundleProblem {
        file: error.file,
        line: error.line,
        message: error.message,
    }
}
