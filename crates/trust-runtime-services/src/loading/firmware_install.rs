//! `firmware.install {transfer}`: the request rule of a firmware update (specification 34 TARGET-04;
//! specification 11, the firmware update contract).
//!
//! [`request`] refuses what the resource state or the platform does not allow, and takes the
//! complete transfer only after those checks pass: an upload that `transfer.begin {"for":
//! "firmware"}` wrote into the firmware slot the chip does not run from. The chip then checks the
//! image where it lies, and the loop commits it in steps: the image's first bytes, their read-back,
//! and the selection of the slot for the next boot on trial. The reply waits for the last step, and
//! the chip resets into the new firmware once the reply is sent. Nothing here holds a flash or a
//! clock.

use alloc::string::ToString;

use serde_json::Value;
use trust_runtime_core::scheduler::ResourceState;

use crate::dispatch::transfers::{take_upload, transfer_member};
use crate::firmware::installed_reply;
use crate::ports::ControlHost;
use crate::protocol::ControlResponse;
use crate::resource::replies::{hung_refusal, resource_not_running};
use crate::transfer::Complete;

/// The refusal of a transfer that is no firmware upload.
pub const NOT_FIRMWARE: &str =
    "the transfer is not a firmware upload: begin it with \"for\": \"firmware\"";

/// Answers `firmware.install {transfer}`. The request pipeline refuses every role below Admin before
/// this runs.
///
/// The first failing check is the reply, in this order: the params (`missing params`, then the
/// `transfer` member's refusal); a hung resource; a resource that is not Stopped, refused as
/// `resource_not_running` with its state; a runtime without firmware slots, which answers
/// `unsupported request`; a transfer that is not an open firmware upload (the session's own text for
/// an unknown one); the complete transfer; the chip's check of the image; and the commit. The
/// transfer is taken only once those checks have passed, so a refusal before it leaves it open.
#[must_use]
pub fn request<H: ControlHost>(id: u64, params: Option<&Value>, host: &H) -> ControlResponse {
    let Some(params) = params else {
        return ControlResponse::error(id, "missing params".to_string());
    };
    let transfer = match transfer_member(id, params) {
        Ok(transfer) => transfer,
        Err(refusal) => return refusal,
    };
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if !matches!(resource, ResourceState::Stopped) {
        return resource_not_running(id, resource);
    }
    if !host.has_firmware_slots() {
        return ControlResponse::error(id, "unsupported request".to_string());
    }
    if !host.is_firmware_upload(transfer) {
        // An unknown transfer is refused with the session's own text.
        return match host.with_transfers(|session| session.status(transfer).map(|_| ())) {
            Ok(Ok(())) => ControlResponse::error(id, NOT_FIRMWARE.to_string()),
            Ok(Err(error)) => ControlResponse::error(id, error.to_string()),
            Err(refused) => refused.refusal(id),
        };
    }
    let upload = match take_upload(id, transfer, host) {
        Ok(Complete::Store(upload)) => upload,
        Ok(Complete::Memory(..)) => return ControlResponse::error(id, NOT_FIRMWARE.to_string()),
        Err(refusal) => return refusal,
    };
    match host.commit_firmware(upload) {
        Ok((image, slot)) => ControlResponse::ok(id, installed_reply(&image, slot)),
        Err(error) => ControlResponse::error(id, error),
    }
}
