use core::fmt;

/// Byte length of the AMS/TCP prefix.
pub const AMS_TCP_HEADER_LEN: usize = 6;
/// Byte length of the AMS header.
pub const AMS_HEADER_LEN: usize = 32;
/// AMS state flags for an ADS request.
pub const STATE_FLAGS_REQUEST: u16 = 0x0004;
/// AMS state flags for an ADS response.
pub const STATE_FLAGS_RESPONSE: u16 = 0x0005;

/// Six-byte AMS Net ID as it appears on the wire.
pub type AmsNetIdBytes = [u8; 6];

/// Parses a dotted AMS Net ID such as `192.168.10.20.1.1` into wire bytes.
///
/// # Errors
///
/// Returns an error when the text does not contain exactly six u8 segments.
pub fn ams_net_id_text_to_bytes(text: &str) -> Result<AmsNetIdBytes, AmsNetIdTextError> {
    let mut out = [0_u8; 6];
    let mut count = 0_usize;
    for (index, part) in text.split('.').enumerate() {
        if index >= out.len() {
            return Err(AmsNetIdTextError::WrongSegmentCount {
                expected: 6,
                actual: index + 1,
            });
        }
        out[index] = part
            .parse::<u8>()
            .map_err(|_| AmsNetIdTextError::InvalidSegment {
                index,
                value: part.to_string(),
            })?;
        count = index + 1;
    }
    if count != out.len() {
        return Err(AmsNetIdTextError::WrongSegmentCount {
            expected: 6,
            actual: count,
        });
    }
    Ok(out)
}

/// Formats AMS Net ID wire bytes as dotted text.
#[must_use]
pub fn ams_net_id_bytes_to_text(bytes: AmsNetIdBytes) -> String {
    format!(
        "{}.{}.{}.{}.{}.{}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
    )
}

/// Error returned when parsing AMS Net ID text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmsNetIdTextError {
    /// Segment count was not six.
    WrongSegmentCount {
        /// Expected segment count.
        expected: usize,
        /// Actual segment count.
        actual: usize,
    },
    /// One segment was not a valid u8.
    InvalidSegment {
        /// Segment index.
        index: usize,
        /// Segment text.
        value: String,
    },
}

impl fmt::Display for AmsNetIdTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSegmentCount { expected, actual } => {
                write!(f, "AMS Net ID must have {expected} segments, got {actual}")
            }
            Self::InvalidSegment { index, value } => {
                write!(f, "AMS Net ID segment {index} is not a u8: '{value}'")
            }
        }
    }
}

impl std::error::Error for AmsNetIdTextError {}

/// ADS command ID in the AMS header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum CommandId {
    /// ADS Read Device Info.
    ReadDeviceInfo = 0x0001,
    /// ADS Read.
    Read = 0x0002,
    /// ADS Write.
    Write = 0x0003,
    /// ADS Read State.
    ReadState = 0x0004,
    /// ADS Write Control.
    WriteControl = 0x0005,
    /// ADS Add Device Notification.
    AddDeviceNotification = 0x0006,
    /// ADS Delete Device Notification.
    DeleteDeviceNotification = 0x0007,
    /// ADS Device Notification.
    DeviceNotification = 0x0008,
    /// ADS `ReadWrite`.
    ReadWrite = 0x0009,
}

impl CommandId {
    /// Returns the wire command value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self as u16
    }
}

impl TryFrom<u16> for CommandId {
    type Error = AmsParseError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::ReadDeviceInfo),
            0x0002 => Ok(Self::Read),
            0x0003 => Ok(Self::Write),
            0x0004 => Ok(Self::ReadState),
            0x0005 => Ok(Self::WriteControl),
            0x0006 => Ok(Self::AddDeviceNotification),
            0x0007 => Ok(Self::DeleteDeviceNotification),
            0x0008 => Ok(Self::DeviceNotification),
            0x0009 => Ok(Self::ReadWrite),
            other => Err(AmsParseError::InvalidCommandId(other)),
        }
    }
}

/// Direction encoded by AMS state flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmsState {
    /// ADS request frame.
    Request,
    /// ADS response frame.
    Response,
}

impl AmsState {
    /// Returns the state flags for this direction.
    #[must_use]
    pub const fn flags(self) -> u16 {
        match self {
            Self::Request => STATE_FLAGS_REQUEST,
            Self::Response => STATE_FLAGS_RESPONSE,
        }
    }
}

impl TryFrom<u16> for AmsState {
    type Error = AmsParseError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            STATE_FLAGS_REQUEST => Ok(Self::Request),
            STATE_FLAGS_RESPONSE => Ok(Self::Response),
            other => Err(AmsParseError::InvalidStateFlags(other)),
        }
    }
}

/// Parsed AMS header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmsHeader {
    /// Target AMS Net ID.
    pub target_net_id: AmsNetIdBytes,
    /// Target AMS port.
    pub target_port: u16,
    /// Source AMS Net ID.
    pub source_net_id: AmsNetIdBytes,
    /// Source AMS port.
    pub source_port: u16,
    /// ADS command.
    pub command_id: CommandId,
    /// Request/response state.
    pub state: AmsState,
    /// ADS command payload length.
    pub data_length: u32,
    /// AMS-level error code.
    pub error_code: u32,
    /// Invoke ID echoed by responses.
    pub invoke_id: u32,
}

impl AmsHeader {
    /// Parses a 32-byte AMS header.
    ///
    /// # Errors
    ///
    /// Returns an error when the header is truncated, has invalid state flags,
    /// or carries an unknown command ID.
    pub fn parse(bytes: &[u8]) -> Result<Self, AmsParseError> {
        if bytes.len() < AMS_HEADER_LEN {
            return Err(AmsParseError::Truncated {
                needed: AMS_HEADER_LEN,
                actual: bytes.len(),
            });
        }

        Ok(Self {
            target_net_id: read_net_id(bytes, 0)?,
            target_port: read_u16(bytes, 6)?,
            source_net_id: read_net_id(bytes, 8)?,
            source_port: read_u16(bytes, 14)?,
            command_id: CommandId::try_from(read_u16(bytes, 16)?)?,
            state: AmsState::try_from(read_u16(bytes, 18)?)?,
            data_length: read_u32(bytes, 20)?,
            error_code: read_u32(bytes, 24)?,
            invoke_id: read_u32(bytes, 28)?,
        })
    }

    /// Writes this AMS header to `out`.
    pub fn write_to(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.target_net_id);
        out.extend_from_slice(&self.target_port.to_le_bytes());
        out.extend_from_slice(&self.source_net_id);
        out.extend_from_slice(&self.source_port.to_le_bytes());
        out.extend_from_slice(&self.command_id.value().to_le_bytes());
        out.extend_from_slice(&self.state.flags().to_le_bytes());
        out.extend_from_slice(&self.data_length.to_le_bytes());
        out.extend_from_slice(&self.error_code.to_le_bytes());
        out.extend_from_slice(&self.invoke_id.to_le_bytes());
    }

    /// Builds a response header for `payload_len`.
    ///
    /// # Errors
    ///
    /// Returns an error if `payload_len` does not fit in a 32-bit AMS length.
    pub fn response_for(
        &self,
        payload_len: usize,
        ams_error_code: u32,
    ) -> Result<Self, AmsParseError> {
        let data_length =
            u32::try_from(payload_len).map_err(|_| AmsParseError::PayloadTooLarge {
                length: payload_len,
            })?;
        Ok(Self {
            target_net_id: self.source_net_id,
            target_port: self.source_port,
            source_net_id: self.target_net_id,
            source_port: self.target_port,
            command_id: self.command_id,
            state: AmsState::Response,
            data_length,
            error_code: ams_error_code,
            invoke_id: self.invoke_id,
        })
    }
}

/// Parsed AMS/TCP frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmsTcpFrame {
    /// Parsed AMS header.
    pub header: AmsHeader,
    /// ADS command payload.
    pub payload: Vec<u8>,
}

impl AmsTcpFrame {
    /// Parses an AMS/TCP frame from bytes.
    ///
    /// `max_frame_bytes` caps the AMS header + payload length declared in the
    /// AMS/TCP prefix. The 6-byte AMS/TCP prefix itself is not counted.
    ///
    /// # Errors
    ///
    /// Returns an error for truncated data, invalid reserved bytes, invalid
    /// length fields, unsupported command IDs, or invalid state flags.
    pub fn parse(bytes: &[u8], max_frame_bytes: usize) -> Result<Self, AmsParseError> {
        if bytes.len() < AMS_TCP_HEADER_LEN {
            return Err(AmsParseError::Truncated {
                needed: AMS_TCP_HEADER_LEN,
                actual: bytes.len(),
            });
        }
        if bytes.get(0..2) != Some(&[0, 0]) {
            return Err(AmsParseError::NonZeroReserved);
        }

        let ams_len = read_u32(bytes, 2)? as usize;
        if ams_len < AMS_HEADER_LEN {
            return Err(AmsParseError::InvalidAmsLength {
                length: ams_len,
                minimum: AMS_HEADER_LEN,
            });
        }
        if ams_len > max_frame_bytes {
            return Err(AmsParseError::FrameTooLarge {
                length: ams_len,
                max: max_frame_bytes,
            });
        }
        let total_len = AMS_TCP_HEADER_LEN
            .checked_add(ams_len)
            .ok_or(AmsParseError::PayloadTooLarge { length: ams_len })?;
        if bytes.len() < total_len {
            return Err(AmsParseError::Truncated {
                needed: total_len,
                actual: bytes.len(),
            });
        }

        let header_start = AMS_TCP_HEADER_LEN;
        let header_end = header_start + AMS_HEADER_LEN;
        let header_bytes = bytes
            .get(header_start..header_end)
            .ok_or(AmsParseError::Truncated {
                needed: header_end,
                actual: bytes.len(),
            })?;
        let header = AmsHeader::parse(header_bytes)?;
        let expected_payload_len = ams_len - AMS_HEADER_LEN;
        if header.data_length as usize != expected_payload_len {
            return Err(AmsParseError::DataLengthMismatch {
                header_data_length: header.data_length,
                actual_payload_length: expected_payload_len,
            });
        }
        let payload_bytes = bytes
            .get(header_end..total_len)
            .ok_or(AmsParseError::Truncated {
                needed: total_len,
                actual: bytes.len(),
            })?;

        Ok(Self {
            header,
            payload: payload_bytes.to_vec(),
        })
    }

    /// Serializes this frame as AMS/TCP bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload length cannot be represented on the wire
    /// or if the header data length does not match the payload.
    pub fn to_bytes(&self) -> Result<Vec<u8>, AmsParseError> {
        let payload_len =
            u32::try_from(self.payload.len()).map_err(|_| AmsParseError::PayloadTooLarge {
                length: self.payload.len(),
            })?;
        if self.header.data_length != payload_len {
            return Err(AmsParseError::DataLengthMismatch {
                header_data_length: self.header.data_length,
                actual_payload_length: self.payload.len(),
            });
        }
        let ams_len = AMS_HEADER_LEN.checked_add(self.payload.len()).ok_or(
            AmsParseError::PayloadTooLarge {
                length: self.payload.len(),
            },
        )?;
        let ams_len_u32 = u32::try_from(ams_len)
            .map_err(|_| AmsParseError::PayloadTooLarge { length: ams_len })?;

        let mut out = Vec::with_capacity(AMS_TCP_HEADER_LEN + ams_len);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&ams_len_u32.to_le_bytes());
        self.header.write_to(&mut out);
        out.extend_from_slice(&self.payload);
        Ok(out)
    }
}

/// Error raised while parsing or serializing AMS/TCP bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmsParseError {
    /// Not enough bytes were available.
    Truncated {
        /// Number of bytes needed.
        needed: usize,
        /// Number of bytes available.
        actual: usize,
    },
    /// AMS/TCP reserved bytes were not zero.
    NonZeroReserved,
    /// AMS/TCP length was below the minimum AMS header length.
    InvalidAmsLength {
        /// Declared AMS length.
        length: usize,
        /// Minimum valid AMS length.
        minimum: usize,
    },
    /// Declared AMS length exceeds the configured cap.
    FrameTooLarge {
        /// Declared AMS length.
        length: usize,
        /// Configured maximum AMS length.
        max: usize,
    },
    /// Payload length cannot be represented or allocated safely.
    PayloadTooLarge {
        /// Payload or AMS length.
        length: usize,
    },
    /// AMS header data length did not match the frame payload length.
    DataLengthMismatch {
        /// Data length encoded in the AMS header.
        header_data_length: u32,
        /// Actual payload length from AMS/TCP framing.
        actual_payload_length: usize,
    },
    /// Unknown ADS command ID.
    InvalidCommandId(u16),
    /// Unsupported AMS state flags.
    InvalidStateFlags(u16),
}

impl fmt::Display for AmsParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, actual } => {
                write!(f, "truncated AMS frame: needed {needed} bytes, got {actual}")
            }
            Self::NonZeroReserved => write!(f, "AMS/TCP reserved bytes must be zero"),
            Self::InvalidAmsLength { length, minimum } => write!(
                f,
                "invalid AMS length {length}; minimum header length is {minimum}"
            ),
            Self::FrameTooLarge { length, max } => {
                write!(f, "AMS frame length {length} exceeds cap {max}")
            }
            Self::PayloadTooLarge { length } => {
                write!(f, "AMS payload length {length} is too large")
            }
            Self::DataLengthMismatch {
                header_data_length,
                actual_payload_length,
            } => write!(
                f,
                "AMS data length mismatch: header says {header_data_length}, frame has {actual_payload_length}"
            ),
            Self::InvalidCommandId(command) => write!(f, "invalid ADS command id 0x{command:04X}"),
            Self::InvalidStateFlags(flags) => write!(f, "invalid AMS state flags 0x{flags:04X}"),
        }
    }
}

impl std::error::Error for AmsParseError {}

fn read_net_id(bytes: &[u8], offset: usize) -> Result<AmsNetIdBytes, AmsParseError> {
    let slice = bytes
        .get(offset..offset + 6)
        .ok_or(AmsParseError::Truncated {
            needed: offset + 6,
            actual: bytes.len(),
        })?;
    let mut net_id = [0; 6];
    net_id.copy_from_slice(slice);
    Ok(net_id)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, AmsParseError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(AmsParseError::Truncated {
            needed: offset + 2,
            actual: bytes.len(),
        })?;
    let bytes: [u8; 2] = slice.try_into().map_err(|_| AmsParseError::Truncated {
        needed: offset + 2,
        actual: slice.len(),
    })?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, AmsParseError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(AmsParseError::Truncated {
            needed: offset + 4,
            actual: bytes.len(),
        })?;
    let bytes: [u8; 4] = slice.try_into().map_err(|_| AmsParseError::Truncated {
        needed: offset + 4,
        actual: slice.len(),
    })?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::{
        AmsHeader, AmsParseError, AmsState, AmsTcpFrame, CommandId, AMS_HEADER_LEN,
        AMS_TCP_HEADER_LEN,
    };

    const TARGET_NET_ID: [u8; 6] = [5, 23, 91, 12, 1, 1];
    const SOURCE_NET_ID: [u8; 6] = [192, 168, 10, 20, 1, 1];

    fn read_request_bytes() -> Vec<u8> {
        vec![
            0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, // AMS/TCP
            0x05, 0x17, 0x5B, 0x0C, 0x01, 0x01, // target net id
            0x53, 0x03, // target port 851
            0xC0, 0xA8, 0x0A, 0x14, 0x01, 0x01, // source net id
            0x01, 0x80, // source port
            0x02, 0x00, // Read
            0x04, 0x00, // request flags
            0x0C, 0x00, 0x00, 0x00, // data length
            0x00, 0x00, 0x00, 0x00, // AMS error
            0x2A, 0x00, 0x00, 0x00, // invoke id
            0x20, 0x40, 0x00, 0x00, // index group
            0x00, 0x00, 0x00, 0x00, // index offset
            0x04, 0x00, 0x00, 0x00, // read length
        ]
    }

    #[test]
    fn ams_frame_round_trips_valid_read_request_bytes() {
        let bytes = read_request_bytes();
        let frame = AmsTcpFrame::parse(&bytes, 4096).expect("parse read request");

        assert_eq!(frame.header.target_net_id, TARGET_NET_ID);
        assert_eq!(frame.header.target_port, 851);
        assert_eq!(frame.header.source_net_id, SOURCE_NET_ID);
        assert_eq!(frame.header.source_port, 0x8001);
        assert_eq!(frame.header.command_id, CommandId::Read);
        assert_eq!(frame.header.state, AmsState::Request);
        assert_eq!(frame.header.data_length, 12);
        assert_eq!(frame.header.error_code, 0);
        assert_eq!(frame.header.invoke_id, 42);
        assert_eq!(frame.payload.len(), 12);
        assert_eq!(frame.to_bytes().expect("serialize"), bytes);
    }

    #[test]
    fn response_header_swaps_source_and_target() {
        let request = AmsTcpFrame::parse(&read_request_bytes(), 4096).expect("parse request");

        let response = request.header.response_for(8, 0).expect("response header");

        assert_eq!(response.target_net_id, SOURCE_NET_ID);
        assert_eq!(response.target_port, 0x8001);
        assert_eq!(response.source_net_id, TARGET_NET_ID);
        assert_eq!(response.source_port, 851);
        assert_eq!(response.command_id, CommandId::Read);
        assert_eq!(response.state, AmsState::Response);
        assert_eq!(response.data_length, 8);
        assert_eq!(response.invoke_id, 42);
    }

    #[test]
    fn parser_rejects_nonzero_reserved_bytes() {
        let mut bytes = read_request_bytes();
        bytes[1] = 1;

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 4096),
            Err(AmsParseError::NonZeroReserved)
        );
    }

    #[test]
    fn parser_rejects_length_smaller_than_ams_header() {
        let mut bytes = read_request_bytes();
        bytes[2..6].copy_from_slice(&31_u32.to_le_bytes());

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 4096),
            Err(AmsParseError::InvalidAmsLength {
                length: AMS_HEADER_LEN - 1,
                minimum: AMS_HEADER_LEN,
            })
        );
    }

    #[test]
    fn parser_rejects_frame_over_configured_cap_before_payload_use() {
        let bytes = read_request_bytes();

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 16),
            Err(AmsParseError::FrameTooLarge {
                length: 44,
                max: 16,
            })
        );
    }

    #[test]
    fn parser_rejects_truncated_ams_tcp_header() {
        assert_eq!(
            AmsTcpFrame::parse(&[0, 0, 1], 4096),
            Err(AmsParseError::Truncated {
                needed: AMS_TCP_HEADER_LEN,
                actual: 3,
            })
        );
    }

    #[test]
    fn parser_rejects_truncated_payload() {
        let mut bytes = read_request_bytes();
        bytes.truncate(bytes.len() - 1);

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 4096),
            Err(AmsParseError::Truncated {
                needed: AMS_TCP_HEADER_LEN + 44,
                actual: AMS_TCP_HEADER_LEN + 43,
            })
        );
    }

    #[test]
    fn parser_rejects_unknown_command_id() {
        let mut bytes = read_request_bytes();
        bytes[AMS_TCP_HEADER_LEN + 16..AMS_TCP_HEADER_LEN + 18]
            .copy_from_slice(&0x000A_u16.to_le_bytes());

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 4096),
            Err(AmsParseError::InvalidCommandId(0x000A))
        );
    }

    #[test]
    fn parser_rejects_invalid_state_flags() {
        let mut bytes = read_request_bytes();
        bytes[AMS_TCP_HEADER_LEN + 18..AMS_TCP_HEADER_LEN + 20]
            .copy_from_slice(&0x0000_u16.to_le_bytes());

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 4096),
            Err(AmsParseError::InvalidStateFlags(0))
        );
    }

    #[test]
    fn parser_rejects_ams_data_length_mismatch() {
        let mut bytes = read_request_bytes();
        bytes[AMS_TCP_HEADER_LEN + 20..AMS_TCP_HEADER_LEN + 24]
            .copy_from_slice(&8_u32.to_le_bytes());

        assert_eq!(
            AmsTcpFrame::parse(&bytes, 4096),
            Err(AmsParseError::DataLengthMismatch {
                header_data_length: 8,
                actual_payload_length: 12,
            })
        );
    }

    #[test]
    fn serializer_rejects_header_payload_mismatch() {
        let frame = AmsTcpFrame {
            header: AmsHeader {
                target_net_id: TARGET_NET_ID,
                target_port: 851,
                source_net_id: SOURCE_NET_ID,
                source_port: 0x8001,
                command_id: CommandId::Read,
                state: AmsState::Request,
                data_length: 4,
                error_code: 0,
                invoke_id: 1,
            },
            payload: vec![0; 8],
        };

        assert_eq!(
            frame.to_bytes(),
            Err(AmsParseError::DataLengthMismatch {
                header_data_length: 4,
                actual_payload_length: 8,
            })
        );
    }
}
