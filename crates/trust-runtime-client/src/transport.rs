//! The byte streams a control client talks over (specification 34, PORT-05, and specification 11
//! §6.8.1): a TCP or Unix socket to a runtime on a computer or a network, and a chip's USB serial
//! port ([`SerialStream`](crate::SerialStream)). [`ControlClient`](crate::ControlClient) runs its
//! requests and replies over any [`ByteStream`], so one piece of code serves every link.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// A byte stream to a runtime's control endpoint: one JSON request or reply per line, in both
/// directions.
pub trait ByteStream: Read + Write + Send + fmt::Debug {
    /// Bounds every following read and write by `timeout`: one that makes no progress for that
    /// long fails with [`std::io::ErrorKind::TimedOut`] or [`std::io::ErrorKind::WouldBlock`].
    ///
    /// # Errors
    /// The stream refuses the setting, for example for a zero `timeout`.
    fn set_timeout(&self, timeout: Duration) -> io::Result<()>;

    /// The longest frame that the link moves in either direction, in bytes without the line end
    /// (specification 34, PORT-05), or `None` for a link without such a bound, as trust-runtime's
    /// own sockets are.
    ///
    /// A link with a frame is a byte-stream link of a chip: the client never writes a request
    /// longer than the frame, reads no reply line longer than it, and skips the lines on the port
    /// that are no reply.
    fn frame_bytes(&self) -> Option<usize> {
        None
    }
}

impl ByteStream for TcpStream {
    fn set_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.set_read_timeout(Some(timeout))?;
        self.set_write_timeout(Some(timeout))
    }
}

#[cfg(unix)]
impl ByteStream for UnixStream {
    fn set_timeout(&self, timeout: Duration) -> io::Result<()> {
        self.set_read_timeout(Some(timeout))?;
        self.set_write_timeout(Some(timeout))
    }
}
