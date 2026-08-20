use super::*;

#[test]
fn http_shutdown_returns_complete_correlated_acknowledgement() {
    let project = make_project("shutdown-acknowledgement");
    let state = control_state(source_fixture());
    let base = start_test_server(state.clone(), project);

    let body = ureq::post(&format!("{base}/api/control"))
        .header("Content-Type", "application/json")
        .send(r#"{"id":4201,"type":"shutdown"}"#)
        .expect("HTTP shutdown response")
        .body_mut()
        .read_to_string()
        .expect("read complete HTTP shutdown response");
    let response: Value = serde_json::from_str(&body).expect("parse HTTP shutdown response");

    assert_eq!(response["id"], json!(4201));
    assert_eq!(response["ok"], json!(true));
    assert_eq!(response["result"]["status"], json!("stopping"));
}
