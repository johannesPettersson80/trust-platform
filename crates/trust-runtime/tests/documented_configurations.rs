//! The configuration examples of the public reference pages load as the pages write them (checklist
//! G21): a reader copies them, so each must pass the same loader the runtime uses.

use std::path::PathBuf;

use trust_runtime::simulation::SimulationConfig;

/// The text of the first ```toml block of the reference page `page` under docs/public.
fn first_toml_block(page: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/public")
        .join(page);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let start = text
        .find("```toml\n")
        .unwrap_or_else(|| panic!("{page} has no toml block"))
        + "```toml\n".len();
    let end = start
        + text[start..]
            .find("```")
            .unwrap_or_else(|| panic!("{page}'s toml block does not end"));
    text[start..end].to_string()
}

#[test]
fn the_simulation_reference_example_loads() {
    // docs/public/reference/config/simulation-toml.md: its example is a whole simulation.toml, the
    // plant, its coupling, its disturbances and its physics joint. Specification 11 (simulation
    // couplings): "`on_true` and `on_false` require a threshold"; specification 46, section 5.1:
    // validation resolves "feedback-target ownership conflicts", and the page: "Physics feedback
    // targets cannot conflict with coupling targets". Catches an example the loader refuses, as the
    // page's was twice: a coupling with `on_true` and `on_false` and no threshold ("coupling
    // on_true/on_false require threshold"), and a joint whose feedback %IW0 overlapped the
    // coupling's target %IX0.0 ("physics feedback target %IW0 conflicts with coupling target").
    let example = first_toml_block("reference/config/simulation-toml.md");
    let folder = trust_test_support::unique_temp_path("documented-simulation");
    std::fs::create_dir_all(&folder).expect("a folder");
    let path = folder.join("simulation.toml");
    std::fs::write(&path, &example).expect("the example written");
    let loaded = SimulationConfig::load(&path);
    let _ = std::fs::remove_dir_all(&folder);
    if let Err(error) = loaded {
        panic!("the reference page's simulation.toml example: {error}\n{example}");
    }
}
