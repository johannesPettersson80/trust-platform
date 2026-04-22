impl BrowserAnalysisEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace_documents(
        &mut self,
        documents: Vec<DocumentInput>,
    ) -> EngineResult<ApplyDocumentsResult> {
        let mut seen = BTreeSet::new();
        for document in &documents {
            let uri = document.uri.trim();
            if uri.is_empty() {
                return Err(EngineError::new("document uri must not be empty"));
            }
            if !seen.insert(uri.to_string()) {
                return Err(EngineError::new(format!(
                    "duplicate document uri '{uri}' in request"
                )));
            }
        }

        let incoming_uris: BTreeSet<String> = documents.iter().map(|doc| doc.uri.clone()).collect();
        let stale: Vec<String> = self
            .documents
            .keys()
            .filter(|uri| !incoming_uris.contains(*uri))
            .cloned()
            .collect();
        for uri in stale {
            let key = source_key(&uri);
            let _ = self.project.remove_source(&key);
            self.documents.remove(&uri);
        }

        let mut loaded = Vec::with_capacity(documents.len());
        for document in documents {
            let file_id = self
                .project
                .set_source_text(source_key(&document.uri), document.text.clone());
            self.documents.insert(document.uri.clone(), document.text);
            loaded.push(LoadedDocument {
                uri: document.uri,
                file_id: file_id.0,
            });
        }

        Ok(ApplyDocumentsResult { documents: loaded })
    }

    pub fn diagnostics(&self, uri: &str) -> EngineResult<Vec<DiagnosticItem>> {
        let file_id = self.file_id_for_uri(uri)?;
        let source = self.source_for_uri(uri)?;
        let diagnostics = self
            .project
            .with_database(|db| trust_ide::diagnostics::collect_diagnostics(db, file_id));
        let mut items: Vec<DiagnosticItem> = diagnostics
            .into_iter()
            .map(|diagnostic| {
                let mut related = diagnostic
                    .related
                    .into_iter()
                    .map(|related| RelatedInfoItem {
                        range: lsp_range(source, related.range),
                        span: byte_span(related.range),
                        message: related.message,
                    })
                    .collect::<Vec<_>>();
                related.sort_by(|left, right| {
                    left.range
                        .cmp(&right.range)
                        .then_with(|| left.message.cmp(&right.message))
                });
                DiagnosticItem {
                    code: diagnostic.code.code().to_string(),
                    severity: severity_label(diagnostic.severity).to_string(),
                    message: diagnostic.message,
                    range: lsp_range(source, diagnostic.range),
                    span: byte_span(diagnostic.range),
                    related,
                }
            })
            .collect();
        items.sort_by(|left, right| {
            left.range
                .cmp(&right.range)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
                .then_with(|| left.severity.cmp(&right.severity))
        });
        Ok(items)
    }

    pub fn hover(&self, request: HoverRequest) -> EngineResult<Option<HoverItem>> {
        let file_id = self.file_id_for_uri(&request.uri)?;
        let source = self.source_for_uri(&request.uri)?;
        let Some(offset) = position_to_offset(source, request.position.clone()) else {
            return Err(EngineError::new(format!(
                "position {}:{} is outside document '{}'",
                request.position.line, request.position.character, request.uri
            )));
        };
        let result = self.project.with_database(|db| {
            trust_ide::hover_with_filter(
                db,
                file_id,
                TextSize::from(offset),
                &StdlibFilter::allow_all(),
            )
        });
        Ok(result.map(|hover| HoverItem {
            contents: hover.contents,
            range: hover.range.map(|range| lsp_range(source, range)),
        }))
    }

    pub fn completion(&self, request: CompletionRequest) -> EngineResult<Vec<CompletionItem>> {
        let file_id = self.file_id_for_uri(&request.uri)?;
        let source = self.source_for_uri(&request.uri)?;
        let Some(offset) = position_to_offset(source, request.position.clone()) else {
            return Err(EngineError::new(format!(
                "position {}:{} is outside document '{}'",
                request.position.line, request.position.character, request.uri
            )));
        };
        let mut items = self.project.with_database(|db| {
            trust_ide::complete_with_filter(
                db,
                file_id,
                TextSize::from(offset),
                &StdlibFilter::allow_all(),
            )
        });
        let typed_prefix = completion_prefix_at_offset(source, offset);
        items.sort_by(|left, right| {
            completion_match_rank(left.label.as_str(), typed_prefix.as_deref())
                .cmp(&completion_match_rank(
                    right.label.as_str(),
                    typed_prefix.as_deref(),
                ))
                .then_with(|| left.sort_priority.cmp(&right.sort_priority))
                .then_with(|| left.label.cmp(&right.label))
        });
        let limit = request.limit.unwrap_or(50).clamp(1, 500) as usize;
        let completion = items
            .into_iter()
            .take(limit)
            .map(|item| CompletionItem {
                label: item.label.to_string(),
                kind: completion_kind_label(item.kind).to_string(),
                detail: item.detail.map(|value| value.to_string()),
                documentation: item.documentation.map(|value| value.to_string()),
                insert_text: item.insert_text.map(|value| value.to_string()),
                text_edit: item.text_edit.map(|edit| CompletionTextEditItem {
                    range: lsp_range(source, edit.range),
                    new_text: edit.new_text.to_string(),
                }),
                sort_priority: item.sort_priority,
            })
            .collect();
        Ok(completion)
    }

    pub fn references(&self, request: ReferencesRequest) -> EngineResult<Vec<ReferenceItem>> {
        let file_id = self.file_id_for_uri(&request.uri)?;
        let source = self.source_for_uri(&request.uri)?;
        let Some(offset) = position_to_offset(source, request.position.clone()) else {
            return Err(EngineError::new(format!(
                "position {}:{} is outside document '{}'",
                request.position.line, request.position.character, request.uri
            )));
        };
        let include_declaration = request.include_declaration.unwrap_or(true);
        let refs = self.project.with_database(|db| {
            trust_ide::find_references(
                db,
                file_id,
                TextSize::from(offset),
                trust_ide::FindReferencesOptions {
                    include_declaration,
                },
            )
        });
        let mut items: Vec<ReferenceItem> = refs
            .into_iter()
            .filter_map(|reference| {
                let ref_uri = self.uri_for_file_id(reference.file_id)?;
                let ref_source = self.source_for_uri(&ref_uri).ok()?;
                Some(ReferenceItem {
                    uri: ref_uri,
                    range: lsp_range(ref_source, reference.range),
                    is_write: reference.is_write,
                })
            })
            .collect();
        items.sort_by(|a, b| a.uri.cmp(&b.uri).then_with(|| a.range.cmp(&b.range)));
        Ok(items)
    }

    pub fn definition(&self, request: DefinitionRequest) -> EngineResult<Option<DefinitionItem>> {
        let file_id = self.file_id_for_uri(&request.uri)?;
        let source = self.source_for_uri(&request.uri)?;
        let Some(offset) = position_to_offset(source, request.position.clone()) else {
            return Err(EngineError::new(format!(
                "position {}:{} is outside document '{}'",
                request.position.line, request.position.character, request.uri
            )));
        };
        let result = self
            .project
            .with_database(|db| trust_ide::goto_definition(db, file_id, TextSize::from(offset)));
        let item = result.and_then(|def| {
            let def_uri = self.uri_for_file_id(def.file_id)?;
            let def_source = self.source_for_uri(&def_uri).ok()?;
            Some(DefinitionItem {
                uri: def_uri,
                range: lsp_range(def_source, def.range),
            })
        });
        Ok(item)
    }

    pub fn document_highlight(
        &self,
        request: DocumentHighlightRequest,
    ) -> EngineResult<Vec<DocumentHighlightItem>> {
        let file_id = self.file_id_for_uri(&request.uri)?;
        let source = self.source_for_uri(&request.uri)?;
        let Some(offset) = position_to_offset(source, request.position.clone()) else {
            return Err(EngineError::new(format!(
                "position {}:{} is outside document '{}'",
                request.position.line, request.position.character, request.uri
            )));
        };
        let refs = self.project.with_database(|db| {
            trust_ide::find_references(
                db,
                file_id,
                TextSize::from(offset),
                trust_ide::FindReferencesOptions {
                    include_declaration: true,
                },
            )
        });
        let mut items: Vec<DocumentHighlightItem> = refs
            .into_iter()
            .filter(|reference| reference.file_id == file_id)
            .map(|reference| DocumentHighlightItem {
                range: lsp_range(source, reference.range),
                kind: if reference.is_write {
                    "write".to_string()
                } else {
                    "read".to_string()
                },
            })
            .collect();
        items.sort_by(|a, b| a.range.cmp(&b.range));
        Ok(items)
    }

    pub fn rename(&self, request: RenameRequest) -> EngineResult<Vec<RenameEdit>> {
        let file_id = self.file_id_for_uri(&request.uri)?;
        let source = self.source_for_uri(&request.uri)?;
        let Some(offset) = position_to_offset(source, request.position.clone()) else {
            return Err(EngineError::new(format!(
                "position {}:{} is outside document '{}'",
                request.position.line, request.position.character, request.uri
            )));
        };
        let result = self.project.with_database(|db| {
            trust_ide::rename(db, file_id, TextSize::from(offset), &request.new_name)
        });
        let Some(rename_result) = result else {
            return Ok(vec![]);
        };
        let mut edits: Vec<RenameEdit> = rename_result
            .edits
            .into_iter()
            .filter_map(|(edit_file_id, file_edits)| {
                let edit_uri = self.uri_for_file_id(edit_file_id)?;
                let edit_source = self.source_for_uri(&edit_uri).ok()?;
                Some(file_edits.into_iter().map(move |edit| RenameEdit {
                    uri: edit_uri.clone(),
                    range: lsp_range(edit_source, edit.range),
                    new_text: edit.new_text,
                }))
            })
            .flatten()
            .collect();
        edits.sort_by(|a, b| a.uri.cmp(&b.uri).then_with(|| a.range.cmp(&b.range)));
        Ok(edits)
    }

    pub fn status(&self) -> EngineStatus {
        EngineStatus {
            document_count: self.documents.len(),
            uris: self.documents.keys().cloned().collect(),
        }
    }

    fn source_for_uri(&self, uri: &str) -> EngineResult<&str> {
        self.documents
            .get(uri)
            .map(String::as_str)
            .ok_or_else(|| EngineError::new(format!("document '{uri}' is not loaded")))
    }

    fn file_id_for_uri(&self, uri: &str) -> EngineResult<FileId> {
        let key = source_key(uri);
        self.project
            .file_id_for_key(&key)
            .ok_or_else(|| EngineError::new(format!("document '{uri}' is not loaded")))
    }

    fn uri_for_file_id(&self, file_id: FileId) -> Option<String> {
        let key = self.project.key_for_file_id(file_id)?;
        Some(key.display())
    }
}
