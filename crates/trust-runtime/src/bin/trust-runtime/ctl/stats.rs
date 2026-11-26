//! What `ctl stats` prints of a `tasks.stats` reply beyond its tasks: a debug image's Rust POUs
//! (specification 11, "Rust POU budgets").

use anyhow::Context;
use serde_json::Value as JsonValue;

/// The Rust POUs of a debug image in a `tasks.stats` reply, one line each in the order of the
/// image's Rust POU table (specification 11, "Rust POU budgets"); a reply without them, a release
/// image's or an earlier runtime's, prints none.
pub(super) fn print_rust_pous(response: &JsonValue) -> anyhow::Result<()> {
    let Some(pous) = response
        .get("result")
        .and_then(|result| result.get("rust_pous"))
        .and_then(JsonValue::as_array)
    else {
        return Ok(());
    };
    for pou in pous {
        let name = pou
            .get("name")
            .and_then(JsonValue::as_str)
            .context("Rust POU result missing string 'name'")?;
        let budget = pou
            .get("budget_ns")
            .and_then(JsonValue::as_i64)
            .context("Rust POU result missing integer 'budget_ns'")?;
        let longest = pou
            .get("longest_ns")
            .and_then(JsonValue::as_i64)
            .context("Rust POU result missing integer 'longest_ns'")?;
        let over = pou
            .get("over_budget")
            .and_then(JsonValue::as_u64)
            .context("Rust POU result missing non-negative integer 'over_budget'")?;
        outln!("rust_pou={name} budget_ns={budget} longest_ns={longest} over_budget={over}");
    }
    Ok(())
}
