//! The start of a recording (runtime protocol annex, E4.2 and E11.4; specification 35, A4.2): the refusal that asks for a
//! fresh coherent inspection of symbolic bindings, and how many starts a recording sends before it gives up
//! (`start` of `runtimeRecording.ts`). The loop of inspections and starts stays in `trust-lsp`.

/// The most starts one recording sends while the runtime reports that no coherent inspection of its symbols is current
/// (E11.4).
pub const START_ATTEMPT_LIMIT: usize = 5;

/// The code of the refusal that asks for a fresh inspection (E4.2, E0.4).
pub const NO_COHERENT_INSPECTION_CODE: &str = "recording_unavailable";

/// The runtime's text of that refusal.
pub const NO_COHERENT_INSPECTION_TEXT: &str =
    "a current coherent inspection of these symbolic bindings is required";

/// Whether a refused start is the runtime's "no coherent inspection" refusal: its code and its text, as the runtime answers
/// it when no current coherent inspection of the symbolic bindings exists (E4.2).
#[must_use]
pub fn is_no_coherent_inspection(code: Option<&str>, text: &str) -> bool {
    code == Some(NO_COHERENT_INSPECTION_CODE) && text == NO_COHERENT_INSPECTION_TEXT
}

/// Whether a refused start is sent again after a fresh inspection. The attempt is counted from 0. Only a symbolic recording,
/// whose program revision the first inspection has fixed (`program_revision_known`), retries, only on the no-coherent-inspection
/// refusal, and only while attempts remain: the retries are at most `START_ATTEMPT_LIMIT - 1`, so the starts are at most
/// `START_ATTEMPT_LIMIT` (`startAttempt < 4` of `runtimeRecording.ts`).
#[must_use]
pub fn may_retry_start(
    attempt: usize,
    program_revision_known: bool,
    code: Option<&str>,
    text: &str,
) -> bool {
    program_revision_known
        && attempt + 1 < START_ATTEMPT_LIMIT
        && is_no_coherent_inspection(code, text)
}

#[cfg(test)]
mod tests {
    use super::{is_no_coherent_inspection, may_retry_start, START_ATTEMPT_LIMIT};

    const CODE: Option<&str> = Some("recording_unavailable");
    const TEXT: &str = "a current coherent inspection of these symbolic bindings is required";

    #[test]
    fn only_the_no_coherent_inspection_refusal_is_retried() {
        assert!(is_no_coherent_inspection(CODE, TEXT));
        assert!(
            !is_no_coherent_inspection(CODE, "Recording channels are limited to 32."),
            "another recording_unavailable refusal is not retried"
        );
        assert!(
            !is_no_coherent_inspection(Some("invalid_experiment"), TEXT),
            "another code is not retried"
        );
        assert!(
            !is_no_coherent_inspection(None, TEXT),
            "a refusal without its code is not retried"
        );
    }

    #[test]
    fn a_symbolic_start_retries_until_the_limit_of_five_starts() {
        assert_eq!(START_ATTEMPT_LIMIT, 5);
        for attempt in 0..START_ATTEMPT_LIMIT - 1 {
            assert!(
                may_retry_start(attempt, true, CODE, TEXT),
                "attempt {attempt} of five may be followed by a fresh inspection"
            );
        }
        assert!(
            !may_retry_start(START_ATTEMPT_LIMIT - 1, true, CODE, TEXT),
            "the fifth start is the last one"
        );
    }

    #[test]
    fn a_start_without_a_symbolic_program_revision_never_retries() {
        assert!(!may_retry_start(0, false, CODE, TEXT));
    }
}
