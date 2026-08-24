//! Private AMS/TCP loopback client used by the ADS server Doctor.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use trust_ads_core::SymbolDescriptor;
use trust_ads_server::{
    ams_net_id_text_to_bytes, AdsErrorCode, AmsHeader, AmsState, AmsTcpFrame, CommandId,
    ADSIGRP_SUMUP_READ, ADSIGRP_SYM_HNDBYNAME, ADSTRANS_SERVERCYCLE, AMS_HEADER_LEN,
    AMS_TCP_HEADER_LEN,
};

use super::super::AdsServerRuntimeConfig;

const LOOPBACK_IO_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) struct LoopbackAdsClient {
    stream: TcpStream,
    target_net_id: [u8; 6],
    target_port: u16,
    source_net_id: [u8; 6],
    source_port: u16,
    max_frame_bytes: usize,
    next_invoke: u32,
}

impl LoopbackAdsClient {
    pub(super) fn connect(
        addr: SocketAddr,
        config: &AdsServerRuntimeConfig,
        source_net_id: &str,
        source_port: u16,
    ) -> Result<Self, String> {
        let target = config
            .ams_net_id
            .as_ref()
            .ok_or_else(|| "runtime.ads_server.ams_net_id is missing".to_string())?;
        let target_net_id = ams_net_id_text_to_bytes(target.0.as_str())
            .map_err(|error| format!("parse target AMS Net ID: {error}"))?;
        let source_net_id = ams_net_id_text_to_bytes(source_net_id)
            .map_err(|error| format!("parse self-test source AMS Net ID: {error}"))?;
        let stream = TcpStream::connect_timeout(&addr, LOOPBACK_IO_TIMEOUT)
            .map_err(|error| format!("connect {addr}: {error}"))?;
        stream
            .set_read_timeout(Some(LOOPBACK_IO_TIMEOUT))
            .map_err(|error| format!("set read timeout: {error}"))?;
        stream
            .set_write_timeout(Some(LOOPBACK_IO_TIMEOUT))
            .map_err(|error| format!("set write timeout: {error}"))?;
        Ok(Self {
            stream,
            target_net_id,
            target_port: config.ads_port,
            source_net_id,
            source_port,
            max_frame_bytes: config.max_frame_bytes,
            next_invoke: 1,
        })
    }

    pub(super) fn read_state(&mut self) -> Result<(), String> {
        let response = self.request(CommandId::ReadState, Vec::new())?;
        expect_payload_result(&response.payload, AdsErrorCode::NoError)?;
        if response.payload.len() != 8 {
            return Err(format!(
                "ReadState response had {} bytes, expected 8.",
                response.payload.len()
            ));
        }
        Ok(())
    }

    pub(super) fn handle_by_name(&mut self, symbol: &str) -> Result<u32, String> {
        let mut write_data = symbol.as_bytes().to_vec();
        write_data.push(0);
        let mut payload = Vec::new();
        payload.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        payload.extend_from_slice(&(write_data.len() as u32).to_le_bytes());
        payload.extend(write_data);
        let response = self.request(CommandId::ReadWrite, payload)?;
        let data = expect_read_payload(&response.payload, 4)?;
        let handle = u32::from_le_bytes(
            data.try_into()
                .map_err(|_| "handle response was not four bytes".to_string())?,
        );
        if handle == 0 {
            return Err("symbol handle response returned zero".to_string());
        }
        Ok(handle)
    }

    pub(super) fn direct_read(&mut self, symbol: &SymbolDescriptor) -> Result<Vec<u8>, String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&symbol.byte_size.to_le_bytes());
        let response = self.request(CommandId::Read, payload)?;
        expect_read_payload(&response.payload, symbol.byte_size as usize)
    }

    pub(super) fn direct_read_expect_denied(
        &mut self,
        symbol: &SymbolDescriptor,
    ) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&symbol.byte_size.to_le_bytes());
        let response = self.request(CommandId::Read, payload)?;
        expect_payload_result(&response.payload, AdsErrorCode::AccessDenied)
    }

    pub(super) fn sumup_read(&mut self, symbol: &SymbolDescriptor) -> Result<(), String> {
        let mut item = Vec::new();
        item.extend_from_slice(&symbol.index_group.to_le_bytes());
        item.extend_from_slice(&symbol.index_offset.to_le_bytes());
        item.extend_from_slice(&symbol.byte_size.to_le_bytes());
        let mut payload = Vec::new();
        payload.extend_from_slice(&ADSIGRP_SUMUP_READ.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&(4 + symbol.byte_size).to_le_bytes());
        payload.extend_from_slice(&(item.len() as u32).to_le_bytes());
        payload.extend(item);
        let response = self.request(CommandId::ReadWrite, payload)?;
        let data = expect_read_payload(&response.payload, 4 + symbol.byte_size as usize)?;
        expect_payload_result(&data[..4], AdsErrorCode::NoError)?;
        if data.len() != 4 + symbol.byte_size as usize {
            return Err("sum-up read returned an unexpected byte count".to_string());
        }
        Ok(())
    }

    pub(super) fn notification(&mut self, symbol: &SymbolDescriptor) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&symbol.byte_size.to_le_bytes());
        payload.extend_from_slice(&ADSTRANS_SERVERCYCLE.to_le_bytes());
        payload.extend_from_slice(&50_u32.to_le_bytes());
        payload.extend_from_slice(&50_u32.to_le_bytes());
        payload.extend_from_slice(&[0_u8; 16]);
        let response = self.request(CommandId::AddDeviceNotification, payload)?;
        let handle = expect_add_notification(&response.payload)?;
        let notification = self.read_frame()?;
        self.validate_inbound_endpoint(&notification.header)?;
        if notification.header.state != AmsState::Request {
            return Err("DeviceNotification was not a server-to-client request".to_string());
        }
        if notification.header.command_id != CommandId::DeviceNotification {
            return Err(format!(
                "expected DeviceNotification, got {:?}",
                notification.header.command_id
            ));
        }
        if !device_notification_has_complete_sample(
            &notification.payload,
            handle,
            symbol.byte_size as usize,
        ) {
            return Err(
                "DeviceNotification did not contain the complete registered-handle sample"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub(super) fn write(&mut self, symbol: &SymbolDescriptor, bytes: &[u8]) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        payload.extend_from_slice(bytes);
        let response = self.request(CommandId::Write, payload)?;
        expect_payload_result(&response.payload, AdsErrorCode::NoError)
    }

    pub(super) fn write_expect_denied(
        &mut self,
        symbol: &SymbolDescriptor,
        bytes: &[u8],
    ) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        payload.extend_from_slice(bytes);
        let response = self.request(CommandId::Write, payload)?;
        let result = read_u32(&response.payload, 0)?;
        if result == AdsErrorCode::InvalidAccess.value()
            || result == AdsErrorCode::AccessDenied.value()
        {
            Ok(())
        } else {
            Err(format!(
                "write guard returned 0x{result:04X}, expected access denial"
            ))
        }
    }

    pub(super) fn request(
        &mut self,
        command_id: CommandId,
        payload: Vec<u8>,
    ) -> Result<AmsTcpFrame, String> {
        let frame = self.request_frame(command_id, payload)?;
        let invoke_id = frame.header.invoke_id;
        self.stream
            .write_all(&frame.to_bytes().map_err(|error| error.to_string())?)
            .map_err(|error| format!("write ADS request: {error}"))?;

        for _ in 0..8 {
            let response = self.read_frame()?;
            if response.header.command_id == CommandId::DeviceNotification {
                self.validate_inbound_endpoint(&response.header)?;
                if response.header.state != AmsState::Request {
                    return Err("DeviceNotification was not a server-to-client request".to_string());
                }
                continue;
            }
            self.validate_response_header(&response.header)?;
            if response.header.command_id != command_id {
                return Err(format!(
                    "response command mismatch: got {:?}, expected {:?}",
                    response.header.command_id, command_id
                ));
            }
            if response.header.invoke_id != invoke_id {
                return Err(format!(
                    "response invoke id mismatch: got {}, expected {}",
                    response.header.invoke_id, invoke_id
                ));
            }
            if response.header.error_code != 0 {
                return Err(format!(
                    "AMS header error 0x{:04X}",
                    response.header.error_code
                ));
            }
            return Ok(response);
        }
        Err(format!(
            "timed out waiting for {:?} response after notification frames",
            command_id
        ))
    }

    fn validate_inbound_endpoint(&self, header: &AmsHeader) -> Result<(), String> {
        if header.target_net_id != self.source_net_id || header.target_port != self.source_port {
            return Err("inbound ADS frame target did not match the loopback client".to_string());
        }
        if header.source_net_id != self.target_net_id || header.source_port != self.target_port {
            return Err("inbound ADS frame source did not match the ADS server".to_string());
        }
        Ok(())
    }

    fn validate_response_header(&self, header: &AmsHeader) -> Result<(), String> {
        self.validate_inbound_endpoint(header)?;
        if header.state != AmsState::Response {
            return Err("inbound ADS frame was not a response".to_string());
        }
        Ok(())
    }

    fn request_frame(
        &mut self,
        command_id: CommandId,
        payload: Vec<u8>,
    ) -> Result<AmsTcpFrame, String> {
        let data_length = u32::try_from(payload.len())
            .map_err(|_| "ADS request payload length does not fit u32".to_string())?;
        let frame = AmsTcpFrame {
            header: AmsHeader {
                target_net_id: self.target_net_id,
                target_port: self.target_port,
                source_net_id: self.source_net_id,
                source_port: self.source_port,
                command_id,
                state: AmsState::Request,
                data_length,
                error_code: 0,
                invoke_id: self.next_invoke,
            },
            payload,
        };
        self.next_invoke = self.next_invoke.wrapping_add(1).max(1);
        Ok(frame)
    }

    fn read_frame(&mut self) -> Result<AmsTcpFrame, String> {
        let mut prefix = [0_u8; AMS_TCP_HEADER_LEN];
        self.stream
            .read_exact(&mut prefix)
            .map_err(|error| format!("read AMS/TCP prefix: {error}"))?;
        let ams_len = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
        if ams_len < AMS_HEADER_LEN || ams_len > self.max_frame_bytes {
            return Err(format!("invalid AMS/TCP length {ams_len}"));
        }
        let mut bytes = Vec::from(prefix);
        bytes.resize(AMS_TCP_HEADER_LEN + ams_len, 0);
        self.stream
            .read_exact(&mut bytes[AMS_TCP_HEADER_LEN..])
            .map_err(|error| format!("read AMS/TCP payload: {error}"))?;
        AmsTcpFrame::parse(&bytes, self.max_frame_bytes).map_err(|error| error.to_string())
    }
}

fn expect_payload_result(payload: &[u8], expected: AdsErrorCode) -> Result<(), String> {
    let result = read_u32(payload, 0)?;
    if result == expected.value() {
        Ok(())
    } else {
        Err(format!(
            "ADS result 0x{result:04X}, expected 0x{:04X}",
            expected.value()
        ))
    }
}

fn expect_read_payload(payload: &[u8], expected_len: usize) -> Result<Vec<u8>, String> {
    expect_payload_result(payload, AdsErrorCode::NoError)?;
    let len = read_u32(payload, 4)? as usize;
    if len != expected_len {
        return Err(format!("ADS read length {len}, expected {expected_len}"));
    }
    let data = payload
        .get(8..8 + len)
        .ok_or_else(|| "ADS read response was truncated".to_string())?;
    Ok(data.to_vec())
}

pub(super) fn expect_add_notification(payload: &[u8]) -> Result<u32, String> {
    expect_payload_result(payload, AdsErrorCode::NoError)?;
    let handle = read_u32(payload, 4)?;
    if handle == 0 {
        return Err("notification handle response returned zero".to_string());
    }
    Ok(handle)
}

fn device_notification_has_complete_sample(
    payload: &[u8],
    expected_handle: u32,
    expected_len: usize,
) -> bool {
    let Ok(stream_len) = read_u32(payload, 0) else {
        return false;
    };
    let Some(stream_end) = 4_usize.checked_add(stream_len as usize) else {
        return false;
    };
    if stream_end != payload.len() {
        return false;
    }
    let Ok(stamp_count) = read_u32(payload, 4) else {
        return false;
    };
    let mut offset = 8_usize;
    let mut matching_sample_found = false;
    for _ in 0..stamp_count {
        if offset.checked_add(12).is_none_or(|end| end > stream_end) {
            return false;
        }
        offset += 8;
        let Ok(sample_count) = read_u32(payload, offset) else {
            return false;
        };
        offset += 4;
        for _ in 0..sample_count {
            if offset.checked_add(8).is_none_or(|end| end > stream_end) {
                return false;
            }
            let Ok(handle) = read_u32(payload, offset) else {
                return false;
            };
            let Ok(sample_len) = read_u32(payload, offset + 4) else {
                return false;
            };
            offset += 8;
            let Some(next) = offset.checked_add(sample_len as usize) else {
                return false;
            };
            if next > stream_end {
                return false;
            }
            if handle == expected_handle {
                if matching_sample_found || sample_len as usize != expected_len {
                    return false;
                }
                matching_sample_found = true;
            }
            offset = next;
        }
    }
    offset == stream_end && matching_sample_found
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("expected u32 at byte {offset}"))?;
    Ok(u32::from_le_bytes(slice.try_into().map_err(|_| {
        format!("expected four bytes at byte {offset}")
    })?))
}
