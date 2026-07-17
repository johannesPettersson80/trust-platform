//! UTF-16 document positions and incremental text edits.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentRange {
    pub start: DocumentPosition,
    pub end: DocumentPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentChange {
    pub range: Option<DocumentRange>,
    pub text: String,
}

#[derive(Debug)]
pub struct LineIndex<'a> {
    content: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    #[must_use]
    pub fn new(content: &'a str) -> Self {
        let mut line_starts = vec![0];
        let bytes = content.as_bytes();
        let mut offset = 0;
        while offset < bytes.len() {
            match bytes[offset] {
                b'\r' => {
                    offset += 1;
                    if bytes.get(offset) == Some(&b'\n') {
                        offset += 1;
                    }
                    line_starts.push(offset);
                }
                b'\n' => {
                    offset += 1;
                    line_starts.push(offset);
                }
                _ => offset += 1,
            }
        }
        Self {
            content,
            line_starts,
        }
    }

    #[must_use]
    pub fn offset_to_line_col(&self, offset: usize) -> (u32, u32) {
        let offset = self.clamp_to_char_boundary(offset);
        let line_idx = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line_idx];
        let offset = offset.min(self.line_end_without_newline(line_idx));
        let character = self.content[line_start..offset].encode_utf16().count() as u32;
        (line_idx as u32, character)
    }

    #[must_use]
    pub fn position_to_offset(&self, position: DocumentPosition) -> Option<usize> {
        let line_idx = position.line as usize;
        let line_start = *self.line_starts.get(line_idx)?;
        let line_end = self.line_end_without_newline(line_idx);
        let target = position.character;
        let mut character = 0u32;
        for (relative, ch) in self.content[line_start..line_end].char_indices() {
            if character == target {
                return Some(line_start + relative);
            }
            let next = character + ch.len_utf16() as u32;
            if target < next {
                return Some(line_start + relative);
            }
            character = next;
        }
        Some(line_end)
    }

    #[must_use]
    pub fn utf16_len_between(&self, start: usize, end: usize) -> u32 {
        let start = self.clamp_to_char_boundary(start);
        let end = self.clamp_to_char_boundary(end);
        if end <= start {
            return 0;
        }
        self.content[start..end].encode_utf16().count() as u32
    }

    fn line_end_without_newline(&self, line_idx: usize) -> usize {
        let line_start = self
            .line_starts
            .get(line_idx)
            .copied()
            .unwrap_or(self.content.len());
        let mut end = self
            .line_starts
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.content.len());
        if end > line_start && self.content.as_bytes().get(end - 1) == Some(&b'\n') {
            end -= 1;
        }
        if end > line_start && self.content.as_bytes().get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        end
    }

    fn clamp_to_char_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.content.len());
        while offset > 0 && !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

#[must_use]
pub fn apply_content_changes(content: &str, changes: &[ContentChange]) -> Option<String> {
    let mut updated = content.to_string();
    for change in changes {
        if let Some(range) = change.range {
            let index = LineIndex::new(&updated);
            let start = index.position_to_offset(range.start)?;
            let end = index.position_to_offset(range.end)?;
            if start > end || end > updated.len() {
                return None;
            }
            let mut next = String::with_capacity(
                updated.len().saturating_sub(end.saturating_sub(start)) + change.text.len(),
            );
            next.push_str(&updated[..start]);
            next.push_str(&change.text);
            next.push_str(&updated[end..]);
            updated = next;
        } else {
            updated = change.text.clone();
        }
    }
    Some(updated)
}
