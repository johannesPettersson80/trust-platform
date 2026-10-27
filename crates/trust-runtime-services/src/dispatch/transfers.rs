//! The transfer operations of specification 34 (PORT-05): `transfer.begin`, `transfer.part`,
//! `transfer.status`, `transfer.get` and `transfer.abort`, which move a request or a reply longer
//! than the link's frame in parts, and `program.active`, which reports the installed program's
//! identities. The rules are in [`crate::transfer`]; this module reads the parameters and answers
//! through the platform's session.

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use trust_runtime_core::native::signature::Prehash;

use serde_json::{json, Value};

use trust_runtime_core::scheduler::ResourceState;

use crate::ports::{ControlHost, ProgramIdentities, ProgramIdentity, RecordsPort};
use crate::protocol::{ControlRequest, ControlResponse};
use crate::resource::replies::resource_not_running;
use crate::status::{check_number_members, invalid_params, MemberKind};
use crate::transfer::wire::{self, WireError};
use crate::transfer::{Complete, TransferError, TransferId, TransferSession};

/// Answers one of the five transfer operations through the platform's session, or refuses it when
/// the platform keeps no session. `transfer.begin` and `transfer.part` go through the host, which
/// holds an upload in memory or, on a chip whose resource is Stopped, writes it into its program
/// store (specification 34 §5.1).
pub(super) fn answer_transfer<H: ControlHost>(
    request: &ControlRequest,
    host: &H,
) -> ControlResponse {
    let id = request.id;
    if let Err(refused) = host.with_transfers(|_| ()) {
        return refused.refusal(id);
    }
    let Some(params) = request.params.as_ref() else {
        return ControlResponse::error(id, "missing params".into());
    };
    let result = match request.r#type.as_str() {
        "transfer.begin" => begin(id, params, host),
        "transfer.part" => part(id, params, host),
        kind => host
            .with_transfers(|session| transfer_result(id, kind, params, session))
            .unwrap_or_else(|refused| Err(refused.refusal(id))),
    };
    match result {
        Ok(result) => ControlResponse::ok(id, result),
        Err(refusal) => refusal,
    }
}

/// The result of the transfer operation `kind` that reads or ends a transfer, or its refusal.
fn transfer_result(
    id: u64,
    kind: &str,
    params: &Value,
    session: &mut TransferSession,
) -> Result<Value, ControlResponse> {
    match kind {
        "transfer.status" => {
            let transfer = transfer_member(id, params)?;
            session
                .status(transfer)
                .map(wire::status_reply)
                .map_err(|error| session_refusal(id, &error))
        }
        "transfer.get" => get(id, params, session),
        "transfer.abort" => {
            let transfer = transfer_member(id, params)?;
            session
                .abort(transfer)
                .map(|()| wire::abort_reply())
                .map_err(|error| session_refusal(id, &error))
        }
        _ => Err(ControlResponse::error(id, "unsupported request".into())),
    }
}

/// `transfer.begin {bytes, sha256}`: opens an upload.
fn begin<H: ControlHost>(id: u64, params: &Value, host: &H) -> Result<Value, ControlResponse> {
    check_number_members(
        id,
        params,
        &[
            ("bytes", MemberKind::WholeNumber),
            ("sha256", MemberKind::Text),
            ("for", MemberKind::Text),
        ],
    )?;
    let request = wire::begin_request(params).map_err(|error| wire_refusal(id, &error))?;
    let opened = if request.firmware {
        // A chip writes its firmware only while the resource is Stopped, as it installs a program
        // (specification 34 TARGET-04; Appendix D.2: no flash is erased while a program runs).
        let state = host.resource_state();
        if !matches!(state, ResourceState::Stopped) {
            return Err(resource_not_running(id, state));
        }
        host.begin_firmware_upload(request.bytes, request.digest)
    } else {
        host.begin_upload(request.bytes, request.digest)
    };
    match opened {
        Ok(Ok(transfer)) => Ok(wire::begin_reply(transfer)),
        Ok(Err(refusal)) => Err(ControlResponse::error(id, refusal)),
        Err(refused) => Err(refused.refusal(id)),
    }
}

/// `transfer.part {transfer, offset, data, crc32}`: stores one part. The base64 of `data` is
/// borrowed from the request and decoded into the session's part buffer.
fn part<H: ControlHost>(id: u64, params: &Value, host: &H) -> Result<Value, ControlResponse> {
    check_number_members(
        id,
        params,
        &[
            ("transfer", MemberKind::Text),
            ("offset", MemberKind::WholeNumber),
            ("data", MemberKind::Text),
            ("crc32", MemberKind::Text),
        ],
    )?;
    let request = wire::part_request(params).map_err(|error| wire_refusal(id, &error))?;
    match host.receive_part(
        request.transfer,
        request.offset,
        request.encoded,
        request.crc32,
    ) {
        Ok(Ok(next_offset)) => Ok(wire::part_reply(next_offset)),
        Ok(Err(refusal)) => Err(ControlResponse::error(id, refusal)),
        Err(refused) => Err(refused.refusal(id)),
    }
}

/// `transfer.get {transfer, offset, len}`: reads up to a part of the held reply, and frees the reply
/// when the part reaches its last byte (specification 34, PORT-05).
fn get(id: u64, params: &Value, session: &mut TransferSession) -> Result<Value, ControlResponse> {
    check_number_members(
        id,
        params,
        &[
            ("transfer", MemberKind::Text),
            ("offset", MemberKind::WholeNumber),
            ("len", MemberKind::WholeNumber),
        ],
    )?;
    let request = wire::get_request(params).map_err(|error| wire_refusal(id, &error))?;
    let part = session
        .get(request.transfer, request.offset, request.len)
        .map_err(|error| session_refusal(id, &error))?;
    Ok(wire::get_reply(&part))
}

/// The `transfer` member of `transfer.status`, `transfer.abort` and `bytecode.reload {transfer}`.
pub(crate) fn transfer_member(id: u64, params: &Value) -> Result<TransferId, ControlResponse> {
    check_number_members(id, params, &[("transfer", MemberKind::Text)])?;
    wire::transfer_request(params).map_err(|error| wire_refusal(id, &error))
}

/// Takes the complete transfer `transfer`, which ends it (specification 34, PORT-05): the bytes of
/// an upload held in memory, or what the session kept of one written into the program store. An
/// incomplete or unknown transfer is refused with the session's own text, and stays as it is.
pub(crate) fn take_upload<H: ControlHost>(
    id: u64,
    transfer: TransferId,
    host: &H,
) -> Result<Complete, ControlResponse> {
    match host.with_transfers(|session| session.take_complete(transfer)) {
        Ok(Ok(complete)) => Ok(complete),
        Ok(Err(error)) => Err(ControlResponse::error(id, error.to_string())),
        Err(refused) => Err(refused.refusal(id)),
    }
}

/// Takes the bytes of the complete transfer `transfer`, held in memory, which ends it, as
/// [`take_upload`] does, with the hash of the bytes a program image's signature covers when the
/// upload is an image. An upload written into the program store, which only `program.install`
/// takes, is refused and stays open.
pub(crate) fn take_complete<H: ControlHost>(
    id: u64,
    transfer: TransferId,
    host: &H,
) -> Result<(Vec<u8>, Option<Prehash>), ControlResponse> {
    let in_store = host
        .with_transfers(|session| session.in_store(transfer))
        .map_err(|refused| refused.refusal(id))?;
    if in_store {
        return Err(ControlResponse::error(
            id,
            TransferError::InStore.to_string(),
        ));
    }
    match take_upload(id, transfer, host)? {
        Complete::Memory(bytes, prehash) => Ok((bytes, prehash)),
        Complete::Store(_) => Err(ControlResponse::error(
            id,
            TransferError::InStore.to_string(),
        )),
    }
}

/// The refusal of a request whose parameters the wire rules cannot read. Every runtime gives the
/// same text for them (see [`check_number_members`]).
pub(crate) fn wire_refusal(id: u64, error: &WireError) -> ControlResponse {
    let text = match error {
        WireError::Params(reason) => reason.clone(),
        WireError::Member { name, expected } => format!("{name} must be {expected}"),
    };
    invalid_params(id, &text)
}

/// The refusal of a session operation that the session refused: its own message.
fn session_refusal(id: u64, error: &TransferError) -> ControlResponse {
    ControlResponse::error(id, error.to_string())
}

/// `program.active`: the selected program and the program before it, each `null` when the store
/// holds none. A platform without a program store refuses it, and a store that cannot be read answers
/// with its failure.
pub(super) fn program_active<H: RecordsPort>(id: u64, host: &H) -> ControlResponse {
    match host.read_program_identities() {
        Ok(Some(identities)) => ControlResponse::ok(id, identities_json(identities)),
        Ok(None) => ControlResponse::error(id, "unsupported request".into()),
        Err(error) => ControlResponse::error(id, error),
    }
}

fn identities_json(identities: ProgramIdentities) -> Value {
    json!({
        "active": identity_json(identities.active),
        "previous": identity_json(identities.previous),
    })
}

fn identity_json(identity: Option<ProgramIdentity>) -> Value {
    identity.map_or(Value::Null, |identity| {
        json!({
            "sequence": identity.sequence,
            "program_sha256": wire::hex_text(&identity.program_sha256),
            "bytes": identity.bytes,
        })
    })
}

#[cfg(test)]
mod contract_tests;
