//! Bounded request admission and worker dispatch for the embedded web server.

#![allow(missing_docs)]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use crossbeam_channel::{bounded, Receiver, Sender};
use tiny_http::{Header, Method, Request, Response, StatusCode};

pub(super) const READ_WORKER_COUNT: usize = 4;
pub(super) const READ_QUEUE_CAPACITY: usize = 32;
pub(super) const BODY_WORKER_COUNT: usize = 4;
pub(super) const BODY_ADMISSION_LIMIT: usize = 4;

pub(super) type RequestHandler = Arc<dyn Fn(Request) + Send + Sync + 'static>;

pub(super) struct WebRequestDispatcher {
    read_sender: Sender<Request>,
    body_sender: Sender<Request>,
    body_permits: Arc<AtomicUsize>,
}

impl WebRequestDispatcher {
    pub(super) fn new(handler: RequestHandler) -> Self {
        let (read_sender, read_receiver) = bounded(READ_QUEUE_CAPACITY);
        spawn_workers(
            "trust-web-read",
            READ_WORKER_COUNT,
            read_receiver,
            Arc::clone(&handler),
            None,
        );

        let body_permits = Arc::new(AtomicUsize::new(BODY_ADMISSION_LIMIT));
        let (body_sender, body_receiver) = bounded(BODY_ADMISSION_LIMIT);
        spawn_workers(
            "trust-web-body",
            BODY_WORKER_COUNT,
            body_receiver,
            handler,
            Some(Arc::clone(&body_permits)),
        );

        Self {
            read_sender,
            body_sender,
            body_permits,
        }
    }

    pub(super) fn dispatch(&self, request: Request) {
        let has_transfer_encoding = request
            .headers()
            .iter()
            .any(|header| header.field.equiv("Transfer-Encoding"));
        if request_uses_body_lane(
            request.method(),
            request.body_length(),
            has_transfer_encoding,
        ) {
            self.dispatch_body(request);
        } else if let Err(error) = self.read_sender.try_send(request) {
            reject_busy(error.into_inner());
        }
    }

    fn dispatch_body(&self, request: Request) {
        if !try_acquire(&self.body_permits) {
            reject_busy(request);
            return;
        }

        if let Err(error) = self.body_sender.try_send(request) {
            self.body_permits.fetch_add(1, Ordering::Release);
            reject_busy(error.into_inner());
        }
    }
}

fn spawn_workers(
    name: &str,
    count: usize,
    receiver: Receiver<Request>,
    handler: RequestHandler,
    body_permits: Option<Arc<AtomicUsize>>,
) {
    for index in 0..count {
        let worker_name = format!("{name}-{index}");
        let receiver = receiver.clone();
        let handler = Arc::clone(&handler);
        let body_permits = body_permits.clone();
        thread::Builder::new()
            .name(worker_name.clone())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let _permit = body_permits.as_ref().map(BodyPermit::new);
                    if catch_unwind(AssertUnwindSafe(|| handler(request))).is_err() {
                        tracing::error!(worker = worker_name, "web request handler panicked");
                    }
                }
            })
            .expect("spawn bounded web request worker");
    }
}

fn request_uses_body_lane(
    method: &Method,
    body_length: Option<usize>,
    has_transfer_encoding: bool,
) -> bool {
    has_transfer_encoding
        || body_length.unwrap_or(0) > 0
        || !matches!(method, Method::Get | Method::Head | Method::Options)
}

fn try_acquire(permits: &AtomicUsize) -> bool {
    permits
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |available| {
            available.checked_sub(1)
        })
        .is_ok()
}

struct BodyPermit<'a> {
    permits: &'a AtomicUsize,
}

impl<'a> BodyPermit<'a> {
    fn new(permits: &'a Arc<AtomicUsize>) -> Self {
        Self { permits }
    }
}

impl Drop for BodyPermit<'_> {
    fn drop(&mut self) {
        self.permits.fetch_add(1, Ordering::Release);
    }
}

fn reject_busy(request: Request) {
    let response = Response::from_string(
        serde_json::json!({
            "ok": false,
            "denial_code": "server_busy",
            "error": "web request capacity exhausted",
        })
        .to_string(),
    )
    .with_status_code(StatusCode(503))
    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
    .with_header(Header::from_bytes("Retry-After", "1").unwrap())
    .with_header(Header::from_bytes("Connection", "close").unwrap());
    let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_lane_classification_is_conservative() {
        assert!(!request_uses_body_lane(&Method::Get, None, false));
        assert!(!request_uses_body_lane(&Method::Head, None, false));
        assert!(!request_uses_body_lane(&Method::Options, None, false));
        assert!(request_uses_body_lane(&Method::Get, Some(1), false));
        assert!(request_uses_body_lane(&Method::Get, None, true));
        assert!(request_uses_body_lane(&Method::Post, None, false));
        assert!(request_uses_body_lane(&Method::Put, Some(0), false));
        assert!(request_uses_body_lane(&Method::Delete, None, false));
    }

    #[test]
    fn body_permits_are_bounded_and_reusable() {
        let permits = Arc::new(AtomicUsize::new(1));
        assert!(try_acquire(&permits));
        assert!(!try_acquire(&permits));
        drop(BodyPermit::new(&permits));
        assert!(try_acquire(&permits));
    }
}
