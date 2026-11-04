//! Opt-in response bounding for the agent methods whose payloads carry an
//! `issues` array (specification 20 §7): deterministic paging for
//! `lsp.diagnostics` and an `issues_limit` cap for `runtime.build`,
//! `runtime.validate`, and `runtime.compile_reload`. Omitted parameters
//! preserve the established whole-list responses; applied parameters select
//! windows of the same sorted lists and never reorder or drop beyond the
//! requested bound.

use super::AgentCommandError;
use serde_json::Value;

/// Validates the contract's paging parameter pair for `lsp.diagnostics`:
/// both or neither, and both at least 1 when present.
pub(crate) fn validate_page_params(
    page: Option<u64>,
    page_size: Option<u64>,
) -> Result<Option<(u64, u64)>, AgentCommandError> {
    match (page, page_size) {
        (None, None) => Ok(None),
        (Some(page), Some(page_size)) if page >= 1 && page_size >= 1 => Ok(Some((page, page_size))),
        (Some(_), Some(_)) => Err(AgentCommandError::invalid_params(
            "'page' and 'page_size' must both be at least 1 when present.",
        )),
        _ => Err(AgentCommandError::invalid_params(
            "'page' and 'page_size' must be given together.",
        )),
    }
}

/// Replaces `payload["issues"]` with the requested page of the deterministic
/// sort and adds `page`, `pageSize`, `totalIssues`, and `totalPages`. A page
/// beyond `totalPages` yields an empty list with the honest totals. Leaves
/// the payload untouched when it carries no `issues` array.
pub(crate) fn apply_issue_paging(payload: &mut Value, page: u64, page_size: u64) {
    let Some(issues) = payload.get_mut("issues").and_then(Value::as_array_mut) else {
        return;
    };
    let total_issues = issues.len() as u64;
    let total_pages = total_issues.div_ceil(page_size).max(1);
    let start = ((page - 1) * page_size).min(total_issues) as usize;
    let end = (start as u64 + page_size).min(total_issues) as usize;
    *issues = issues[start..end].to_vec();
    payload["page"] = Value::from(page);
    payload["pageSize"] = Value::from(page_size);
    payload["totalIssues"] = Value::from(total_issues);
    payload["totalPages"] = Value::from(total_pages);
}

/// Validates the contract's `issues_limit` cap: absent or at least 1.
pub(crate) fn validate_issue_limit(limit: Option<u64>) -> Result<Option<u64>, AgentCommandError> {
    match limit {
        None => Ok(None),
        Some(0) => Err(AgentCommandError::invalid_params(
            "'issues_limit' must be at least 1 when present.",
        )),
        Some(limit) => Ok(Some(limit)),
    }
}

/// Caps `payload["issues"]` at `limit` entries of the deterministic order and
/// adds `issuesTruncated: true` and `totalIssues`, but only when the list was
/// longer than the limit; otherwise the payload is left unchanged. Leaves the
/// payload untouched when it carries no `issues` array.
pub(crate) fn apply_issue_cap(payload: &mut Value, limit: u64) {
    let Some(issues) = payload.get_mut("issues").and_then(Value::as_array_mut) else {
        return;
    };
    let total_issues = issues.len() as u64;
    if limit >= total_issues {
        return;
    }
    issues.truncate(limit as usize);
    payload["issuesTruncated"] = Value::from(true);
    payload["totalIssues"] = Value::from(total_issues);
}

/// Bounds a raw multi-line diagnostic dump — a failed build projects the
/// compiler's `file: error[E…]: …` lines as one text — at the applied
/// `issues_limit`: the dump's first `limit` lines stand and one closing line
/// names the omitted remainder and the dump's true total. No limit, or a dump
/// no longer than the limit, returns the text unchanged.
pub(crate) fn bound_issue_lines(text: &str, limit: Option<u64>) -> String {
    let Some(limit) = limit else {
        return text.to_string();
    };
    let total_lines = text.lines().count() as u64;
    if total_lines <= limit {
        return text.to_string();
    }
    let shown = text
        .lines()
        .take(limit as usize)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{shown}\n… {} more diagnostic line(s) omitted by 'issues_limit' ({} total).",
        total_lines - limit,
        total_lines
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload_with_issues(count: usize) -> Value {
        json!({
            "target": "t",
            "issues": (0..count)
                .map(|i| json!({"message": format!("issue {i}")}))
                .collect::<Vec<_>>(),
        })
    }

    #[test]
    fn page_params_are_validated_as_a_pair_at_least_one() {
        assert!(validate_page_params(None, None).unwrap().is_none());
        assert_eq!(
            validate_page_params(Some(1), Some(50)).unwrap(),
            Some((1, 50))
        );
        let alone = validate_page_params(Some(1), None).unwrap_err();
        let zero = validate_page_params(Some(0), Some(5)).unwrap_err();
        let zero_size = validate_page_params(Some(1), Some(0)).unwrap_err();
        for error in [alone, zero, zero_size] {
            assert_eq!(error.code, -32602);
        }
    }

    #[test]
    fn paging_slices_the_sorted_list_and_reports_honest_totals() {
        let mut payload = payload_with_issues(5);
        apply_issue_paging(&mut payload, 2, 2);
        let issues = payload["issues"].as_array().unwrap();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0]["message"], "issue 2");
        assert_eq!(issues[1]["message"], "issue 3");
        assert_eq!(payload["page"], 2);
        assert_eq!(payload["pageSize"], 2);
        assert_eq!(payload["totalIssues"], 5);
        assert_eq!(payload["totalPages"], 3);
    }

    #[test]
    fn paging_beyond_the_last_page_is_empty_with_honest_totals() {
        let mut payload = payload_with_issues(5);
        apply_issue_paging(&mut payload, 9, 2);
        assert!(payload["issues"].as_array().unwrap().is_empty());
        assert_eq!(payload["totalIssues"], 5);
        assert_eq!(payload["totalPages"], 3);
    }

    #[test]
    fn paging_an_empty_list_reports_one_page() {
        let mut payload = payload_with_issues(0);
        apply_issue_paging(&mut payload, 1, 10);
        assert!(payload["issues"].as_array().unwrap().is_empty());
        assert_eq!(payload["totalIssues"], 0);
        assert_eq!(payload["totalPages"], 1);
    }

    #[test]
    fn paging_leaves_payloads_without_issues_untouched() {
        let mut payload = json!({"target": "t"});
        apply_issue_paging(&mut payload, 1, 10);
        assert!(payload.get("page").is_none());
    }

    #[test]
    fn issue_limit_is_validated_as_at_least_one() {
        assert!(validate_issue_limit(None).unwrap().is_none());
        assert_eq!(validate_issue_limit(Some(1)).unwrap(), Some(1));
        let zero = validate_issue_limit(Some(0)).unwrap_err();
        assert_eq!(zero.code, -32602);
    }

    #[test]
    fn cap_truncates_and_flags_only_when_exceeded() {
        let mut payload = payload_with_issues(5);
        apply_issue_cap(&mut payload, 3);
        assert_eq!(payload["issues"].as_array().unwrap().len(), 3);
        assert_eq!(payload["issuesTruncated"], true);
        assert_eq!(payload["totalIssues"], 5);

        let mut payload = payload_with_issues(3);
        apply_issue_cap(&mut payload, 3);
        assert_eq!(payload["issues"].as_array().unwrap().len(), 3);
        assert!(payload.get("issuesTruncated").is_none());
        assert!(payload.get("totalIssues").is_none());
    }

    #[test]
    fn cap_leaves_payloads_without_issues_untouched() {
        let mut payload = json!({"target": "t"});
        apply_issue_cap(&mut payload, 1);
        assert!(payload.get("issuesTruncated").is_none());
    }

    fn dump_with_lines(count: usize) -> String {
        (0..count)
            .map(|i| format!("src/a.st: error[E1]: failure {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn bound_issue_lines_without_a_limit_is_the_identity() {
        let text = dump_with_lines(4);
        assert_eq!(bound_issue_lines(&text, None), text);
    }

    #[test]
    fn bound_issue_lines_leaves_a_dump_within_the_limit_unchanged() {
        let text = dump_with_lines(3);
        assert_eq!(bound_issue_lines(&text, Some(3)), text);
        assert_eq!(bound_issue_lines(&text, Some(9)), text);
    }

    #[test]
    fn bound_issue_lines_truncates_and_names_the_omitted_remainder() {
        let text = dump_with_lines(4);
        let bounded = bound_issue_lines(&text, Some(2));
        let lines: Vec<&str> = bounded.lines().collect();
        assert_eq!(
            lines.len(),
            3,
            "first limit lines plus one marker: {bounded}"
        );
        assert_eq!(lines[0], "src/a.st: error[E1]: failure 0");
        assert_eq!(lines[1], "src/a.st: error[E1]: failure 1");
        assert_eq!(
            lines[2],
            "… 2 more diagnostic line(s) omitted by 'issues_limit' (4 total)."
        );
    }

    #[test]
    fn bound_issue_lines_keeps_an_empty_text_unchanged() {
        assert_eq!(bound_issue_lines("", Some(1)), "");
    }
}
