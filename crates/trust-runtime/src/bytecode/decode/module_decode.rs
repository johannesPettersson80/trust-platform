impl BytecodeModule {
    pub fn decode(bytes: &[u8]) -> Result<Self, BytecodeError> {
        let mut reader = BytecodeReader::new(bytes);
        let magic = reader.read_bytes(4)?;
        if magic != MAGIC {
            return Err(BytecodeError::InvalidMagic);
        }
        let major = reader.read_u16()?;
        let minor = reader.read_u16()?;
        let flags = reader.read_u32()?;
        let header_size = reader.read_u16()?;
        let section_count = reader.read_u16()? as usize;
        let section_table_off = reader.read_u32()? as usize;
        let checksum = reader.read_u32()?;

        if header_size < HEADER_SIZE {
            return Err(BytecodeError::InvalidHeader("header size too small".into()));
        }
        if section_table_off < HEADER_SIZE as usize {
            return Err(BytecodeError::InvalidHeader(
                "section table before header".into(),
            ));
        }
        if !section_table_off.is_multiple_of(4) {
            return Err(BytecodeError::SectionAlignment);
        }

        let table_len = section_count
            .checked_mul(SECTION_ENTRY_SIZE)
            .ok_or_else(|| BytecodeError::InvalidSectionTable("section table overflow".into()))?;
        let table_end = section_table_off
            .checked_add(table_len)
            .ok_or_else(|| BytecodeError::InvalidSectionTable("section table overflow".into()))?;
        if table_end > bytes.len() {
            return Err(BytecodeError::InvalidSectionTable(
                "section table out of bounds".into(),
            ));
        }

        if flags & HEADER_FLAG_CRC32 != 0 {
            let actual = crc32fast::hash(&bytes[section_table_off..]);
            if actual != checksum {
                return Err(BytecodeError::InvalidChecksum {
                    expected: checksum,
                    actual,
                });
            }
        }

        if major != SUPPORTED_MAJOR_VERSION {
            return Err(BytecodeError::UnsupportedVersion { major, minor });
        }

        let mut entries = Vec::with_capacity(section_count);
        let mut table_reader = BytecodeReader::new(&bytes[section_table_off..table_end]);
        for _ in 0..section_count {
            let id = table_reader.read_u16()?;
            let flags = table_reader.read_u16()?;
            let offset = table_reader.read_u32()?;
            let length = table_reader.read_u32()?;
            entries.push(SectionEntry {
                id,
                flags,
                offset,
                length,
            });
        }

        validate_section_entries(bytes.len(), &entries)?;

        let mut sections = Vec::new();
        for entry in entries {
            let start = entry.offset as usize;
            let end = start + entry.length as usize;
            let payload = &bytes[start..end];
            let data = decode_section_data(BytecodeVersion { major, minor }, entry.id, payload)?;
            sections.push(Section {
                id: entry.id,
                flags: entry.flags,
                data,
            });
        }

        Ok(Self {
            version: BytecodeVersion { major, minor },
            flags,
            sections,
        })
    }
}
