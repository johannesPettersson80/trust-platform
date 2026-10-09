// The link check in the ST file (specification 39, §8.6): when a faceplate field refers to a
// member that a pair's function block does not have, the block's name shows the problem, besides
// the pair file's own check. The pairs are those of the nearest folder above the file with
// `hmi/pairs/`, a project's or a library's; members are the block's own and its bases'.

/// The pair problems of an ST file, at the names of the blocks they concern, in the pair file's
/// words with the pair file named.
fn hmi_pair_diagnostics(state: &ServerState, uri: &Url, content: &str, file_id: FileId) -> Vec<Diagnostic> {
    let Some(path) = uri_to_path(uri) else { return Vec::new() };
    let Some(root) = path.ancestors().skip(1).take(8).find(|folder| folder.join("hmi/pairs").is_dir()).map(Path::to_path_buf) else {
        return Vec::new();
    };
    let pairs = hmi_pairs_of(&root);
    if pairs.is_empty() {
        return Vec::new();
    }
    let table = state.with_database(|db| db.file_symbols_with_project(file_id));
    let program = HmiSymbolProgram { table: &table };
    // Only the blocks this file declares show their pairs' problems.
    let declared: Vec<(String, &trust_hir::symbols::Symbol)> = program.blocks().filter(|(_, symbol)| symbol.origin.is_none()).collect();
    let mut diagnostics = Vec::new();
    for (file_name, pair) in pairs {
        let Some(found) = trust_runtime::hmi::binding_checks::block_named(declared.iter().map(|(name, _)| name.as_str()), &pair.block) else {
            continue;
        };
        let Some((_, block)) = declared.iter().find(|(name, _)| name == found) else { continue };
        let Some(faceplate) = hmi_faceplate_of(&root, &pair.faceplate) else { continue };
        for problem in trust_runtime::hmi::binding_checks::check_pair(&pair, &faceplate, &program) {
            diagnostics.push(Diagnostic {
                range: hmi_name_range(content, block.range.start().into(), &block.name),
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E-HMI-PAIR".to_string())),
                source: Some("truST".to_string()),
                message: format!("the pair hmi/pairs/{file_name}: {}", problem.message),
                ..Default::default()
            });
        }
    }
    diagnostics
}

/// The project's symbols as the pair check reads a program: its function blocks and programs by
/// their names with their namespaces, their members and the types those hold.
struct HmiSymbolProgram<'a> {
    table: &'a trust_hir::symbols::SymbolTable,
}

impl<'a> HmiSymbolProgram<'a> {
    /// The function blocks and programs, by their names with their namespaces.
    fn blocks(&self) -> impl Iterator<Item = (String, &'a trust_hir::symbols::Symbol)> + '_ {
        let table = self.table;
        table.iter().filter(|symbol| matches!(symbol.kind, SymbolKind::FunctionBlock | SymbolKind::Program)).map(move |symbol| {
            let mut parts = vec![symbol.name.to_string()];
            let mut parent = symbol.parent.and_then(|id| table.get(id));
            while let Some(namespace) = parent.filter(|candidate| matches!(candidate.kind, SymbolKind::Namespace)) {
                parts.push(namespace.name.to_string());
                parent = namespace.parent.and_then(|id| table.get(id));
            }
            parts.reverse();
            (parts.join("."), symbol)
        })
    }

    /// A block's own variables and parameters with their types.
    fn own_members(&self, owner: trust_hir::SymbolId) -> impl Iterator<Item = (String, trust_hir::TypeId)> + '_ {
        self.table
            .iter()
            .filter(move |symbol| symbol.parent == Some(owner) && matches!(symbol.kind, SymbolKind::Variable { .. } | SymbolKind::Parameter { .. }))
            .map(|symbol| (symbol.name.to_string(), symbol.type_id))
    }
}

impl trust_runtime::hmi::binding_checks::PairProgram for HmiSymbolProgram<'_> {
    fn block_members(&self, name: &str) -> Option<Vec<(String, trust_hir::TypeId)>> {
        let blocks: Vec<(String, &trust_hir::symbols::Symbol)> = self.blocks().collect();
        let found = trust_runtime::hmi::binding_checks::block_named(blocks.iter().map(|(name, _)| name.as_str()), name)?;
        let (_, block) = blocks.iter().find(|(candidate, _)| candidate == found)?;
        let mut members = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut current = Some(block.id);
        // A derived block has its bases' members too (IEC 61131-3 Ed.3 §6.6.5.3).
        while let Some(owner) = current.filter(|owner| seen.insert(*owner)) {
            members.extend(self.own_members(owner));
            current = self.table.extends_reference(owner).and_then(|reference| self.table.resolve_oop_reference_for_owner(owner, reference.name().display().as_str()));
        }
        Some(members)
    }

    fn type_of(&self, id: trust_hir::TypeId) -> Option<&trust_hir::Type> {
        self.table.type_by_id(self.table.resolve_alias_type(id))
    }
}

/// The pair files under a folder's `hmi/pairs/` that load, by file name.
fn hmi_pairs_of(root: &Path) -> Vec<(String, trust_runtime::hmi::page_files::HmiPair)> {
    let Ok(entries) = std::fs::read_dir(root.join("hmi/pairs")) else { return Vec::new() };
    let mut pairs: Vec<(String, trust_runtime::hmi::page_files::HmiPair)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(entry.path()).ok()?;
            name.ends_with(".toml").then_some(())?;
            let pair = trust_runtime::hmi::page_files::parse_pair(&format!("hmi/pairs/{name}"), &text).ok()?;
            Some((name, pair))
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

/// A faceplate by its name: the folder's own, else one of its libraries'.
fn hmi_faceplate_of(root: &Path, name: &str) -> Option<trust_runtime::hmi::page_files::HmiFaceplate> {
    let file = format!("hmi/panels/{name}.toml");
    std::iter::once(root.to_path_buf())
        .chain(trust_runtime::bundle_builder::dependency_roots(root).unwrap_or_default())
        .map(|folder| folder.join(&file))
        .find(|path| path.is_file())
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| trust_runtime::hmi::page_files::parse_faceplate(&file, &text).ok())
}

/// The range of a name at or after an offset: the block's name in its declaration.
fn hmi_name_range(content: &str, from: usize, name: &str) -> Range {
    let lower = content.to_ascii_lowercase();
    let wanted = name.to_ascii_lowercase();
    let mut at = lower.get(from..).and_then(|rest| rest.find(&wanted)).map(|offset| from + offset);
    // A whole word: not a part of a longer name.
    while let Some(start) = at {
        let before = lower[..start].chars().next_back();
        let after = lower[start + wanted.len()..].chars().next();
        let word = |character: Option<char>| character.is_some_and(|character| character == '_' || character.is_alphanumeric());
        if !word(before) && !word(after) {
            break;
        }
        at = lower[start + 1..].find(&wanted).map(|offset| start + 1 + offset);
    }
    let start = at.unwrap_or(from);
    let end = start + name.len().min(content.len().saturating_sub(start));
    let offset = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    Range { start: offset_to_position(content, offset(start)), end: offset_to_position(content, offset(end)) }
}
