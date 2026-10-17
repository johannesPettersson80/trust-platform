//! A ring of bytes in fixed storage, oldest first.

/// At most `N` bytes, oldest first, kept inline: what a link's receive interrupt has handed over
/// and the loop has not taken yet. It never allocates.
#[derive(Debug, Clone)]
pub struct ByteRing<const N: usize> {
    bytes: [u8; N],
    /// The index of the oldest byte in `bytes`.
    head: usize,
    len: usize,
}

impl<const N: usize> ByteRing<N> {
    /// An empty ring.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            head: 0,
            len: 0,
        }
    }

    /// The bytes the ring holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the ring holds no byte.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the ring holds `N` bytes and takes no more.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// The bytes the ring still takes.
    #[must_use]
    pub const fn free(&self) -> usize {
        N - self.len
    }

    /// Appends as many of `bytes` as fit, in order, and returns how many it took.
    pub fn push(&mut self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(self.free());
        let tail = self.slot(self.len);
        let first = count.min(N - tail);
        self.bytes[tail..tail + first].copy_from_slice(&bytes[..first]);
        self.bytes[..count - first].copy_from_slice(&bytes[first..count]);
        self.len += count;
        count
    }

    /// The byte `offset` places after the oldest one, if the ring holds it.
    #[must_use]
    pub fn get(&self, offset: usize) -> Option<u8> {
        if offset < self.len {
            Some(self.bytes[self.slot(offset)])
        } else {
            None
        }
    }

    /// The offset from the oldest byte of the first `byte` at offset `from` or later.
    #[must_use]
    pub fn find(&self, byte: u8, from: usize) -> Option<usize> {
        let (front, back) = self.as_slices();
        front
            .iter()
            .chain(back)
            .skip(from)
            .position(|&held| held == byte)
            .map(|at| from + at)
    }

    /// Copies the oldest bytes into `out`, as many as `out` holds or the ring has, and returns how
    /// many it copied. The ring keeps them.
    pub fn copy_front(&self, out: &mut [u8]) -> usize {
        let (front, back) = self.as_slices();
        let first = out.len().min(front.len());
        let second = (out.len() - first).min(back.len());
        out[..first].copy_from_slice(&front[..first]);
        out[first..first + second].copy_from_slice(&back[..second]);
        first + second
    }

    /// Drops the oldest `count` bytes, or every byte when the ring holds fewer.
    pub fn discard(&mut self, count: usize) {
        let count = count.min(self.len);
        self.head = self.slot(count);
        self.len -= count;
    }

    /// Drops every byte.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// The bytes oldest first, as the two runs the ring stores them in; the second is empty
    /// unless the bytes wrap around the end of the storage.
    #[must_use]
    pub fn as_slices(&self) -> (&[u8], &[u8]) {
        let first = self.len.min(N - self.head);
        (
            &self.bytes[self.head..self.head + first],
            &self.bytes[..self.len - first],
        )
    }

    /// The index in `bytes` of the place `offset` after the oldest byte, for an offset of at most
    /// `N`.
    const fn slot(&self, offset: usize) -> usize {
        let at = self.head + offset;
        if at >= N {
            at - N
        } else {
            at
        }
    }
}

impl<const N: usize> Default for ByteRing<N> {
    fn default() -> Self {
        Self::new()
    }
}
