//! The variables a project's services may write, as `trust.toml` gives them (specification 48
//! RU-63): each `[services.<name>] writes`, in `trust.toml`'s order, and the refusal of a `writes`
//! that holds no paths. Every expected value is worked out from the specification.

use std::path::PathBuf;

use super::{service_writes, ServiceWrites};

/// A project folder holding `trust.toml` with `text`, or none.
fn project(text: Option<&str>) -> PathBuf {
    let root = trust_test_support::unique_temp_path("service-writes");
    std::fs::create_dir_all(&root).expect("the project folder");
    if let Some(text) = text {
        std::fs::write(root.join("trust.toml"), text).expect("trust.toml");
    }
    root
}

#[test]
fn each_services_writes_are_read_in_trust_tomls_order() {
    // recipe before dosing, as trust.toml gives them though not as the alphabet orders them; each
    // service's paths as written, in their order. monitor writes nothing and idle's `writes` is
    // empty: neither is a service that writes. Catches services sorted, paths respelled, and a
    // service without paths that the image would list.
    let root = project(Some(
        "[services.recipe]\nheartbeat = \"500ms\"\nwrites = [\"Main.target\", \"main.SPEED\"]\n\n\
         [services.monitor]\nheartbeat = \"1s\"\n\n\
         [services.idle]\nwrites = []\n\n\
         [services.dosing]\nwrites = [\"Dose\"]\n",
    ));
    assert_eq!(
        service_writes(&root),
        Ok(vec![
            ServiceWrites {
                name: "recipe".into(),
                writes: vec!["Main.target".into(), "main.SPEED".into()],
            },
            ServiceWrites {
                name: "dosing".into(),
                writes: vec!["Dose".into()],
            },
        ])
    );
}

#[test]
fn a_project_without_trust_toml_or_services_has_no_service_that_writes() {
    // Catches a build that refuses a Structured Text project without services.
    assert_eq!(service_writes(&project(None)), Ok(Vec::new()));
    assert_eq!(
        service_writes(&project(Some("[rust]\ncrate = \"rust\"\n"))),
        Ok(Vec::new())
    );
}

#[test]
fn a_writes_that_holds_no_paths_is_refused_naming_the_service() {
    // A text where an array belongs, a number among the paths and an empty path. Catches a
    // `writes` read as no writes, which would leave the service without the variables its author
    // gave it, and an empty path in the image.
    let refusal = "[services.recipe] writes is an array of the paths of the variables the service \
                   may write (specification 48 RU-63)";
    for writes in ["\"Main.target\"", "[\"Main.target\", 1]", "[\"\"]"] {
        let root = project(Some(&format!("[services.recipe]\nwrites = {writes}\n")));
        assert_eq!(service_writes(&root), Err(refusal.to_string()), "{writes}");
    }
}
