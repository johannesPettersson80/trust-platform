//! The rules of the scan crate graph (specification 48 RU-09, RU-20): no crate of it but the SDK's
//! and those the SDK is built from has a build script or is a procedural macro, none of them holds
//! `unsafe` code, and none depends on a crate of the SDK that RU-12 does not publish for projects.
//! Since no procedural macro of the graph writes code, every crate's code comes from its source
//! text, so the build finds `unsafe` in the sources: a crate whose sources name it is refused,
//! naming the place. The graph is cargo's own, from its metadata of the build's package for the
//! module's target.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The SDK's crates that a crate outside the SDK may depend on, those RU-12 publishes for projects;
/// the others the SDK is built from serve the generated code (specification 48 RU-20).
const PUBLISHED: [&str; 4] = [
    "trust-plc",
    "trust-plc-macros",
    "trust-service",
    "trust-sim",
];

/// A package of the graph, as cargo's metadata describes it.
struct Package {
    name: String,
    version: String,
    manifest: PathBuf,
    /// The kinds of its targets: `lib`, `custom-build`, `proc-macro` and the others.
    kinds: Vec<String>,
    /// The folder of its library's sources.
    sources: Option<PathBuf>,
}

/// The rules the graph of the build's `metadata` breaks, each as a message: `sdk` is the folder of
/// the SDK's crates, `project` the folder of the project's crate, `bindings` the folder of layer 1
/// when the project's crate calls Structured Text, a package the build generates as it generates
/// its own (specification 48 RU-39).
pub(super) fn check(
    metadata: &Value,
    sdk: &Path,
    project: &Path,
    bindings: Option<&Path>,
) -> Vec<String> {
    let packages: HashMap<&str, Package> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|package| Some((package["id"].as_str()?, read_package(package)?)))
        .collect();
    let dependencies: HashMap<&str, Vec<(&str, bool)>> = metadata["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let deps = node["deps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|dep| {
                    let kinds = dep["dep_kinds"].as_array()?;
                    // A development dependency is not built into the module.
                    let built = kinds
                        .iter()
                        .any(|kind| kind["kind"].as_str() != Some("dev"));
                    Some((dep["pkg"].as_str()?, built))
                })
                .collect();
            Some((node["id"].as_str()?, deps))
        })
        .collect();
    // Cargo names each package by its canonical path; the folders may be named through a link
    // (macOS's temporary folder `/var` is `/private/var`), so both sides are compared canonical.
    let canonical =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let (sdk, project) = (canonical(sdk), canonical(project));
    let bindings = bindings.map(canonical);
    let under = |package: &Package, folder: &Path| {
        package
            .manifest
            .parent()
            .is_some_and(|parent| canonical(parent).starts_with(folder))
    };
    // The SDK's crates and every crate they are built from (RU-09's exception).
    let sdk_crates: Vec<&str> = packages
        .iter()
        .filter(|(_, package)| under(package, &sdk))
        .map(|(id, _)| *id)
        .collect();
    let mut exempt = closure(&sdk_crates, &dependencies);
    // Layer 1 is the generated code's, which the SDK's crates serve, as the build's own package
    // is; it holds no code of the project's but the Structured Text it generates.
    let layer_one: Vec<&str> = packages
        .iter()
        .filter(|(_, package)| {
            bindings
                .as_deref()
                .is_some_and(|folder| under(package, folder))
        })
        .map(|(id, _)| *id)
        .collect();
    exempt.extend(layer_one.iter().copied());
    // The project's crate and every crate it is built with. The build's own package, the generated
    // code that `cargo metadata` runs in, lies inside a Rust-first project's crate folder, the
    // project's own (RU-65), and is none of the project's crates.
    let generated = metadata["resolve"]["root"].as_str();
    let roots: Vec<&str> = packages
        .iter()
        .filter(|(id, package)| {
            Some(**id) != generated && !layer_one.contains(id) && under(package, &project)
        })
        .map(|(id, _)| *id)
        .collect();
    let mut errors = Vec::new();
    let mut ids: Vec<&str> = closure(&roots, &dependencies)
        .into_iter()
        .filter(|id| !exempt.contains(id))
        .collect();
    ids.sort_unstable();
    for id in ids {
        let Some(package) = packages.get(id) else {
            continue;
        };
        let named = format!("{} {}", package.name, package.version);
        if package.kinds.iter().any(|kind| kind == "custom-build") {
            errors.push(format!(
                "{named} has a build script, which would run code on this computer while the \
                 project builds: no crate of the scan crate graph but the SDK's has one \
                 (specification 48 RU-09)"
            ));
        }
        if package.kinds.iter().any(|kind| kind == "proc-macro") {
            errors.push(format!(
                "{named} is a procedural macro, which would run code on this computer while the \
                 project builds: no crate of the scan crate graph but the SDK's is one \
                 (specification 48 RU-09)"
            ));
        }
        for (dependency, built) in dependencies.get(id).into_iter().flatten() {
            let Some(internal) = packages
                .get(dependency)
                .filter(|_| *built && sdk_crates.contains(dependency))
            else {
                continue;
            };
            if !PUBLISHED.contains(&internal.name.as_str()) {
                errors.push(format!(
                    "{named} depends on {} {}, which the SDK is built from: a crate outside the SDK \
                     reaches it through `trust-plc` (specification 48 RU-20)",
                    internal.name, internal.version
                ));
            }
        }
        if let Some(sources) = &package.sources {
            for (file, line) in unsafe_places(sources) {
                errors.push(format!(
                    "{named} holds `unsafe` code at {}:{line}: no crate of the scan crate graph \
                     but the SDK's interface crate does (specification 48 RU-20)",
                    file.display()
                ));
            }
        }
    }
    errors
}

fn read_package(package: &Value) -> Option<Package> {
    let targets = package["targets"].as_array()?;
    let kinds: Vec<String> = targets
        .iter()
        .flat_map(|target| target["kind"].as_array().into_iter().flatten())
        .filter_map(|kind| kind.as_str().map(str::to_string))
        .collect();
    let sources = targets
        .iter()
        .find(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("lib")))
        })
        .and_then(|target| target["src_path"].as_str())
        .and_then(|path| Path::new(path).parent().map(Path::to_path_buf));
    Some(Package {
        name: package["name"].as_str()?.to_string(),
        version: package["version"].as_str()?.to_string(),
        manifest: PathBuf::from(package["manifest_path"].as_str()?),
        kinds,
        sources,
    })
}

/// The packages `roots` reach through the dependencies they are built with, `roots` with them.
fn closure<'a>(
    roots: &[&'a str],
    dependencies: &HashMap<&'a str, Vec<(&'a str, bool)>>,
) -> HashSet<&'a str> {
    let mut reached: HashSet<&str> = roots.iter().copied().collect();
    let mut queue: VecDeque<&str> = roots.iter().copied().collect();
    while let Some(id) = queue.pop_front() {
        for (dependency, built) in dependencies.get(id).into_iter().flatten() {
            if *built && reached.insert(dependency) {
                queue.push_back(dependency);
            }
        }
    }
    reached
}

/// Where the Rust sources below `folder` name `unsafe`: each file and line, in the order of the
/// files' paths. A token is read as the compiler reads it, so a comment or a string that says
/// "unsafe" is none.
fn unsafe_places(folder: &Path) -> Vec<(PathBuf, usize)> {
    let mut files = Vec::new();
    let mut folders = vec![folder.to_path_buf()];
    while let Some(current) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut places = Vec::new();
    for file in files {
        let Some(tokens) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| text.parse::<proc_macro2::TokenStream>().ok())
        else {
            continue;
        };
        let mut lines = Vec::new();
        unsafe_lines(tokens, &mut lines);
        places.extend(lines.into_iter().map(|line| (file.clone(), line)));
    }
    places
}

fn unsafe_lines(tokens: proc_macro2::TokenStream, lines: &mut Vec<usize>) {
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Ident(ident) if ident == "unsafe" => {
                lines.push(ident.span().start().line);
            }
            proc_macro2::TokenTree::Group(group) => unsafe_lines(group.stream(), lines),
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "crate_graph_tests.rs"]
mod tests;
