//! The program region of a heap (specification 44 §10.3, §12.3): memory the CPU executes, which
//! holds the blocks of programs, a module copied in to run there and the image an online change
//! uploads, whose module runs where it lies. Each block starts on a 64-byte boundary, the alignment
//! of a module's code (specification 44 §5.3). A block asked with that alignment, a module copy, is
//! placed as high as it fits, and any other block, an upload, as low as it fits: an installed
//! program's module then lies at the top while the first change's image lies at the bottom, and a
//! changed image and the next change's upload, which alternate, find room beside each other.

use core::cell::RefCell;

use critical_section::Mutex;

/// The alignment of every block of the region: the alignment of a module's code.
pub const BLOCK_ALIGN: usize = 64;

/// The most blocks the region holds at once: the module of the installed program, the image of
/// the program that runs and the upload of the next change, with room to spare.
const MAX_BLOCKS: usize = 8;

/// A block of the region: its offset from the region's start and its length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Block {
    offset: usize,
    len: usize,
}

/// The region's span and its blocks, ordered by offset.
#[derive(Debug)]
struct Blocks {
    start: usize,
    size: usize,
    list: [Block; MAX_BLOCKS],
    count: usize,
}

impl Blocks {
    /// The free span from `from` to the next block (or the end), for the gap before block `index`.
    fn gap(&self, index: usize) -> (usize, usize) {
        let from = if index == 0 {
            0
        } else {
            let before = self.list[index - 1];
            before.offset + before.len
        };
        let to = if index == self.count {
            self.size
        } else {
            self.list[index].offset
        };
        (from, to)
    }

    /// Where a block of `len` bytes goes: the lowest or the highest aligned offset that fits in a
    /// gap, with the index the block takes in the list.
    fn place(&self, len: usize, high: bool) -> Option<(usize, usize)> {
        let absolute = |offset: usize| self.start + offset;
        let fits = |index: usize| -> Option<usize> {
            let (from, to) = self.gap(index);
            if high {
                let end = absolute(to);
                let start = end.checked_sub(len)? & !(BLOCK_ALIGN - 1);
                (start >= absolute(from)).then(|| start - self.start)
            } else {
                let start = absolute(from).checked_add(BLOCK_ALIGN - 1)? & !(BLOCK_ALIGN - 1);
                (start.checked_add(len)? <= absolute(to)).then(|| start - self.start)
            }
        };
        if high {
            (0..=self.count)
                .rev()
                .find_map(|index| fits(index).map(|offset| (offset, index)))
        } else {
            (0..=self.count).find_map(|index| fits(index).map(|offset| (offset, index)))
        }
    }

    fn insert(&mut self, index: usize, block: Block) {
        self.list.copy_within(index..self.count, index + 1);
        self.list[index] = block;
        self.count += 1;
    }

    fn remove(&mut self, offset: usize) -> bool {
        let Some(index) = self.list[..self.count]
            .iter()
            .position(|block| block.offset == offset)
        else {
            return false;
        };
        self.list.copy_within(index + 1..self.count, index);
        self.count -= 1;
        self.list[self.count] = Block::default();
        true
    }
}

/// The program region of a heap; without [`ProgramRegion::init`] it holds nothing.
#[derive(Debug)]
pub struct ProgramRegion {
    blocks: Mutex<RefCell<Blocks>>,
}

impl ProgramRegion {
    /// A region without memory.
    pub const fn empty() -> Self {
        Self {
            blocks: Mutex::new(RefCell::new(Blocks {
                start: 0,
                size: 0,
                list: [Block { offset: 0, len: 0 }; MAX_BLOCKS],
                count: 0,
            })),
        }
    }

    /// Gives the region the `size` bytes at `start`.
    pub fn init(&self, start: usize, size: usize) {
        critical_section::with(|cs| {
            let mut blocks = self.blocks.borrow_ref_mut(cs);
            blocks.start = start;
            blocks.size = size;
            blocks.count = 0;
        });
    }

    /// Whether the region has memory.
    pub fn is_set(&self) -> bool {
        critical_section::with(|cs| self.blocks.borrow_ref(cs).size != 0)
    }

    /// The region's bytes.
    pub fn capacity(&self) -> usize {
        critical_section::with(|cs| self.blocks.borrow_ref(cs).size)
    }

    /// Whether `address` lies in the region.
    pub fn contains(&self, address: usize) -> bool {
        critical_section::with(|cs| {
            let blocks = self.blocks.borrow_ref(cs);
            blocks.size != 0 && address >= blocks.start && address - blocks.start < blocks.size
        })
    }

    /// The address of a new block of `len` bytes, placed as high as it fits when `high` is set and
    /// as low as it fits otherwise, on a [`BLOCK_ALIGN`] boundary; `None` when no gap holds it or
    /// the region holds its most blocks.
    pub fn allocate(&self, len: usize, high: bool) -> Option<usize> {
        critical_section::with(|cs| {
            let mut blocks = self.blocks.borrow_ref_mut(cs);
            if len == 0 || blocks.size == 0 || blocks.count == MAX_BLOCKS {
                return None;
            }
            let (offset, index) = blocks.place(len, high)?;
            blocks.insert(index, Block { offset, len });
            Some(blocks.start + offset)
        })
    }

    /// Frees the block at `address`; `false` when no block starts there.
    pub fn free(&self, address: usize) -> bool {
        critical_section::with(|cs| {
            let mut blocks = self.blocks.borrow_ref_mut(cs);
            let Some(offset) = address.checked_sub(blocks.start) else {
                return false;
            };
            blocks.remove(offset)
        })
    }

    /// The bytes the region's blocks take.
    pub fn used(&self) -> usize {
        critical_section::with(|cs| {
            let blocks = self.blocks.borrow_ref(cs);
            blocks.list[..blocks.count]
                .iter()
                .map(|block| block.len)
                .sum()
        })
    }
}

#[cfg(test)]
mod tests;
