use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

use region::LockGuard;
use tiverse_mmap::{Mmap, MmapOptions, ReadWrite};

use super::{T0ChannelOwnership, T0PinningMode, T0PinningProvider, T0ShmConfig};

const HEADER_SIZE: usize = 256;
const MAGIC: u64 = 0x5452_5553_545f_5430; // "TRUST_T0"
const HEADER_VERSION: u64 = 1;

const OFFSET_MAGIC: usize = 0;
const OFFSET_VERSION: usize = 8;
const OFFSET_CHANNEL_TAG: usize = 16;
const OFFSET_SCHEMA_ID_TAG: usize = 24;
const OFFSET_SCHEMA_HASH_TAG: usize = 32;
const OFFSET_SCHEMA_VERSION: usize = 40;
const OFFSET_SLOT_SIZE: usize = 48;
const OFFSET_STALE_AFTER_READS: usize = 56;
const OFFSET_MAX_SPIN_RETRIES: usize = 64;
const OFFSET_MAX_SPIN_TIME_US: usize = 72;
const OFFSET_OWNERSHIP: usize = 80;
const OFFSET_SEQLOCK: usize = 88;
const OFFSET_WRITE_SEQ: usize = 96;
const OFFSET_READ_SEQ: usize = 104;
const OFFSET_PAYLOAD_LEN: usize = 112;
const OFFSET_OVERRUN_COUNT: usize = 120;

#[derive(Debug, Clone, Copy)]
pub(super) struct ShmChannelContract<'a> {
    pub channel_tag: u64,
    pub schema_id_tag: u64,
    pub schema_hash_tag: u64,
    pub schema_version: u32,
    pub slot_size: usize,
    pub stale_after_reads: u8,
    pub max_spin_retries: u8,
    pub max_spin_time_us: u64,
    pub ownership: T0ChannelOwnership,
    pub channel_id: &'a str,
}

pub(super) struct ShmChannel {
    path: PathBuf,
    lock_guard: Option<LockGuard>,
    map: Mmap<ReadWrite>,
}

impl std::fmt::Debug for ShmChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShmChannel")
            .field("path", &self.path)
            .field("slot_size", &self.slot_size())
            .field("pinned", &self.is_pinned())
            .finish()
    }
}

impl ShmChannel {
    pub(super) fn create_or_open(
        config: &T0ShmConfig,
        contract: ShmChannelContract<'_>,
    ) -> Result<Self, String> {
        if contract.slot_size == 0 {
            return Err("slot_size must be greater than zero".to_string());
        }
        std::fs::create_dir_all(&config.root_dir).map_err(|error| {
            format!(
                "failed to create T0 SHM directory '{}': {error}",
                config.root_dir.display()
            )
        })?;

        let path =
            channel_mapping_path(&config.root_dir, contract.channel_id, contract.channel_tag);
        let total_size = HEADER_SIZE.saturating_add(contract.slot_size);
        let (created, file) = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => (true, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .map_err(|open_error| {
                        format!(
                            "failed to open existing T0 SHM mapping '{}': {open_error}",
                            path.display()
                        )
                    })?;
                (false, file)
            }
            Err(error) => {
                return Err(format!(
                    "failed to create T0 SHM mapping '{}': {error}",
                    path.display()
                ));
            }
        };

        if created {
            file.set_len(total_size as u64).map_err(|error| {
                format!(
                    "failed to size T0 SHM mapping '{}': {error}",
                    path.display()
                )
            })?;
        }

        let file_len = file.metadata().map_err(|error| {
            format!(
                "failed to read T0 SHM metadata for '{}': {error}",
                path.display()
            )
        })?;
        if file_len.len() < total_size as u64 {
            return Err(format!(
                "existing T0 SHM mapping '{}' is smaller than required size {}",
                path.display(),
                total_size
            ));
        }
        drop(file);

        let mut map = map_shared_rw(&path, total_size)?;
        if created {
            initialize_header(&mut map, contract);
        } else {
            validate_header(&map, contract)?;
        }

        let lock_guard = lock_pages(config, &map)?;
        if matches!(config.pinning_mode, T0PinningMode::Required) && lock_guard.is_none() {
            return Err(format!(
                "required page pinning failed for T0 SHM channel '{}'",
                contract.channel_id
            ));
        }

        Ok(Self {
            path,
            lock_guard,
            map,
        })
    }

    #[must_use]
    pub(super) fn is_pinned(&self) -> bool {
        self.lock_guard.is_some()
    }

    #[must_use]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub(super) fn slot_size(&self) -> usize {
        read_u64(&self.map, OFFSET_SLOT_SIZE) as usize
    }

    #[must_use]
    pub(super) fn seqlock(&self) -> u64 {
        read_u64(&self.map, OFFSET_SEQLOCK)
    }

    pub(super) fn set_seqlock(&mut self, value: u64) {
        write_u64(&mut self.map, OFFSET_SEQLOCK, value);
    }

    #[must_use]
    pub(super) fn write_seq(&self) -> u64 {
        read_u64(&self.map, OFFSET_WRITE_SEQ)
    }

    pub(super) fn set_write_seq(&mut self, value: u64) {
        write_u64(&mut self.map, OFFSET_WRITE_SEQ, value);
    }

    #[must_use]
    pub(super) fn read_seq(&self) -> u64 {
        read_u64(&self.map, OFFSET_READ_SEQ)
    }

    pub(super) fn set_read_seq(&mut self, value: u64) {
        write_u64(&mut self.map, OFFSET_READ_SEQ, value);
    }

    #[must_use]
    pub(super) fn payload_len(&self) -> usize {
        read_u64(&self.map, OFFSET_PAYLOAD_LEN) as usize
    }

    pub(super) fn set_payload_len(&mut self, payload_len: usize) -> Result<(), String> {
        if payload_len > self.slot_size() {
            return Err(format!(
                "payload_len {} exceeds slot_size {}",
                payload_len,
                self.slot_size()
            ));
        }
        write_u64(&mut self.map, OFFSET_PAYLOAD_LEN, payload_len as u64);
        Ok(())
    }

    #[must_use]
    pub(super) fn overrun_count(&self) -> u64 {
        read_u64(&self.map, OFFSET_OVERRUN_COUNT)
    }

    pub(super) fn set_overrun_count(&mut self, value: u64) {
        write_u64(&mut self.map, OFFSET_OVERRUN_COUNT, value);
    }

    pub(super) fn write_payload(&mut self, payload: &[u8]) -> Result<(), String> {
        if payload.len() > self.slot_size() {
            return Err(format!(
                "payload size {} exceeds slot_size {}",
                payload.len(),
                self.slot_size()
            ));
        }
        let slot_range = self.slot_range(payload.len());
        self.map[slot_range].copy_from_slice(payload);
        self.set_payload_len(payload.len())
    }

    pub(super) fn copy_payload_into(
        &self,
        out: &mut [u8],
        expected_len: usize,
    ) -> Result<usize, String> {
        let payload_len = self.payload_len();
        if payload_len != expected_len {
            return Err(format!(
                "slot payload size {} does not match subscriber payload size {}",
                payload_len, expected_len
            ));
        }
        if out.len() < payload_len {
            return Err(format!(
                "output buffer size {} is smaller than payload size {}",
                out.len(),
                payload_len
            ));
        }
        let slot_range = self.slot_range(payload_len);
        out[..payload_len].copy_from_slice(&self.map[slot_range]);
        Ok(payload_len)
    }

    fn slot_range(&self, payload_len: usize) -> std::ops::Range<usize> {
        let end = HEADER_SIZE.saturating_add(payload_len);
        HEADER_SIZE..end
    }

    #[cfg(test)]
    pub(super) fn set_writer_stuck_for_test(&mut self) {
        self.set_seqlock(self.seqlock() | 1);
    }
}

fn channel_mapping_path(root: &Path, channel_id: &str, channel_tag: u64) -> PathBuf {
    let sanitized: String = channel_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    root.join(format!("{sanitized}-{channel_tag:016x}.t0shm"))
}

fn map_shared_rw(path: &Path, len: usize) -> Result<Mmap<ReadWrite>, String> {
    let options = MmapOptions::new().path(path).len(len);
    #[cfg(unix)]
    let options = options.shared();
    options.map_readwrite().map_err(|error| {
        format!(
            "failed to map '{}' as shared memory: {error}",
            path.display()
        )
    })
}

fn initialize_header(map: &mut Mmap<ReadWrite>, contract: ShmChannelContract<'_>) {
    map.fill(0);
    write_u64(map, OFFSET_MAGIC, MAGIC);
    write_u64(map, OFFSET_VERSION, HEADER_VERSION);
    write_u64(map, OFFSET_CHANNEL_TAG, contract.channel_tag);
    write_u64(map, OFFSET_SCHEMA_ID_TAG, contract.schema_id_tag);
    write_u64(map, OFFSET_SCHEMA_HASH_TAG, contract.schema_hash_tag);
    write_u64(
        map,
        OFFSET_SCHEMA_VERSION,
        u64::from(contract.schema_version),
    );
    write_u64(map, OFFSET_SLOT_SIZE, contract.slot_size as u64);
    write_u64(
        map,
        OFFSET_STALE_AFTER_READS,
        u64::from(contract.stale_after_reads),
    );
    write_u64(
        map,
        OFFSET_MAX_SPIN_RETRIES,
        u64::from(contract.max_spin_retries),
    );
    write_u64(map, OFFSET_MAX_SPIN_TIME_US, contract.max_spin_time_us);
    write_u64(
        map,
        OFFSET_OWNERSHIP,
        ownership_to_wire(contract.ownership) as u64,
    );
}

fn validate_header(map: &Mmap<ReadWrite>, contract: ShmChannelContract<'_>) -> Result<(), String> {
    if read_u64(map, OFFSET_MAGIC) != MAGIC {
        return Err("shared channel header magic mismatch".to_string());
    }
    if read_u64(map, OFFSET_VERSION) != HEADER_VERSION {
        return Err("shared channel header version mismatch".to_string());
    }
    if read_u64(map, OFFSET_CHANNEL_TAG) != contract.channel_tag {
        return Err("shared channel id contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_SCHEMA_ID_TAG) != contract.schema_id_tag {
        return Err("shared channel schema_id contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_SCHEMA_HASH_TAG) != contract.schema_hash_tag {
        return Err("shared channel schema_hash contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_SCHEMA_VERSION) != u64::from(contract.schema_version) {
        return Err("shared channel schema_version contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_SLOT_SIZE) != contract.slot_size as u64 {
        return Err("shared channel slot_size contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_STALE_AFTER_READS) != u64::from(contract.stale_after_reads) {
        return Err("shared channel stale_after_reads contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_MAX_SPIN_RETRIES) != u64::from(contract.max_spin_retries) {
        return Err("shared channel stale retry budget mismatch".to_string());
    }
    if read_u64(map, OFFSET_MAX_SPIN_TIME_US) != contract.max_spin_time_us {
        return Err("shared channel max_spin_time_us contract mismatch".to_string());
    }
    if read_u64(map, OFFSET_OWNERSHIP) != ownership_to_wire(contract.ownership) as u64 {
        return Err("shared channel ownership contract mismatch".to_string());
    }
    Ok(())
}

fn lock_pages(config: &T0ShmConfig, map: &Mmap<ReadWrite>) -> Result<Option<LockGuard>, String> {
    match config.pinning_mode {
        T0PinningMode::Disabled => Ok(None),
        T0PinningMode::BestEffort | T0PinningMode::Required => match config.pinning_provider {
            T0PinningProvider::None => Ok(None),
            T0PinningProvider::Os => match region::lock(map.as_ptr(), map.len()) {
                Ok(lock_guard) => Ok(Some(lock_guard)),
                Err(error) if matches!(config.pinning_mode, T0PinningMode::BestEffort) => {
                    let _ = error;
                    Ok(None)
                }
                Err(error) => Err(format!("failed to pin T0 SHM pages: {error}")),
            },
        },
    }
}

fn ownership_to_wire(ownership: T0ChannelOwnership) -> u8 {
    match ownership {
        T0ChannelOwnership::PublisherWrites => 1,
    }
}

fn read_u64(map: &Mmap<ReadWrite>, offset: usize) -> u64 {
    let end = offset.saturating_add(std::mem::size_of::<u64>());
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&map[offset..end]);
    u64::from_le_bytes(raw)
}

fn write_u64(map: &mut Mmap<ReadWrite>, offset: usize, value: u64) {
    let end = offset.saturating_add(std::mem::size_of::<u64>());
    map[offset..end].copy_from_slice(&value.to_le_bytes());
}
