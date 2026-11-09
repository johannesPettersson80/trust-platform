use super::*;

use rustc_hash::{FxHashMap, FxHashSet};
use text_size::TextRange;
use tower_lsp::lsp_types::DocumentSymbol;
use trust_hir::symbols::SymbolTable;
use trust_hir::{Symbol, SymbolId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

/// The outline of a document (specification 14 §6.3.3): a tree of its declarations, each with its
/// members, variables and parameters among them, by the symbols' parents.
pub fn document_symbol(
    state: &ServerState,
    params: DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let symbols = state.with_database(|db| db.file_symbols(doc.file_id));
    let root = parse(&doc.content).syntax();
    // Builtin symbols have an empty range at offset 0.
    let outlined: FxHashSet<SymbolId> = symbols
        .iter()
        .filter(|symbol| !symbol.range.is_empty())
        .map(|symbol| symbol.id)
        .collect();
    let mut children: FxHashMap<Option<SymbolId>, Vec<&Symbol>> = FxHashMap::default();
    for symbol in symbols
        .iter()
        .filter(|symbol| outlined.contains(&symbol.id))
    {
        let parent = symbol.parent.filter(|parent| outlined.contains(parent));
        children.entry(parent).or_default().push(symbol);
    }
    let outline = Outline {
        symbols: &symbols,
        children: &children,
        root: &root,
        content: &doc.content,
    };
    Some(DocumentSymbolResponse::Nested(outline.level(None)))
}

struct Outline<'a> {
    symbols: &'a SymbolTable,
    children: &'a FxHashMap<Option<SymbolId>, Vec<&'a Symbol>>,
    root: &'a SyntaxNode,
    content: &'a str,
}

impl Outline<'_> {
    /// The entries whose parent is `parent`, in source order, each with its own children.
    fn level(&self, parent: Option<SymbolId>) -> Vec<DocumentSymbol> {
        let mut level = self.children.get(&parent).cloned().unwrap_or_default();
        level.sort_by_key(|symbol| symbol.range.start());
        level
            .into_iter()
            .map(|symbol| {
                let selection = symbol.range;
                let range = declaration_range(self.root, selection)
                    .filter(|range| range.contains_range(selection))
                    .unwrap_or(selection);
                let children = self.level(Some(symbol.id));
                #[allow(deprecated)]
                DocumentSymbol {
                    name: display_symbol_name(self.symbols, symbol),
                    detail: variable_detail(self.symbols, symbol),
                    kind: lsp_symbol_kind(self.symbols, symbol),
                    tags: None,
                    deprecated: None,
                    range: self.lsp_range(range),
                    selection_range: self.lsp_range(selection),
                    children: (!children.is_empty()).then_some(children),
                }
            })
            .collect()
    }

    fn lsp_range(&self, range: TextRange) -> Range {
        Range {
            start: offset_to_position(self.content, range.start().into()),
            end: offset_to_position(self.content, range.end().into()),
        }
    }
}

/// The range of the whole declaration whose name is at `name`: the POU, namespace, type,
/// configuration element, enumeration value or variable declaration around it, without the
/// comments and blanks at its edges.
fn declaration_range(root: &SyntaxNode, name: TextRange) -> Option<TextRange> {
    if !root.text_range().contains_range(name) {
        return None;
    }
    let element = root.covering_element(name);
    let declaration = element
        .clone()
        .into_node()
        .or_else(|| element.into_token().and_then(|token| token.parent()))?
        .ancestors()
        .find(|node| {
            matches!(
                node.kind(),
                SyntaxKind::Program
                    | SyntaxKind::Function
                    | SyntaxKind::FunctionBlock
                    | SyntaxKind::Class
                    | SyntaxKind::Interface
                    | SyntaxKind::Method
                    | SyntaxKind::Property
                    | SyntaxKind::Action
                    | SyntaxKind::Namespace
                    | SyntaxKind::Configuration
                    | SyntaxKind::Resource
                    | SyntaxKind::TaskConfig
                    | SyntaxKind::ProgramConfig
                    | SyntaxKind::TypeDecl
                    | SyntaxKind::EnumValue
                    | SyntaxKind::VarDecl
            )
        })?;
    let mut tokens = declaration
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia());
    let first = tokens.next()?;
    let last = tokens.last().unwrap_or_else(|| first.clone());
    Some(TextRange::new(
        first.text_range().start(),
        last.text_range().end(),
    ))
}

/// A variable's, parameter's or constant's section and type: `VAR_INPUT : INT`.
fn variable_detail(symbols: &SymbolTable, symbol: &Symbol) -> Option<String> {
    let section = match symbol.kind {
        HirSymbolKind::Variable { qualifier } => qualifier.keyword(),
        HirSymbolKind::Parameter { direction } => direction.keyword(),
        HirSymbolKind::Constant => symbol
            .declared_qualifier
            .map_or("VAR", |qualifier| qualifier.keyword()),
        _ => return None,
    };
    let constant = if symbol.is_constant || matches!(symbol.kind, HirSymbolKind::Constant) {
        " CONSTANT"
    } else {
        ""
    };
    Some(match symbols.type_name(symbol.type_id) {
        Some(type_name) => format!("{section}{constant} : {type_name}"),
        None => format!("{section}{constant}"),
    })
}

pub fn workspace_symbol(
    state: &ServerState,
    params: WorkspaceSymbolParams,
) -> Option<Vec<SymbolInformation>> {
    let request_ticket = state.begin_semantic_request();
    workspace_symbol_with_ticket(state, params, request_ticket)
}

fn workspace_symbol_with_ticket(
    state: &ServerState,
    params: WorkspaceSymbolParams,
    request_ticket: u64,
) -> Option<Vec<SymbolInformation>> {
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let query = params.query.trim().to_lowercase();
    let query_empty = query.is_empty();

    let file_ids = state.with_database(|db| db.file_ids());
    let mut result = Vec::new();

    for file_id in file_ids {
        if state.semantic_request_cancelled(request_ticket) {
            return None;
        }

        let doc = match state.document_for_file_id(file_id) {
            Some(doc) => doc,
            None => continue,
        };

        let config = state.workspace_config_for_uri(&doc.uri);
        let (priority, visibility) = config
            .map(|config| (config.workspace.priority, config.workspace.visibility))
            .unwrap_or((0, WorkspaceVisibility::default()));
        if !visibility.allows_query(query_empty) {
            continue;
        }

        let symbols = state.with_database(|db| db.file_symbols(file_id));
        for symbol in symbols.iter() {
            if state.semantic_request_cancelled(request_ticket) {
                return None;
            }

            let name = display_symbol_name(&symbols, symbol);
            if !query_empty && !name.to_lowercase().contains(&query) {
                continue;
            }

            let kind = lsp_symbol_kind(&symbols, symbol);
            let range = Range {
                start: offset_to_position(&doc.content, symbol.range.start().into()),
                end: offset_to_position(&doc.content, symbol.range.end().into()),
            };
            let container_name = symbol_container_name(&symbols, symbol);

            #[allow(deprecated)]
            result.push((
                priority,
                SymbolInformation {
                    name,
                    kind,
                    location: Location {
                        uri: doc.uri.clone(),
                        range,
                    },
                    container_name,
                    tags: None,
                    deprecated: None,
                },
            ));
        }
    }

    result.sort_by(|(prio_a, sym_a), (prio_b, sym_b)| {
        prio_b
            .cmp(prio_a)
            .then_with(|| sym_a.name.cmp(&sym_b.name))
            .then_with(|| sym_a.location.uri.as_str().cmp(sym_b.location.uri.as_str()))
            // Keep snapshot order deterministic when multiple symbols share name+uri.
            .then_with(|| sym_a.container_name.cmp(&sym_b.container_name))
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .start
                    .line
                    .cmp(&sym_b.location.range.start.line)
            })
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .start
                    .character
                    .cmp(&sym_b.location.range.start.character)
            })
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .end
                    .line
                    .cmp(&sym_b.location.range.end.line)
            })
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .end
                    .character
                    .cmp(&sym_b.location.range.end.character)
            })
    });
    let result = result.into_iter().map(|(_, symbol)| symbol).collect();
    Some(result)
}

pub async fn workspace_symbol_with_progress(
    client: &Client,
    state: &ServerState,
    params: WorkspaceSymbolParams,
) -> Option<Vec<SymbolInformation>> {
    let work_done_token = params.work_done_progress_params.work_done_token.clone();
    let partial_token = params.partial_result_params.partial_result_token.clone();
    let message = if params.query.trim().is_empty() {
        None
    } else {
        Some(format!("Query: {}", params.query))
    };
    send_work_done_begin(
        client,
        &work_done_token,
        "Searching workspace symbols",
        message,
    )
    .await;

    let result = workspace_symbol(state, params);

    let streamed_partials = partial_token.is_some();
    if let Some(symbols) = result.as_ref()
        && streamed_partials
    {
        let total = symbols.len().max(1);
        let mut emitted = 0usize;
        for chunk in symbols.chunks(PARTIAL_CHUNK_SIZE) {
            send_partial_result(client, &partial_token, chunk.to_vec()).await;
            emitted = emitted.saturating_add(chunk.len());
            let percentage = ((emitted as f64 / total as f64) * 100.0).round() as u32;
            send_work_done_report(
                client,
                &work_done_token,
                Some(format!("Symbols: {emitted}/{total}")),
                Some(percentage.min(100)),
            )
            .await;
        }
    }

    let count = result.as_ref().map(|items| items.len()).unwrap_or(0);
    send_work_done_end(
        client,
        &work_done_token,
        Some(format!("Found {count} symbol(s)")),
    )
    .await;
    if streamed_partials {
        result.map(|_| Vec::new())
    } else {
        result
    }
}
