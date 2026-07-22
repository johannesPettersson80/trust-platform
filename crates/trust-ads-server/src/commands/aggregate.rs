//! ADS sum-up command dispatch.

use std::ops::Range;

use super::requests::ReadWriteRequest;
use super::wire::{push_result, read_response, read_u32, u32_len};
use super::{CommandContext, CommandDispatcher};
use crate::AdsErrorCode;

impl CommandDispatcher {
    pub(super) fn sumup_read(
        &self,
        request: ReadWriteRequest<'_>,
        ctx: &CommandContext<'_>,
    ) -> Vec<u8> {
        let count = request.index_offset as usize;
        if count == 0 {
            return read_response(AdsErrorCode::ServiceNotSupported, &[]);
        }
        if count > self.max_sumup_items {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        let Some(header_len) = count.checked_mul(12) else {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        };
        if request.write_data.len() != header_len {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }

        let mut metadata = Vec::with_capacity(count * 4);
        let mut data = Vec::new();
        for index in 0..count {
            let offset = index * 12;
            let index_group = read_u32(request.write_data, offset).unwrap_or(0);
            let index_offset = read_u32(request.write_data, offset + 4).unwrap_or(0);
            let read_length = read_u32(request.write_data, offset + 8).unwrap_or(0);
            let (code, item_data) = self.read_address(index_group, index_offset, read_length, ctx);
            push_result(&mut metadata, code);
            if code == AdsErrorCode::NoError {
                data.extend(item_data);
            }
        }
        metadata.extend(data);
        read_response(AdsErrorCode::NoError, &metadata)
    }

    pub(super) fn sumup_read_ex(
        &self,
        request: ReadWriteRequest<'_>,
        ctx: &CommandContext<'_>,
    ) -> Vec<u8> {
        let count = request.index_offset as usize;
        if count == 0 {
            return read_response(AdsErrorCode::ServiceNotSupported, &[]);
        }
        if count > self.max_sumup_items {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        let Some(header_len) = count.checked_mul(12) else {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        };
        if request.write_data.len() != header_len {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }

        let mut metadata = Vec::with_capacity(count * 8);
        let mut data = Vec::new();
        for index in 0..count {
            let offset = index * 12;
            let index_group = read_u32(request.write_data, offset).unwrap_or(0);
            let index_offset = read_u32(request.write_data, offset + 4).unwrap_or(0);
            let read_length = read_u32(request.write_data, offset + 8).unwrap_or(0);
            let (code, item_data) = self.read_address(index_group, index_offset, read_length, ctx);
            push_result(&mut metadata, code);
            if code == AdsErrorCode::NoError {
                let Ok(item_len) = u32_len(item_data.len()) else {
                    metadata.extend_from_slice(&0_u32.to_le_bytes());
                    continue;
                };
                metadata.extend_from_slice(&item_len.to_le_bytes());
                data.extend(item_data);
            } else {
                metadata.extend_from_slice(&0_u32.to_le_bytes());
            }
        }
        metadata.extend(data);
        read_response(AdsErrorCode::NoError, &metadata)
    }

    pub(super) fn sumup_write(
        &mut self,
        request: ReadWriteRequest<'_>,
        ctx: &CommandContext<'_>,
    ) -> Vec<u8> {
        let count = request.index_offset as usize;
        if count > self.max_sumup_items {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        let Some(header_len) = count.checked_mul(12) else {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        };
        if request.write_data.len() < header_len {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        if request.write_data.len().saturating_sub(header_len) > self.max_write_bytes {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        let write_ranges = match validate_write_ranges(request.write_data, count, 12, 8, header_len)
        {
            Ok(ranges) => ranges,
            Err(code) => return read_response(code, &[]),
        };

        let mut response_data = Vec::with_capacity(count * 4);
        for (index, data_range) in write_ranges.into_iter().enumerate() {
            let offset = index * 12;
            let index_group = read_u32(request.write_data, offset).unwrap_or(0);
            let index_offset = read_u32(request.write_data, offset + 4).unwrap_or(0);
            let data = &request.write_data[data_range];
            let code = self.write_address(index_group, index_offset, data, ctx);
            push_result(&mut response_data, code);
        }
        read_response(AdsErrorCode::NoError, &response_data)
    }

    pub(super) fn sumup_read_write(
        &mut self,
        request: ReadWriteRequest<'_>,
        ctx: &CommandContext<'_>,
    ) -> Vec<u8> {
        let count = request.index_offset as usize;
        if count > self.max_sumup_items {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        let Some(header_len) = count.checked_mul(16) else {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        };
        if request.write_data.len() < header_len {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        if request.write_data.len().saturating_sub(header_len) > self.max_write_bytes {
            return read_response(AdsErrorCode::InvalidSize, &[]);
        }
        let write_ranges =
            match validate_write_ranges(request.write_data, count, 16, 12, header_len) {
                Ok(ranges) => ranges,
                Err(code) => return read_response(code, &[]),
            };

        let mut metadata = Vec::with_capacity(count * 8);
        let mut read_data = Vec::new();
        for (index, data_range) in write_ranges.into_iter().enumerate() {
            let offset = index * 16;
            let index_group = read_u32(request.write_data, offset).unwrap_or(0);
            let index_offset = read_u32(request.write_data, offset + 4).unwrap_or(0);
            let read_length = read_u32(request.write_data, offset + 8).unwrap_or(0);
            let data = &request.write_data[data_range];

            let write_code = if data.is_empty() {
                AdsErrorCode::NoError
            } else {
                self.write_address(index_group, index_offset, data, ctx)
            };
            if write_code != AdsErrorCode::NoError {
                push_result(&mut metadata, write_code);
                metadata.extend_from_slice(&0_u32.to_le_bytes());
                continue;
            }
            let (read_code, item_data) =
                self.read_address(index_group, index_offset, read_length, ctx);
            push_result(&mut metadata, read_code);
            if read_code == AdsErrorCode::NoError {
                let Ok(item_len) = u32_len(item_data.len()) else {
                    push_result(&mut metadata, AdsErrorCode::InvalidSize);
                    metadata.extend_from_slice(&0_u32.to_le_bytes());
                    continue;
                };
                metadata.extend_from_slice(&item_len.to_le_bytes());
                read_data.extend(item_data);
            } else {
                metadata.extend_from_slice(&0_u32.to_le_bytes());
            }
        }
        metadata.extend(read_data);
        read_response(AdsErrorCode::NoError, &metadata)
    }
}

fn validate_write_ranges(
    payload: &[u8],
    count: usize,
    header_stride: usize,
    write_length_offset: usize,
    header_len: usize,
) -> Result<Vec<Range<usize>>, AdsErrorCode> {
    let mut write_offset = header_len;
    let mut ranges = Vec::with_capacity(count);
    for index in 0..count {
        let header_offset = index
            .checked_mul(header_stride)
            .ok_or(AdsErrorCode::InvalidSize)?;
        let length_offset = header_offset
            .checked_add(write_length_offset)
            .ok_or(AdsErrorCode::InvalidSize)?;
        let write_length = read_u32(payload, length_offset)? as usize;
        let end = write_offset
            .checked_add(write_length)
            .ok_or(AdsErrorCode::InvalidSize)?;
        if end > payload.len() {
            return Err(AdsErrorCode::InvalidSize);
        }
        ranges.push(write_offset..end);
        write_offset = end;
    }
    if write_offset != payload.len() {
        return Err(AdsErrorCode::InvalidSize);
    }
    Ok(ranges)
}
