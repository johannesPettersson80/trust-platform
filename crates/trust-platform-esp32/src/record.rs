//! Terminal `B1,FAIL` record: fixed size, sanitized code, no allocation.

const PREFIX: &[u8] = b"B1,FAIL,";
/// Longest code kept from the caller; a longer code is truncated, never split into records.
const CODE_MAX: usize = 64;
/// Largest encoded record: the prefix, the code and the CRLF terminator.
pub const FAIL_RECORD_MAX: usize = PREFIX.len() + CODE_MAX + 2;

/// Encode `B1,FAIL,<code>\r\n` into `out` and return its length. At most 64 code bytes are kept,
/// and each byte outside `[A-Za-z0-9_]` is written as `_`.
pub fn fail_record(code: &str, out: &mut [u8; FAIL_RECORD_MAX]) -> usize {
    let body = PREFIX
        .iter()
        .copied()
        .chain(code.bytes().take(CODE_MAX).map(sanitize));
    let mut len = 0;
    for byte in body.chain(*b"\r\n") {
        out[len] = byte;
        len += 1;
    }
    len
}

fn sanitize(byte: u8) -> u8 {
    if byte.is_ascii_alphanumeric() || byte == b'_' {
        byte
    } else {
        b'_'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(code: &str) -> ([u8; FAIL_RECORD_MAX], usize) {
        let mut out = [0u8; FAIL_RECORD_MAX];
        let len = fail_record(code, &mut out);
        (out, len)
    }

    #[test]
    fn plain_code_gives_one_crlf_terminated_record() {
        let (out, len) = encode("board-init");
        assert_eq!(&out[..len], b"B1,FAIL,board_init\r\n");
    }

    #[test]
    fn unsafe_characters_are_replaced_and_empty_code_is_allowed() {
        let (out, len) = encode("a,b\r\nc");
        assert_eq!(&out[..len], b"B1,FAIL,a_b__c\r\n");
        let (out, len) = encode("");
        assert_eq!(&out[..len], b"B1,FAIL,\r\n");
    }

    #[test]
    fn long_code_is_truncated_to_the_fixed_capacity() {
        let long = [b'x'; 200];
        let code = core::str::from_utf8(&long).unwrap();
        let (out, len) = encode(code);
        assert_eq!(len, FAIL_RECORD_MAX);
        assert_eq!(&out[..8], b"B1,FAIL,");
        assert_eq!(&out[8..72], &[b'x'; 64]);
        assert_eq!(&out[72..], b"\r\n");
    }
}
