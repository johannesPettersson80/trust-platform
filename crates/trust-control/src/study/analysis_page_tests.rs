//! The analysis page of a study (§0.1a, `controlStudio/page`; A8.1): the tool the Tune page shows and its inputs are kept with the page.

use serde_json::{json, Value};

use super::{Page, Study};
use crate::messages::{AnalysisInputs, AnalysisPage, AnalysisTool, FrequencyGrid};

#[test]
fn the_study_keeps_the_analysis_tool_and_its_inputs_with_the_page() {
    let mut study = Study::new("tank", "revision-1");
    study.page = Page::Tune;
    study.analysis = Some(AnalysisPage {
        tool: AnalysisTool::Frequency,
        inputs: AnalysisInputs {
            grid: Some(FrequencyGrid {
                minimum_frequency: 0.1,
                maximum_frequency: 100.0,
                points: 128,
            }),
            ..AnalysisInputs::default()
        },
        model_revision: None,
    });
    let text = study.encode().expect("the study is stored");
    let stored = Study::decode(&text, "tank").expect("the study is read back");
    assert_eq!(stored.page, Page::Tune);
    assert_eq!(stored.analysis, study.analysis);
    let stored_text: Value = serde_json::from_str(&text).expect("the study is JSON");
    assert_eq!(stored_text["analysis"]["tool"], json!("frequency"));
    assert_eq!(
        stored_text["analysis"]["inputs"]["grid"]["points"],
        json!(128)
    );
}

#[test]
fn a_study_without_an_analysis_writes_no_analysis_and_an_older_study_reads_without_one() {
    let study = Study::new("tank", "revision-1");
    let text = study.encode().expect("the study is stored");
    assert!(
        !text.contains("analysis"),
        "an empty analysis page is not written: {text}"
    );
    let stored = Study::decode(&text, "tank").expect("the study is read back");
    assert_eq!(stored.analysis, None);
    assert_eq!(
        stored.page,
        Page::Identify,
        "a new study opens at Identify (A8.6)"
    );
}
