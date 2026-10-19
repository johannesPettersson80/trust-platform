//! The reply to a request on a link that moves frames of a bounded length (specification 34,
//! PORT-05). The response is written into parts of 4,096 bytes, each taken from the heap as the
//! writing reaches it. A reply that fits the frame becomes the one line that is sent. A longer one
//! is handed to the platform's transfer session in its parts, never joined, and answered with its
//! head. A reply that is longer than the session holds, or that the heap has no room for, is
//! refused, never cut and never an abort. The request was served in every case, so it is audited as
//! served and the work after its reply stays.

use alloc::string::{String, ToString};

use super::{audited, serialization_failed, serialize_response, PreparedLine};
use crate::ports::ControlHost;
use crate::protocol::{ControlRequest, ControlResponse, ReplyWriter, Written};
use crate::status::{self, EventRead};
use crate::transfer::{reserve_part, wire, PartReserve, ReplyHead, ReplyParts, TransferError};

/// How a reply is written, and what it is held to.
struct Limits {
    /// The longest frame the link moves.
    frame: usize,
    /// The longest reply the writer takes: the session's capacity, and at least the frame, so that a
    /// reply that fits the frame is never refused for the capacity (specification 34, PORT-05).
    bound: usize,
    /// The length named by the refusal of a reply above the bound: the session's capacity, or the
    /// frame when the platform keeps no session.
    limit: usize,
    /// How each part of the reply is taken from the heap.
    reserve: PartReserve,
}

impl Limits {
    /// The limits of a link of `frame`-byte frames served by `host`: the capacity and the way of
    /// taking parts of its transfer session, or the frame alone when it keeps no session and so can
    /// hold no reply.
    fn of<H: ControlHost>(host: &H, frame: usize) -> Self {
        match host.with_transfers(|session| (session.reply_capacity(), session.part_reserve())) {
            Ok((capacity, reserve)) => Self {
                frame,
                bound: capacity.max(frame),
                limit: capacity,
                reserve,
            },
            Err(_) => Self {
                frame,
                bound: frame,
                limit: frame,
                reserve: reserve_part,
            },
        }
    }

    /// The line to send for the reply `reply` to the request that `served` answers, or the head of
    /// the reply, or its refusal. `served` carries the request's id and audit record id.
    fn delivered<H: ControlHost>(
        &self,
        reply: ReplyWriter,
        served: &ControlResponse,
        host: &H,
    ) -> String {
        match reply.finish(self.frame) {
            Written::Line(line) => line,
            Written::Parts(parts) => self.held(parts, served, host),
            Written::Overflow => refusal(
                served,
                &TransferError::ReplyTooLong {
                    capacity: self.limit,
                },
            ),
            Written::NoRoom { written } => {
                refusal(served, &TransferError::NoRoomForReply { written })
            }
            Written::NotText(error) => serialization_failed(&error),
        }
    }

    /// The head of the reply `parts`, which the platform's session holds whole, answered with the id
    /// and audit record id of `served`. A reply the session cannot hold is refused with the
    /// session's own message. A session that cannot be locked holds nothing, so the reply is refused
    /// at the frame.
    fn held<H: ControlHost>(
        &self,
        parts: ReplyParts,
        served: &ControlResponse,
        host: &H,
    ) -> String {
        match host.with_transfers(|session| session.hold_reply(parts)) {
            Ok(Ok(head)) => self.head_line(&head, served, host),
            Ok(Err(error)) => refusal(served, &error),
            Err(_) => refusal(
                served,
                &TransferError::ReplyTooLong {
                    capacity: self.frame,
                },
            ),
        }
    }

    /// The head of a held reply as the line to send. A head that does not fit the frame cannot be
    /// sent, so nobody could fetch the reply: its parts are freed at once and the reply is refused.
    fn head_line<H: ControlHost>(
        &self,
        head: &ReplyHead,
        served: &ControlResponse,
        host: &H,
    ) -> String {
        let line = serialize_response(&served.replaced_by_result(wire::reply_head(head)));
        if line.len() <= self.frame {
            return line;
        }
        // Nothing to report: a session that cannot be locked holds nothing to free.
        let _ = host.with_transfers(|session| session.abort(head.transfer));
        refusal(
            served,
            &TransferError::ReplyTooLong {
                capacity: self.frame,
            },
        )
    }
}

/// The response that replaces a reply which cannot be delivered: the failure `error`, with the id
/// and audit record id of `served`. It is written whole, as a short line, whatever the frame.
fn refusal(served: &ControlResponse, error: &TransferError) -> String {
    serialize_response(&served.replaced_by_error(error.to_string()))
}

/// `response` for a link that moves frames of at most `frame` bytes: the line to send, and the work
/// to run after it is written. The response is written member by member into the reply's parts, so
/// it is the text that `serialize_response` writes, though it is never built whole.
pub(super) fn within<H: ControlHost>(
    mut response: ControlResponse,
    frame: usize,
    host: &H,
) -> PreparedLine {
    let after_response = response.after_response.take();
    let limits = Limits::of(host, frame);
    let mut reply = ReplyWriter::new(limits.bound, limits.reserve);
    // A refusal is recorded in the writer and answered by `delivered`, never lost.
    let _ = response.write_to(&mut reply);
    PreparedLine {
        response: limits.delivered(reply, &response, host),
        after_response,
    }
}

/// The reply to the admitted event read `request` for a link that moves frames of at most `frame`
/// bytes, written event by event into the reply's parts. Each part is taken as the events reach it,
/// so a short read takes one part, and a reply longer than the frame is held by the platform's
/// session when it has one. The read was served, so it is audited as served whether its reply is
/// delivered or refused, as `within` keeps the audit record of any reply it replaces.
pub(super) fn event_read_within<H: ControlHost>(
    read: EventRead,
    request: &ControlRequest,
    host: &H,
    client: Option<&str>,
    frame: usize,
) -> PreparedLine {
    let limits = Limits::of(host, frame);
    let mut reply = ReplyWriter::ok(request.id, limits.bound, limits.reserve);
    if let Err(refused) =
        status::write_event_read(read, request.id, request.params.as_ref(), host, &mut reply)
    {
        // The part the start of the reply took is freed before the refusal takes its own.
        drop(reply);
        return within(audited(host, request, client, refused), frame, host);
    }
    let served = audited(
        host,
        request,
        client,
        ControlResponse::ok(request.id, serde_json::Value::Null),
    );
    // A refusal is recorded in the writer and answered by `delivered`, never lost.
    let _ = served.write_ok_end(&mut reply);
    PreparedLine {
        response: limits.delivered(reply, &served, host),
        after_response: None,
    }
}
