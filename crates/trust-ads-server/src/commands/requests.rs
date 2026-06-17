//! ADS command request payload parsing.

use crate::commands::wire::read_u32;
use crate::AdsErrorCode;

const ADS_NOTIFICATION_100NS_PER_MS: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IndexLengthRequest {
    pub(super) index_group: u32,
    pub(super) index_offset: u32,
    pub(super) length: u32,
}

impl IndexLengthRequest {
    pub(super) fn parse(payload: &[u8]) -> Result<Self, AdsErrorCode> {
        if payload.len() != 12 {
            return Err(AdsErrorCode::InvalidSize);
        }
        Ok(Self {
            index_group: read_u32(payload, 0)?,
            index_offset: read_u32(payload, 4)?,
            length: read_u32(payload, 8)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WriteRequest<'a> {
    pub(super) index_group: u32,
    pub(super) index_offset: u32,
    pub(super) data: &'a [u8],
}

impl<'a> WriteRequest<'a> {
    pub(super) fn parse(payload: &'a [u8]) -> Result<Self, AdsErrorCode> {
        if payload.len() < 12 {
            return Err(AdsErrorCode::InvalidSize);
        }
        let write_length = read_u32(payload, 8)? as usize;
        if payload.len() != 12 + write_length {
            return Err(AdsErrorCode::InvalidSize);
        }
        Ok(Self {
            index_group: read_u32(payload, 0)?,
            index_offset: read_u32(payload, 4)?,
            data: &payload[12..],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReadWriteRequest<'a> {
    pub(super) index_group: u32,
    pub(super) index_offset: u32,
    pub(super) read_length: u32,
    pub(super) write_data: &'a [u8],
}

impl<'a> ReadWriteRequest<'a> {
    pub(super) fn parse(payload: &'a [u8]) -> Result<Self, AdsErrorCode> {
        if payload.len() < 16 {
            return Err(AdsErrorCode::InvalidSize);
        }
        let write_length = read_u32(payload, 12)? as usize;
        if payload.len() != 16 + write_length {
            return Err(AdsErrorCode::InvalidSize);
        }
        Ok(Self {
            index_group: read_u32(payload, 0)?,
            index_offset: read_u32(payload, 4)?,
            read_length: read_u32(payload, 8)?,
            write_data: &payload[16..],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AddNotificationRequest {
    pub(super) index_group: u32,
    pub(super) index_offset: u32,
    pub(super) byte_len: u32,
    pub(super) transmission_mode: u32,
    pub(super) max_delay_ms: u32,
    pub(super) cycle_time_ms: u32,
}

impl AddNotificationRequest {
    pub(super) fn parse(payload: &[u8]) -> Result<Self, AdsErrorCode> {
        if payload.len() != 24 && payload.len() != 40 {
            return Err(AdsErrorCode::InvalidSize);
        }
        if payload.len() == 40 && payload.get(24..40) != Some(&[0; 16]) {
            return Err(AdsErrorCode::InvalidData);
        }
        Ok(Self {
            index_group: read_u32(payload, 0)?,
            index_offset: read_u32(payload, 4)?,
            byte_len: read_u32(payload, 8)?,
            transmission_mode: read_u32(payload, 12)?,
            max_delay_ms: normalize_notification_time_ms(read_u32(payload, 16)?),
            cycle_time_ms: normalize_notification_time_ms(read_u32(payload, 20)?),
        })
    }
}

fn normalize_notification_time_ms(raw: u32) -> u32 {
    if raw != 0 && raw.is_multiple_of(ADS_NOTIFICATION_100NS_PER_MS) {
        raw / ADS_NOTIFICATION_100NS_PER_MS
    } else {
        raw
    }
}
