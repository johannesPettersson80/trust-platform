//! `trust-runtime ctl` writes its request line in one write (specification 11, "Runtime control
//! request, job, and state-surface contract").

use super::*;

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
fn the_request_line_goes_out_in_one_write() {
    // Catches the request written as its JSON and then its newline: the TCP stack holds the
    // newline until the runtime acknowledges the JSON, 41 to 44 ms a request on Linux.
    let request = json!({"id": 1, "type": "status", "auth": null});
    let mut writes = Writes::default();
    let mut response = std::io::Cursor::new(b"{\"id\":1,\"ok\":true}\n");
    exchange_control_request(&mut writes, &mut response, &request).expect("a valid exchange");
    assert_eq!(
        writes.0,
        [b"{\"auth\":null,\"id\":1,\"type\":\"status\"}\n".to_vec()]
    );
}
