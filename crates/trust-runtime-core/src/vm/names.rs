//! The module's names held compactly (specification 34 MEM-14): the container's string table as
//! one text buffer and the end offset of each entry, not one owned string per entry. An entry too
//! long for a short string's inline bytes also keeps one shared string, so that a scan that names
//! it shares that string instead of copying the name to the heap (MEM-09; C1-19c4).

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Index;

use smol_str::SmolStr;

use crate::error::RuntimeError;

/// The string table of a module: entry `i` is `text[ends[i - 1]..ends[i]]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    text: String,
    ends: Vec<u32>,
    /// Each entry that a short string holds on the heap, under its index, in index order.
    shared: Vec<(u32, SmolStr)>,
}

impl Names {
    /// The empty table.
    pub const fn new() -> Self {
        Self {
            text: String::new(),
            ends: Vec::new(),
            shared: Vec::new(),
        }
    }

    /// Copy `entries` into one buffer, reserving exactly the bytes they need.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::PreparationLimit`] when the buffer cannot be reserved or holds more than
    /// `u32::MAX` bytes.
    pub fn from_entries(entries: &[SmolStr]) -> Result<Self, RuntimeError> {
        let bytes = entries.iter().map(|entry| entry.len()).sum::<usize>();
        u32::try_from(bytes).map_err(|_| RuntimeError::PreparationLimit)?;
        let mut names = Self::default();
        names
            .text
            .try_reserve_exact(bytes)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        names
            .ends
            .try_reserve_exact(entries.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        let long = entries
            .iter()
            .filter(|entry| entry.is_heap_allocated())
            .count();
        names
            .shared
            .try_reserve_exact(long)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for entry in entries {
            if entry.is_heap_allocated() {
                // The container's own string, shared rather than copied.
                names.shared.push((names.ends.len() as u32, entry.clone()));
            }
            names.text.push_str(entry);
            names.ends.push(names.text.len() as u32);
        }
        Ok(names)
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.ends.len()
    }

    /// Whether the table has no entries.
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    /// Entry `index`, when the table holds it.
    pub fn get(&self, index: usize) -> Option<&str> {
        let end = *self.ends.get(index)? as usize;
        let start = match index {
            0 => 0,
            _ => self.ends[index - 1] as usize,
        };
        Some(&self.text[start..end])
    }

    /// Entry `index` as a short string, when the table holds it: a long entry's shared string,
    /// whose copy takes no heap, and a short entry held inline. A scan names a member with it.
    pub fn name(&self, index: usize) -> Option<SmolStr> {
        let entry = self.get(index)?;
        let at = u32::try_from(index).ok()?;
        Some(
            match self.shared.binary_search_by_key(&at, |&(shared, _)| shared) {
                Ok(found) => self.shared[found].1.clone(),
                Err(_) => SmolStr::new(entry),
            },
        )
    }

    /// The entries in table order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        (0..self.len()).map(|index| &self[index])
    }

    /// Append `name` and return its index, as a module built from source names a new string.
    pub(crate) fn push(&mut self, name: &str) -> u32 {
        let index = self.ends.len() as u32;
        let shared = SmolStr::new(name);
        if shared.is_heap_allocated() {
            self.shared.push((index, shared));
        }
        self.text.push_str(name);
        self.ends.push(self.text.len() as u32);
        index
    }

    /// Replace entry `index` with `name`, for tests that give a fixture an unusual name.
    #[cfg(test)]
    pub(crate) fn replace(&mut self, index: usize, name: &str) {
        let entries = self
            .iter()
            .enumerate()
            .map(|(at, entry)| SmolStr::new(if at == index { name } else { entry }))
            .collect::<Vec<_>>();
        *self = Self::from_entries(&entries).expect("a test table fits");
    }
}

impl Index<usize> for Names {
    type Output = str;

    fn index(&self, index: usize) -> &str {
        self.get(index).expect("the string index is in the table")
    }
}

impl<S: AsRef<str>> FromIterator<S> for Names {
    fn from_iter<I: IntoIterator<Item = S>>(entries: I) -> Self {
        let mut names = Self::default();
        for entry in entries {
            names.push(entry.as_ref());
        }
        names
    }
}

/// Order two names as IEC 61131-3 compares them: without regard to ASCII case.
pub(crate) fn compare_names(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_uppercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_uppercase()))
}

/// Ids by name (specification 34 MEM-14): each entry is a name's index in the module's [`Names`]
/// and its id, sorted by name without regard to ASCII case for a binary search. It replaces a hash
/// map keyed by an owned upper-case copy of each name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameIndex {
    entries: Vec<(u32, u32)>,
}

impl NameIndex {
    /// An empty index with room for `count` names.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::PreparationLimit`] when the room cannot be reserved.
    pub(crate) fn with_capacity(count: usize) -> Result<Self, RuntimeError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        Ok(Self { entries })
    }

    /// Add name `name_idx` of `names` with `id`; `false`, and nothing added, when the index already
    /// holds that name in any case.
    pub(crate) fn insert(&mut self, names: &Names, name_idx: u32, id: u32) -> bool {
        match self.position(names, &names[name_idx as usize]) {
            Ok(_) => false,
            Err(at) => {
                self.entries.insert(at, (name_idx, id));
                true
            }
        }
    }

    /// The id of `name`, in any case.
    pub(crate) fn get(&self, names: &Names, name: &str) -> Option<u32> {
        let at = self.position(names, name).ok()?;
        Some(self.entries[at].1)
    }

    /// Give name `name_idx` of `names` the id `id`, replacing the id of that name in any case, as a
    /// legacy module that defines a POU again does.
    #[cfg(feature = "hir")]
    pub(crate) fn set(&mut self, names: &Names, name_idx: u32, id: u32) {
        match self.position(names, &names[name_idx as usize]) {
            Ok(at) => self.entries[at] = (name_idx, id),
            Err(at) => self.entries.insert(at, (name_idx, id)),
        }
    }

    /// Each name's index and its id, in name order.
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = (u32, u32)> + '_ {
        self.entries.iter().copied()
    }

    fn position(&self, names: &Names, name: &str) -> Result<usize, usize> {
        self.entries
            .binary_search_by(|&(index, _)| compare_names(&names[index as usize], name))
    }
}

/// The methods of every class-like owner by name (specification 34 MEM-14): one array of each
/// method's owner, its name's index in the module's [`Names`] and its POU id, sorted by owner and
/// then by name without regard to ASCII case. It replaces a hash map per owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodIndex {
    entries: Vec<(u32, u32, u32)>,
}

impl MethodIndex {
    /// An empty index with room for `count` methods.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::PreparationLimit`] when the room cannot be reserved.
    pub(crate) fn with_capacity(count: usize) -> Result<Self, RuntimeError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        Ok(Self { entries })
    }

    /// Add method `name_idx` of `owner` with its POU `pou`; `false`, and nothing added, when the
    /// owner already has a method of that name in any case.
    pub(crate) fn insert(&mut self, names: &Names, owner: u32, name_idx: u32, pou: u32) -> bool {
        match self.position(names, owner, &names[name_idx as usize]) {
            Ok(_) => false,
            Err(at) => {
                self.entries.insert(at, (owner, name_idx, pou));
                true
            }
        }
    }

    /// The POU of `owner`'s method `name`, in any case.
    pub(crate) fn get(&self, names: &Names, owner: u32, name: &str) -> Option<u32> {
        let at = self.position(names, owner, name).ok()?;
        Some(self.entries[at].2)
    }

    /// Each method's owner, name index and POU, by owner and then by name.
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = (u32, u32, u32)> + '_ {
        self.entries.iter().copied()
    }

    fn position(&self, names: &Names, owner: u32, name: &str) -> Result<usize, usize> {
        self.entries.binary_search_by(|&(at, index, _)| {
            at.cmp(&owner)
                .then_with(|| compare_names(&names[index as usize], name))
        })
    }
}

/// An index of names read with the table that holds them, as a public accessor hands it out.
#[cfg(feature = "hir")]
#[derive(Debug, Clone, Copy)]
pub struct NameLookup<'a> {
    names: &'a Names,
    index: &'a NameIndex,
}

#[cfg(feature = "hir")]
impl<'a> NameLookup<'a> {
    pub(crate) fn new(names: &'a Names, index: &'a NameIndex) -> Self {
        Self { names, index }
    }

    /// The id of `name`, in any case.
    pub fn get(&self, name: &str) -> Option<u32> {
        self.index.get(self.names, name)
    }

    /// Each name and its id, in name order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&'a str, u32)> + 'a {
        let names = self.names;
        self.index
            .entries
            .iter()
            .map(move |&(index, id)| (&names[index as usize], id))
    }

    /// The number of names.
    pub fn len(&self) -> usize {
        self.index.entries.len()
    }

    /// Whether the index holds no names.
    pub fn is_empty(&self) -> bool {
        self.index.entries.is_empty()
    }
}

impl crate::memory::figure::HeapBytes for Names {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.text);
        count.of(&self.ends);
        count.of(&self.shared);
    }
}

impl crate::memory::figure::HeapBytes for NameIndex {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.entries);
    }
}

impl crate::memory::figure::HeapBytes for MethodIndex {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MEM-14: a name is read back from the one buffer exactly as the container's table held it,
    /// the empty name included. Catches an end offset that is off by one entry or one byte.
    #[test]
    fn names_read_back_from_one_buffer() {
        let entries = [
            SmolStr::new("Main"),
            SmolStr::new(""),
            SmolStr::new("A_name_longer_than_twenty_three_bytes"),
        ];
        let names = Names::from_entries(&entries).unwrap();
        assert_eq!(names.len(), 3);
        assert_eq!(names.get(0), Some("Main"));
        assert_eq!(names.get(1), Some(""));
        assert_eq!(&names[2], "A_name_longer_than_twenty_three_bytes");
        assert_eq!(names.get(3), None);
        // 4 + 0 + 37 bytes of text, nothing more.
        assert_eq!(names.text.len(), 41);
        assert_eq!(
            names.iter().collect::<Vec<_>>(),
            ["Main", "", entries[2].as_str()]
        );
    }

    /// MEM-14: an index finds each name in any case by a binary search of its sorted entries, and
    /// refuses a second name that differs only in case. Catches an index sorted by exact bytes,
    /// which misses "main" for "Main", and one that keeps a duplicate.
    #[test]
    fn a_name_index_finds_a_name_in_any_case_and_refuses_a_duplicate() {
        let names: Names = ["Pump", "Main", "MAIN", "valve"].into_iter().collect();
        let mut index = NameIndex::with_capacity(4).unwrap();
        assert!(index.insert(&names, 0, 10));
        assert!(index.insert(&names, 1, 20));
        assert!(!index.insert(&names, 2, 30));
        assert!(index.insert(&names, 3, 40));
        assert_eq!(index.get(&names, "main"), Some(20));
        assert_eq!(index.get(&names, "PUMP"), Some(10));
        assert_eq!(index.get(&names, "Valve"), Some(40));
        assert_eq!(index.get(&names, "Motor"), None);
        // In upper-case order: MAIN, PUMP, VALVE.
        assert_eq!(
            index.iter().collect::<Vec<_>>(),
            [(1, 20), (0, 10), (3, 40)]
        );
    }

    /// MEM-14: a method is found by its owner and its name in any case, and the same name under two
    /// owners is two methods. Catches a search that ignores the owner.
    #[test]
    fn a_method_index_finds_a_method_by_owner_and_name() {
        let names: Names = ["Start", "Stop", "START"].into_iter().collect();
        let mut methods = MethodIndex::with_capacity(4).unwrap();
        assert!(methods.insert(&names, 8, 0, 80));
        assert!(methods.insert(&names, 3, 0, 30));
        assert!(methods.insert(&names, 8, 1, 81));
        assert!(!methods.insert(&names, 8, 2, 82));
        assert_eq!(methods.get(&names, 8, "start"), Some(80));
        assert_eq!(methods.get(&names, 3, "START"), Some(30));
        assert_eq!(methods.get(&names, 3, "Stop"), None);
        assert_eq!(methods.get(&names, 8, "STOP"), Some(81));
    }

    /// MEM-09 and MEM-14: a name longer than a short string's 23 inline bytes is named by sharing
    /// one string, so that naming it in a scan copies nothing to the heap, and a short name is held
    /// inline, in a table read from a container and in one built name by name. Catches a lookup
    /// that copies a long name each time (the two names would lie at two addresses) and one that
    /// gives a long name's string to the wrong index.
    #[test]
    fn a_long_name_is_shared_and_a_short_one_held_inline() {
        let long = "a_member_whose_name_is_rather_long";
        for names in [
            Names::from_entries(&[SmolStr::new("level"), SmolStr::new(long)]).unwrap(),
            ["level", long].into_iter().collect::<Names>(),
        ] {
            let first = names.name(1).unwrap();
            let second = names.name(1).unwrap();
            assert_eq!(first, long);
            assert!(first.is_heap_allocated());
            assert_eq!(first.as_ptr(), second.as_ptr());
            let short = names.name(0).unwrap();
            assert_eq!(short, "level");
            assert!(!short.is_heap_allocated());
            assert_eq!(names.name(2), None);
        }
    }
}
