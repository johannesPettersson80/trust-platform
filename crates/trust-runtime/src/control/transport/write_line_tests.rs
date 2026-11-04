//! A control line goes out in one write (specification 11, "Runtime control request, job, and
//! state-surface contract").

use std::io::Write;

use super::write_line;

/// A writer that keeps each write it is given.
#[derive(Default)]
struct Writes(Vec<Vec<u8>>);

impl Write for Writes {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.push(buf.to_vec());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_line_and_its_newline_go_out_in_one_write() {
    // Catches a reply written as its JSON and then its newline: the TCP stack holds the newline
    // until the client acknowledges the JSON, 41 to 44 ms a request on Linux.
    let mut writes = Writes::default();
    write_line(&mut writes, r#"{"id":1,"ok":true}"#).expect("written");
    assert_eq!(writes.0, [b"{\"id\":1,\"ok\":true}\n".to_vec()]);
}
