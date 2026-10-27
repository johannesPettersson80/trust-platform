//! The paging of a variables listing: which children of a handle a `debug.variables` request lists
//! (specification 11 §6.13.3, "Variable pages"; specification 13, "Variables / Evaluate").
//!
//! The Debug Adapter Protocol pages the children of a container with the arguments `filter`,
//! `start` and `count` of its `variables` request. [`Page`] holds the three and cuts a list of
//! children to the window they name, before the children are projected, so that only the children
//! of the window take handles. Computers and chips list through the same functions, so both answer
//! a given page with the same entries.

use serde::Deserialize;

/// How many elements of an array a request lists when it names no `count`: the chunk in which
/// VS Code pages a large array (specification 11 §6.13.3, "Variable pages").
pub const PAGE_SIZE: usize = 100;

/// The kind of child variables a request asks for (DAP `VariablesArguments.filter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChildKind {
    /// The elements of an array.
    Indexed,
    /// Every other child: the members of a structure, the variables of a scope or of an instance,
    /// the target of a reference, and the areas and rows of the I/O scope.
    Named,
}

/// The window of the children that a request lists (specification 11 §6.13.3, "Variable pages").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Page {
    /// The kind of children to list, or both kinds when absent.
    pub filter: Option<ChildKind>,
    /// The index of the first child to list, among the children that `filter` keeps.
    pub start: usize,
    /// The most children to list. When it is absent or `0`, an array lists [`PAGE_SIZE`] elements
    /// and the named children are all listed.
    pub count: Option<usize>,
}

impl Page {
    /// The page that lists every child of either kind, however many there are.
    #[must_use]
    pub fn everything() -> Self {
        Self {
            filter: None,
            start: 0,
            count: Some(usize::MAX),
        }
    }

    /// `children`, the children of kind `kind` in their order, cut to this page: none when `filter`
    /// keeps the other kind, else the children from `start`, at most the page's count.
    pub fn select<I: Iterator>(
        self,
        kind: ChildKind,
        children: I,
    ) -> impl Iterator<Item = I::Item> {
        let (skip, take) = self.window(kind);
        children.skip(skip).take(take)
    }

    /// How many children of kind `kind` to skip, and the most to list after them.
    fn window(self, kind: ChildKind) -> (usize, usize) {
        if self.filter.is_some_and(|filter| filter != kind) {
            return (0, 0);
        }
        let take = match self.count {
            Some(count) if count > 0 => count,
            _ => match kind {
                ChildKind::Indexed => PAGE_SIZE,
                ChildKind::Named => usize::MAX,
            },
        };
        (self.start, take)
    }
}
