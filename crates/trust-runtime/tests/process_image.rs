//! The process-image area limit: an ST binding beyond it refuses the build.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::io::PROCESS_IMAGE_AREA_LIMIT;

#[test]
fn source_binding_above_process_image_cap_is_rejected() {
    let source = format!(
        r#"
PROGRAM Main
VAR
    marker AT %MB{} : BYTE;
END_VAR
END_PROGRAM
"#,
        PROCESS_IMAGE_AREA_LIMIT
    );

    let err = match TestHarness::from_source(&source) {
        Ok(_) => panic!("oversized ST binding should fail compilation"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("process image area limit"),
        "expected process-image cap error, got {err}"
    );
}
