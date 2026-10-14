impl Runtime {
    /// Get the current debug control handle, if set.
    #[must_use]
    pub fn debug_control(&self) -> Option<crate::debug::DebugControl> {
        self.debug.clone()
    }

    /// Register statement locations for a file id.
    pub fn register_statement_locations(
        &mut self,
        file_id: u32,
        locations: Vec<crate::debug::SourceLocation>,
    ) {
        self.statement_index.insert(file_id, locations);
    }

    /// Register source text used for debug location remapping.
    pub fn register_source_text(&mut self, file_id: u32, text: impl Into<String>) {
        self.source_text_index.insert(file_id, text.into());
    }

    /// Register a source label alias (path or virtual file label) for a file id.
    pub fn register_source_label(&mut self, file_id: u32, label: impl Into<smol_str::SmolStr>) {
        self.source_label_index.insert(label.into(), file_id);
    }

    /// Resolve a breakpoint to a statement location for the given file and source.
    #[must_use]
    pub fn resolve_breakpoint_location(
        &self,
        source: &str,
        file_id: u32,
        line: u32,
        column: u32,
    ) -> Option<crate::debug::SourceLocation> {
        let locations = self.statement_index.get(&file_id)?;
        crate::debug::resolve_breakpoint_location(source, file_id, locations, line, column)
    }

    pub(super) fn resolve_vm_debug_location(
        &self,
        file_label: &str,
        line_1based: u32,
        column_1based: u32,
    ) -> Option<crate::debug::SourceLocation> {
        let file_id = self
            .source_label_index
            .get(file_label)
            .copied()
            .or_else(|| parse_virtual_file_label(file_label))?;
        let source = self.source_text_index.get(&file_id)?;
        self.resolve_breakpoint_location(
            source,
            file_id,
            line_1based.saturating_sub(1),
            column_1based.saturating_sub(1),
        )
    }
}

fn parse_virtual_file_label(label: &str) -> Option<u32> {
    let suffix = label.strip_prefix("file_")?;
    suffix.parse::<u32>().ok()
}
