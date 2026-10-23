//! DAP protocol framing IO.
//! - read_message: parse Content-Length payload
//! - write_message/write_message_locked: emit payload
//! - write_protocol_log: optional transcript logging

use std::io::{self, BufRead, BufWriter, Write};
use std::sync::{Arc, Mutex};

const CONTENT_LENGTH: &str = "Content-Length";

pub(super) fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':')
            && name.trim().eq_ignore_ascii_case(CONTENT_LENGTH)
            && let Ok(length) = value.trim().parse::<usize>()
        {
            content_length = Some(length);
        }
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;

    let mut buffer = vec![0u8; length];
    reader.read_exact(&mut buffer)?;
    let payload = String::from_utf8(buffer)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid utf-8 payload"))?;
    Ok(Some(payload))
}

pub(super) fn write_message<W: Write>(writer: &mut W, payload: &str) -> io::Result<()> {
    let length = payload.len();
    write!(writer, "Content-Length: {length}\r\n\r\n")?;
    writer.write_all(payload.as_bytes())?;
    writer.flush()
}

pub(super) fn write_message_locked(
    writer: &Arc<Mutex<BufWriter<io::Stdout>>>,
    payload: &str,
) -> io::Result<()> {
    let mut writer = writer
        .lock()
        .map_err(|_| io::Error::other("stdout lock poisoned"))?;
    write_message(&mut *writer, payload)
}

pub(super) fn write_protocol_log(
    logger: &Arc<Mutex<BufWriter<std::fs::File>>>,
    direction: &str,
    payload: &str,
) -> io::Result<()> {
    let mut logger = logger
        .lock()
        .map_err(|_| io::Error::other("dap log lock poisoned"))?;
    writeln!(logger, "{direction} {payload}")?;
    logger.flush()
}
