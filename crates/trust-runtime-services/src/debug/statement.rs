//! Spans and statement identities, the coordinates a breakpoint matches (specification 13,
//! "Breakpoint evaluation, source resolution, and hook contract").
//!
//! A position is any ordered value. A runtime that holds the source text uses byte offsets; a chip
//! that holds only its `LINE_TABLE` uses `(line, column)` pairs (specification 12 §6.13). Within
//! one file the two orders are the same, so one matching rule serves both.

/// A half-open span `[start, end)` of positions in one source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span<P> {
    /// The first position inside the span.
    pub start: P,
    /// The position just after the span. It lies outside the span.
    pub end: P,
}

impl<P: Ord + Copy> Span<P> {
    /// The span from `start` up to, not including, `end`.
    #[must_use]
    pub fn new(start: P, end: P) -> Self {
        Self { start, end }
    }

    /// Whether a statement that starts at `statement_start` lies in this span, which is
    /// `start <= statement_start < end` (specification 13). A statement that starts at `end` only
    /// touches the span and does not match. A statement that starts inside the span matches even
    /// when it runs past `end`. An empty span matches nothing.
    #[must_use]
    pub fn contains_start(&self, statement_start: P) -> bool {
        self.start <= statement_start && statement_start < self.end
    }
}

/// The identity of a breakpoint's statement at one generation: its source file, the span of the
/// statement the breakpoint names, and the generation of the breakpoint set that names it. Cycle
/// suppression keys on it (specification 13: `(file_id,start,end,generation)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StatementId<P> {
    /// The source file (specification 13, "file ID"). A statement in another file never matches.
    pub file_id: u32,
    /// The span of the statement the breakpoint names.
    pub span: Span<P>,
    /// The generation of the breakpoint set. Replacing a file's breakpoints gives its new
    /// breakpoints a new generation (specification 13, "Runtime debug state, mutation, and
    /// projection invariants").
    pub generation: u64,
}

impl<P: Ord + Copy> StatementId<P> {
    /// Whether a statement that starts at `start` in `file_id` matches the breakpoint this identity
    /// names: it lies in the same file and starts inside the span (specification 13).
    #[must_use]
    pub fn matches(&self, file_id: u32, start: P) -> bool {
        self.file_id == file_id && self.span.contains_start(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_includes_its_start_and_excludes_its_end() {
        // Specification 13: the start is inclusive and the end is exclusive. Wrong behaviour
        // caught: an exclusive start misses the breakpoint's first position, and an inclusive end
        // matches the statement that follows the span.
        let span = Span::new(10_u32, 20);
        assert!(span.contains_start(10));
        assert!(span.contains_start(19));
        assert!(!span.contains_start(9));
        assert!(!span.contains_start(20));
    }

    #[test]
    fn an_empty_span_contains_no_start() {
        // Specification 13: a span from 10 to 10 is half-open and holds no position. Wrong
        // behaviour caught: an inclusive end, which matches the statement that starts at 10.
        assert!(!Span::new(10_u32, 10).contains_start(10));
    }

    #[test]
    fn an_enclosing_statement_that_starts_before_a_body_span_does_not_match_it() {
        // Specification 13: an enclosing statement starts before the statement the breakpoint
        // names, so it does not match. The FOR statement starts at 88 and its body span is
        // 131..149; the body starts at 131. Wrong behaviour caught: an overlap rule, which matched
        // the FOR and stopped at the loop header before the loop stored its temporaries.
        let body = Span::new(131_u32, 149);
        assert!(!body.contains_start(88));
        assert!(body.contains_start(131));
    }
}
