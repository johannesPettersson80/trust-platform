//! Source location utilities.

#![allow(missing_docs)]

use super::SourceLocation;

/// Resolve a line/column breakpoint to the nearest statement boundary.
///
/// Lines/columns are 0-based and measured in bytes.
pub fn resolve_breakpoint_location(
    source: &str,
    file_id: u32,
    statements: &[SourceLocation],
    line: u32,
    column: u32,
) -> Option<SourceLocation> {
    let line_starts = line_starts(source);
    let line_idx = usize::try_from(line).ok()?;
    if line_idx >= line_starts.len() {
        return None;
    }
    let line_start = line_starts[line_idx];
    let line_end = if line_idx + 1 < line_starts.len() {
        line_starts[line_idx + 1].saturating_sub(1)
    } else {
        source.len()
    };
    let column = usize::try_from(column).ok()?;
    let offset = (line_start + column).min(line_end);
    let offset_u32 = u32::try_from(offset).ok()?;

    let mut best_on_line_before: Option<(u32, SourceLocation)> = None;
    let mut best_on_line_after: Option<(u32, SourceLocation)> = None;
    for stmt in statements.iter().copied().filter(|s| s.file_id == file_id) {
        let stmt_start = usize::try_from(stmt.start).ok()?;
        let stmt_line_idx = match line_starts.binary_search(&stmt_start) {
            Ok(idx) => idx,
            Err(next) => next.saturating_sub(1),
        };
        if stmt_line_idx != line_idx {
            continue;
        }
        let stmt_col = stmt_start
            .checked_sub(line_starts[stmt_line_idx])
            .and_then(|col| u32::try_from(col).ok())
            .unwrap_or(0);
        if stmt_col <= column as u32 {
            match best_on_line_before {
                Some((best_col, _)) if stmt_col <= best_col => {}
                _ => best_on_line_before = Some((stmt_col, stmt)),
            }
        } else {
            match best_on_line_after {
                Some((best_col, _)) if stmt_col >= best_col => {}
                _ => best_on_line_after = Some((stmt_col, stmt)),
            }
        }
    }
    if let Some((_, stmt)) = best_on_line_before.or(best_on_line_after) {
        return Some(stmt);
    }

    let mut best_containing: Option<SourceLocation> = None;
    for stmt in statements.iter().copied().filter(|s| s.file_id == file_id) {
        if stmt.start <= offset_u32 && offset_u32 <= stmt.end {
            let span = stmt.end.saturating_sub(stmt.start);
            if best_containing
                .map(|current| span < current.end.saturating_sub(current.start))
                .unwrap_or(true)
            {
                best_containing = Some(stmt);
            }
        }
    }
    if best_containing.is_some() {
        return best_containing;
    }

    let mut next_stmt: Option<SourceLocation> = None;
    for stmt in statements.iter().copied().filter(|s| s.file_id == file_id) {
        if stmt.start >= offset_u32
            && next_stmt
                .map(|current| stmt.start < current.start)
                .unwrap_or(true)
        {
            next_stmt = Some(stmt);
        }
    }
    next_stmt
}

/// Convert a byte offset into a 0-based line/column.
#[must_use]
pub fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    let offset = usize::try_from(offset).unwrap_or(0);
    let line_starts = line_starts(source);
    let line_idx = match line_starts.binary_search(&offset) {
        Ok(idx) => idx,
        Err(next) => next.saturating_sub(1),
    };
    let line_start = line_starts.get(line_idx).copied().unwrap_or(0);
    let col = offset.saturating_sub(line_start);
    (
        u32::try_from(line_idx).unwrap_or(0),
        u32::try_from(col).unwrap_or(0),
    )
}

/// Convert a statement location into a 0-based line/column using its start offset.
#[must_use]
pub fn location_to_line_col(source: &str, location: &SourceLocation) -> (u32, u32) {
    offset_to_line_col(source, location.start)
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = Vec::with_capacity(128);
    starts.push(0);
    for (idx, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_breakpoint_prefers_statement_on_line() {
        let source = "IF x THEN\nEND_IF;\nOutSignal := 1;\n";
        let outer_start = source.find("IF").unwrap();
        let outer_end = source.len() - 1;
        let inner_start = source.find("OutSignal := 1;").unwrap();
        let inner_end = inner_start + "OutSignal := 1;".len();
        let line = 2;

        let statements = vec![
            SourceLocation::new(0, outer_start as u32, outer_end as u32),
            SourceLocation::new(0, inner_start as u32, inner_end as u32),
        ];

        let resolved = resolve_breakpoint_location(source, 0, &statements, line, 0).unwrap();
        assert_eq!(resolved.start, inner_start as u32);
    }
}
