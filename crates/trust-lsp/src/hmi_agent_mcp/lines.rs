//! The message lines of standard input (specification 43, §3.1): one message per line, ended by
//! `\n` or `\r\n`, at most the limit's bytes before the line end. A longer line is read to its end
//! without being kept, so the next line is read as the next message.

use std::io::{self, BufRead};

/// One line of input.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Line {
    /// The bytes of a line within the limit, without its line end.
    Text(Vec<u8>),
    /// A line longer than the limit; its bytes were not kept.
    TooLong,
}

/// Reads the next line of at most `limit` bytes before its line end, or `None` at the end of the
/// input. A last line without a line end is a line.
pub(crate) fn read_line(input: &mut impl BufRead, limit: usize) -> io::Result<Option<Line>> {
    let mut line = Vec::new();
    let mut too_long = false;
    let mut read_any = false;
    loop {
        let available = match input.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            if !read_any {
                return Ok(None);
            }
            break;
        }
        read_any = true;
        let newline = available.iter().position(|&byte| byte == b'\n');
        let part = &available[..newline.unwrap_or(available.len())];
        // One byte beyond the limit may be the `\r` of a `\r\n` line end.
        if !too_long && line.len() + part.len() > limit + 1 {
            too_long = true;
            line = Vec::new();
        }
        if !too_long {
            line.extend_from_slice(part);
        }
        let used = part.len() + usize::from(newline.is_some());
        input.consume(used);
        if newline.is_some() {
            break;
        }
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    Ok(Some(if too_long || line.len() > limit {
        Line::TooLong
    } else {
        Line::Text(line)
    }))
}
