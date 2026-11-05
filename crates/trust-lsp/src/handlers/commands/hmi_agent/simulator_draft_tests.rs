//! Tests of the draft a simulator journey exercises (specification 43, AT-08): its pages by page
//! name, and the refusals that end a journey before its first step, on a temporary project with a
//! compiling program and a draft composed through the authoring command.

use serde_json::Value;

use crate::state::ServerState;

use super::super::argument::Argument;
use super::super::error::{
    AgentError, COMPILATION_FAILED, CONFLICT, EXPIRED_DRAFT, PROJECT_UNAVAILABLE,
};
use super::super::simulator_project::JourneyProject;
use super::draft_models;

/// The journey's argument on `project`, as the authoring commands read it.
fn argument(project: &JourneyProject) -> Argument {
    Argument::read(vec![project.fields()]).expect("the argument")
}

/// The refusal of the draft `draft` at `revision` on `project`.
fn refused(
    state: &ServerState,
    project: &JourneyProject,
    draft: &str,
    revision: &str,
) -> AgentError {
    draft_models(state, &argument(project), draft, revision).expect_err("a refusal")
}

/// Catches: a page named by its file (`hmi/pages/settings.toml`) or with its extension
/// (`settings.toml`), which no step names, so that every step would find no draft page and end the
/// journey with `CONFLICT`; and a model that is not the page file's.
#[test]
fn the_draft_s_pages_are_named_by_their_file_stems() {
    let project = JourneyProject::new("stems");
    let state = ServerState::new();
    let draft = project.settings_draft(&state, "stems");
    let models = draft_models(&state, &argument(&project), &draft, "1").expect("the pages");
    let names: Vec<&str> = models.keys().map(String::as_str).collect();
    assert_eq!(names, ["settings"]);
    let settings = &models["settings"];
    assert_eq!(settings["title"], "Settings", "{settings}");
    let ids: Vec<&str> = settings["object"]
        .as_array()
        .expect("the page's objects")
        .iter()
        .filter_map(|object| object["id"].as_str())
        .collect();
    assert_eq!(ids, ["sp"], "{settings}");
}

/// Catches: a journey that exercises a draft composed against an older program, whose bindings
/// may no longer hold, here a program whose variable names did not change.
#[test]
fn a_changed_program_refuses_the_draft_with_conflict() {
    let project = JourneyProject::new("program");
    let state = ServerState::new();
    let draft = project.settings_draft(&state, "program");
    project.write(
        "src/main.st",
        "PROGRAM Main\nVAR Level : REAL := 2.0; END_VAR\nEND_PROGRAM\n",
    );
    let refusal = refused(&state, &project, &draft, "1");
    assert_eq!(refusal.code, CONFLICT);
    assert_eq!(
        refusal.message,
        "the program changed since this simulation proposal was created."
    );
}

/// Catches: a journey that exercises a draft while the program does not compile, so that the
/// program's revision cannot be known and the draft's bindings cannot be checked.
#[test]
fn a_program_that_does_not_compile_refuses_the_draft() {
    let project = JourneyProject::new("broken");
    let state = ServerState::new();
    let draft = project.settings_draft(&state, "broken");
    project.write(
        "src/main.st",
        "PROGRAM Main\nVAR Level : REAL; END_VAR\nLevel := ;\nEND_PROGRAM\n",
    );
    assert_eq!(
        refused(&state, &project, &draft, "1").code,
        COMPILATION_FAILED
    );
}

/// Catches: a journey run on a draft that does not exist, on a revision the draft no longer has
/// (the tool call named revision `2` of a draft at `1`), or on another workspace's draft.
#[test]
fn a_draft_the_journey_cannot_exercise_is_refused() {
    let project = JourneyProject::new("drafts");
    let state = ServerState::new();
    let draft = project.settings_draft(&state, "drafts");
    let unavailable = refused(
        &state,
        &project,
        "00000000-0000-4000-8000-000000000000",
        "1",
    );
    assert_eq!(unavailable.code, EXPIRED_DRAFT, "{unavailable}");
    let later = refused(&state, &project, &draft, "2");
    assert_eq!(
        (later.code, later.message.as_str()),
        (CONFLICT, "draft revision changed.")
    );
    let mut other = project.fields();
    other["workspaceId"] = Value::from("another-workspace");
    let other = Argument::read(vec![other]).expect("the argument");
    let theirs = draft_models(&state, &other, &draft, "1").expect_err("another workspace's draft");
    assert_eq!(theirs.code, PROJECT_UNAVAILABLE, "{theirs}");
}
