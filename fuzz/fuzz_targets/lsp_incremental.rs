#![no_main]

use libfuzzer_sys::fuzz_target;
use trust_lsp::document_text::{
    apply_content_changes, ContentChange, DocumentPosition, DocumentRange,
};

const MAX_TEXT_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let body = &data[8..data.len().min(MAX_TEXT_BYTES + 8)];
    let content = String::from_utf8_lossy(body).into_owned();
    let change = ContentChange {
        range: Some(DocumentRange {
            start: DocumentPosition {
                line: u32::from(data[0]),
                character: u32::from(data[1]),
            },
            end: DocumentPosition {
                line: u32::from(data[2]),
                character: u32::from(data[3]),
            },
        }),
        text: String::from_utf8_lossy(&data[4..8]).into_owned(),
    };
    let full_sync = ContentChange {
        range: None,
        text: content.clone(),
    };
    let _ = apply_content_changes(&content, &[change]);
    let _ = apply_content_changes(&content, &[full_sync]);
});
