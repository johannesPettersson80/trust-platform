//! The engine's methods as `ControlStudio::execute` dispatches them (specification 35, §0.1a): the storage folder and the runtime that
//! a request names, a malformed request, the page last viewed (A8.1, A8.6), and the operations that `controlStudio/open` names while they
//! run (A8.6).

use std::fs;

use trust_control::study::{Page, Study};

use super::test_support::{example_project, identified_section, Ignore};
use super::*;

#[test]
fn an_empty_storage_folder_is_the_engines_journal_folder_and_a_named_one_is_kept() {
    // §0.1a and A8.3: the extension asks for the folder the command line uses by leaving the field empty.
    let environment = |name: &str| {
        matches!(name, "HOME" | "LOCALAPPDATA").then(|| std::ffi::OsString::from("/home/engineer"))
    };
    let mut request = json!({ "projectRoot": "/project", "storageFolder": "" });
    default_storage_folder(&mut request, &environment).expect("the folder is found");
    let expected = settings::default_journal_folder(Platform::current(), &environment)
        .expect("the folder is found");
    assert_eq!(request["storageFolder"], expected.display().to_string());
    let mut named = json!({ "storageFolder": "/journals" });
    default_storage_folder(&mut named, &environment).expect("kept");
    assert_eq!(named["storageFolder"], "/journals");
    let mut without = json!({ "projectRoot": "/project" });
    default_storage_folder(&mut without, &environment).expect("kept");
    assert_eq!(
        without,
        json!({ "projectRoot": "/project" }),
        "no field is added"
    );
}

#[test]
fn a_live_request_whose_connection_names_another_runtime_is_refused_before_it_is_decoded() {
    // A2.7: the loop is bound to `simulator` and the window selects `runtime:plc`. The engine refuses the request with the sentence
    // and the reconnect it offers, before the request is decoded, so the request has no other fields and still gets that answer.
    let studio = ControlStudio::new(Arc::new(Ignore));
    let other = json!({ "projectRoot": "/project", "runtimeId": "simulator", "loopId": "tank",
        "connection": { "endpoint": "tcp://127.0.0.1:9", "runtimeId": "runtime:plc" } });
    let answer = studio.execute(TUNE, &[other]).expect("answered");
    assert_eq!(answer["failure"]["cause"], "runtime", "{answer}");
    assert_eq!(
        answer["failure"]["nextAction"], "connect-runtime",
        "{answer}"
    );
    assert_eq!(
        answer["failure"]["message"],
        "Control Studio is connected to simulator; the selected runtime is runtime:plc.",
        "{answer}"
    );
    let unselected = json!({ "projectRoot": "/project", "runtimeId": "simulator", "loopId": "tank",
        "connection": { "endpoint": "tcp://127.0.0.1:9", "runtimeId": "" } });
    let answer = studio.execute(TUNE, &[unselected]).expect("answered");
    assert!(
        answer.to_string().contains("the selected runtime is none."),
        "{answer}"
    );
    let same = json!({ "projectRoot": "/project", "runtimeId": "simulator", "loopId": "tank",
        "connection": { "endpoint": "tcp://127.0.0.1:9", "runtimeId": "simulator" } });
    let answer = studio.execute(TUNE, &[same]).expect("answered");
    assert!(!answer.to_string().contains("is connected to"), "{answer}");
}

#[test]
fn a_malformed_request_or_an_unknown_operation_answers_with_a_failure() {
    let studio = ControlStudio::new(Arc::new(Ignore));
    let malformed = studio
        .execute(OPEN, &[json!({ "projectRoot": "/x" })])
        .expect("answered");
    assert!(
        malformed["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid arguments")),
        "{malformed}"
    );
    let unknown = studio
        .execute(STATUS, &[json!({ "operationId": "operation-0" })])
        .expect("answered");
    assert!(unknown.get("failure").is_some(), "{unknown}");
    assert!(
        studio.execute("textDocument/hover", &[]).is_none(),
        "another method is not Control Studio's"
    );
}

#[test]
fn the_page_last_viewed_is_recorded_and_opening_returns_it_and_starts_nothing() {
    let (root, example, request) = example_project("page");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(
        opened["result"]["page"], "Identify",
        "a new study opens at Identify (A8.6): {opened}"
    );
    let page = json!({
        "projectRoot": root.display().to_string(),
        "runtimeId": example.runtime_id,
        "loopId": example.id,
        "page": "Tune",
    });
    let answered = studio.execute(PAGE, &[page]).expect("answered");
    assert_eq!(answered, json!({ "result": {} }), "{answered}");
    let reopened = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(reopened["result"]["page"], "Tune", "{reopened}");
    assert_eq!(
        reopened["result"]["reattached"],
        json!([]),
        "setting the page starts no operation"
    );
    let study = storage::open_study(&root, &example.id, &example.revision).expect("study");
    assert_eq!(study.page, Page::Tune);
    assert!(
        study.sections.is_empty() && study.recordings.is_empty(),
        "setting the page changes nothing else"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn the_page_keeps_its_method_choice_as_view_state_and_a_request_without_a_view_keeps_it() {
    // §0.1a `controlStudio/page`, A8.6: the Identify page's Method, including the opened-file and supplied-model entries that start
    // no campaign, comes back on open; a later page change without a view keeps it.
    let (root, example, request) = example_project("page-view");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let page = |view: Option<Value>| {
        let mut body = json!({
            "projectRoot": root.display().to_string(),
            "runtimeId": example.runtime_id,
            "loopId": example.id,
            "page": "Identify",
        });
        if let Some(view) = view {
            body["view"] = view;
        }
        body
    };
    let answered = studio
        .execute(
            PAGE,
            &[page(Some(json!({ "identifyMethod": "supplied-model" })))],
        )
        .expect("answered");
    assert_eq!(answered, json!({ "result": {} }), "{answered}");
    let reopened = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(
        reopened["result"]["pageView"],
        json!({ "identifyMethod": "supplied-model" }),
        "{reopened}"
    );
    studio.execute(PAGE, &[page(None)]).expect("answered");
    let again = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(
        again["result"]["pageView"]["identifyMethod"], "supplied-model",
        "a request without a view keeps the recorded one: {again}"
    );
    let refused = studio
        .execute(
            PAGE,
            &[page(Some(json!({ "identifyMethod": "m".repeat(65) })))],
        )
        .expect("answered");
    assert!(refused.get("failure").is_some(), "{refused}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn an_unknown_page_is_a_failure_with_its_next_action() {
    let studio = ControlStudio::new(Arc::new(Ignore));
    let unknown = studio
        .execute(
            PAGE,
            &[json!({
                "projectRoot": "/x",
                "runtimeId": "simulator",
                "loopId": "tank",
                "page": "Monitor",
            })],
        )
        .expect("answered");
    assert!(
        unknown["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid arguments")),
        "{unknown}"
    );
    assert_eq!(unknown["failure"]["cause"], "implementation", "{unknown}");
    assert_eq!(unknown["failure"]["nextAction"], "none", "{unknown}");
    assert!(unknown.get("result").is_none(), "{unknown}");
}

#[test]
fn open_names_the_operations_the_loop_runs_while_they_run_and_forgets_them_once_they_end() {
    // A8.6: a reopened panel follows the operations that still run for the loop. An ended one is in the study, not in `running`.
    let (root, _example, request) = example_project("running-operations");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let (release, wait) = std::sync::mpsc::channel::<()>();
    let started = studio.operations().start("identify", move |_| {
        let _ = wait.recv();
        Ok(json!({}))
    });
    studio.remember_started(
        std::slice::from_ref(&request),
        &json!({ "operationId": started.operation_id }),
    );
    let running = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(
        running["result"]["running"],
        json!([started.operation_id]),
        "{running}"
    );
    release.send(()).expect("the operation is released");
    for _ in 0..500 {
        let ended = studio
            .operations()
            .status(&started.operation_id)
            .is_ok_and(|update| update.state != OperationState::Running);
        if ended {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let ended = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(ended["result"]["running"], json!([]), "{ended}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn an_operation_of_another_loop_is_not_named_by_this_loops_open() {
    let (root, _example, request) = example_project("running-other-loop");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let started = studio.operations().start("identify", |_| Ok(json!({})));
    let other = json!({ "projectRoot": root.display().to_string(), "loopId": "another-loop" });
    studio.remember_started(
        std::slice::from_ref(&other),
        &json!({ "operationId": started.operation_id }),
    );
    studio.remember_started(
        std::slice::from_ref(&request),
        &json!({ "operationId": "operation-unknown" }),
    );
    let opened = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(opened["result"]["running"], json!([]), "{opened}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn reset_answers_the_stage_evidence_of_the_fresh_study() {
    // A8.4, B9.4: after Reset the new study holds no model, candidate or passed validation, and the answer says so, so the page
    // takes that evidence from the engine rather than assuming it.
    let (root, example, request) = example_project("reset-evidence");
    let mut study = Study::new(&example.id, &example.revision);
    study
        .set_section(&identified_section(&example, false))
        .expect("the Identify section is kept");
    storage::save_study(&root, &study, &[]).expect("the study is kept");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut reset_request = request;
    reset_request["action"] = json!({ "action": "reset" });
    let answer = studio
        .execute(IDENTIFY, &[reset_request])
        .expect("answered");
    let evidence = &answer["result"]["stageEvidence"];
    assert_eq!(evidence["hasModel"], json!(false), "{answer}");
    assert_eq!(evidence["hasCandidate"], json!(false), "{answer}");
    assert_eq!(evidence["validationHasResult"], json!(false), "{answer}");
    assert_eq!(evidence["validationPassed"], json!(false), "{answer}");
    fs::remove_dir_all(&root).expect("clean up");
}
