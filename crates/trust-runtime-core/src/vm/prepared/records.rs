//! A table of a prepared program's fixed-width records (specification 34 MEM-14, the program image in
//! place): decoded on the heap, or read where the records lie in the program's image when a chip
//! maps it. Either way the heap holds the part of each record that a scan reads, which a scan
//! reaches by reference.

use alloc::vec::Vec;
use core::fmt::Debug;

use crate::error::RuntimeError;
use crate::memory::figure::{HeapBytes, HeapCount};
use crate::vm::module::invalid_bytecode;

/// A record of which a scan reads only a part: the record splits into that part and the rest, and
/// the two parts join to give the record back.
pub(crate) trait Split: Clone + PartialEq + Debug + HeapBytes {
    /// The part of the record that a scan reads.
    type Scan: Copy + Debug + HeapBytes;
    /// The record's other fields.
    type Rest: Debug + HeapBytes;
    /// The record's two parts.
    fn split(self) -> (Self::Scan, Self::Rest);
    /// The part of `record` that a scan reads.
    fn scan_part(record: &Self) -> Self::Scan;
    /// The record whose parts are `scan` and `rest`.
    fn join(scan: &Self::Scan, rest: &Self::Rest) -> Self;
}

/// A table of fixed-width records of type `T`, `WIDTH` bytes each in the program's image.
#[derive(Debug)]
pub(crate) enum Records<T: Split> {
    /// Every record decoded on the heap as its two parts, which take the bytes the record takes: the
    /// table of a program prepared from bytes, which the prepared program does not keep.
    Decoded(Vec<(T::Scan, T::Rest)>),
    /// The records where they lie in the program's mapped image from byte `first`; the records that
    /// preparation changed, decoded and in index order; and the part of each record that a scan
    /// reads.
    InImage {
        first: usize,
        changed: Vec<(u32, T)>,
        scan: Vec<T::Scan>,
    },
}

impl<T: Split> Records<T> {
    /// Every record of `records`, decoded, each as its two parts. The parts reuse the records'
    /// vector, which then holds exactly the records.
    pub(crate) fn decoded(records: Vec<T>) -> Self {
        let mut parts: Vec<(T::Scan, T::Rest)> = records.into_iter().map(Split::split).collect();
        parts.shrink_to_fit();
        Self::Decoded(parts)
    }

    /// The table of `records`, as preparation holds them, read where the records of `WIDTH` bytes
    /// from byte `first` lie in `image`: each record that differs from its record in the image,
    /// which preparation changed, is kept as preparation holds it, and the part of each record that
    /// a scan reads is kept on the heap.
    ///
    /// # Errors
    ///
    /// When the image does not hold the records, or the heap has no room for the table.
    pub(crate) fn in_image<const WIDTH: usize>(
        image: &[u8],
        first: usize,
        records: &[T],
        decode: fn(&[u8; WIDTH]) -> Option<T>,
    ) -> Result<Self, RuntimeError> {
        let missing =
            || invalid_bytecode(smol_str::SmolStr::new_static("records outside the image"));
        let room = |_| RuntimeError::PreparationLimit;
        let mut changed_count = 0;
        for (index, record) in records.iter().enumerate() {
            if read(image, first, index, decode).ok_or_else(missing)? != *record {
                changed_count += 1;
            }
        }
        let mut changed = Vec::new();
        changed.try_reserve_exact(changed_count).map_err(room)?;
        let mut scan = Vec::new();
        scan.try_reserve_exact(records.len()).map_err(room)?;
        for (index, record) in records.iter().enumerate() {
            if read(image, first, index, decode).as_ref() != Some(record) {
                let index = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
                changed.push((index, record.clone()));
            }
            scan.push(T::scan_part(record));
        }
        Ok(Self::InImage {
            first,
            changed,
            scan,
        })
    }

    /// Record `index`: joined from its decoded parts, as preparation changed it, or read where it
    /// lies in `image`.
    pub(crate) fn get<const WIDTH: usize>(
        &self,
        image: &[u8],
        index: usize,
        decode: fn(&[u8; WIDTH]) -> Option<T>,
    ) -> Option<T> {
        match self {
            Self::Decoded(parts) => parts.get(index).map(|(scan, rest)| T::join(scan, rest)),
            Self::InImage {
                first,
                changed,
                scan,
            } => {
                if index >= scan.len() {
                    return None;
                }
                let key = u32::try_from(index).ok()?;
                match changed.binary_search_by_key(&key, |(at, _)| *at) {
                    Ok(at) => Some(changed[at].1.clone()),
                    Err(_) => read(image, *first, index, decode),
                }
            }
        }
    }

    /// The part of record `index` that a scan reads, which the heap holds.
    #[inline]
    pub(crate) fn scan(&self, index: usize) -> Option<&T::Scan> {
        match self {
            Self::Decoded(parts) => parts.get(index).map(|(scan, _)| scan),
            Self::InImage { scan, .. } => scan.get(index),
        }
    }

    /// The number of records.
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Decoded(parts) => parts.len(),
            Self::InImage { scan, .. } => scan.len(),
        }
    }

    /// Replace record `index` with `record`, as a test that forges a prepared program does.
    #[cfg(test)]
    pub(crate) fn set(&mut self, index: usize, record: T) {
        match self {
            Self::Decoded(parts) => parts[index] = record.split(),
            Self::InImage { changed, scan, .. } => {
                scan[index] = T::scan_part(&record);
                let key = u32::try_from(index).expect("a record index");
                match changed.binary_search_by_key(&key, |(at, _)| *at) {
                    Ok(at) => changed[at].1 = record,
                    Err(at) => changed.insert(at, (key, record)),
                }
            }
        }
    }

    /// Replace every record with `records`, as a test that forges a prepared program does.
    #[cfg(test)]
    pub(crate) fn replace(&mut self, records: Vec<T>) {
        *self = Self::decoded(records);
    }
}

/// Record `index` of `WIDTH` bytes from byte `first` where it lies in `image`.
fn read<T, const WIDTH: usize>(
    image: &[u8],
    first: usize,
    index: usize,
    decode: fn(&[u8; WIDTH]) -> Option<T>,
) -> Option<T> {
    let start = index.checked_mul(WIDTH)?.checked_add(first)?;
    let bytes: &[u8; WIDTH] = image
        .get(start..start.checked_add(WIDTH)?)?
        .try_into()
        .ok()?;
    decode(bytes)
}

impl<T: Split> HeapBytes for Records<T> {
    /// A decoded table takes its records' parts; a table in the image takes the records that
    /// preparation changed and the part of each record that a scan reads.
    fn add_heap(&self, count: &mut HeapCount) {
        match self {
            Self::Decoded(parts) => count.of(parts),
            Self::InImage { changed, scan, .. } => {
                count.of(changed);
                count.of(scan);
            }
        }
    }
}
