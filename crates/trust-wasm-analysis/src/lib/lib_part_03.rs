#[cfg_attr(all(target_arch = "wasm32", feature = "wasm"), wasm_bindgen)]
pub struct WasmAnalysisEngine {
    inner: BrowserAnalysisEngine,
}

#[cfg_attr(all(target_arch = "wasm32", feature = "wasm"), wasm_bindgen)]
impl WasmAnalysisEngine {
    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(constructor)
    )]
    pub fn new() -> Self {
        Self {
            inner: BrowserAnalysisEngine::new(),
        }
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = applyDocumentsJson)
    )]
    pub fn apply_documents_json(&mut self, documents_json: &str) -> Result<String, String> {
        let documents: Vec<DocumentInput> = serde_json::from_str(documents_json)
            .map_err(|err| format!("invalid documents json: {err}"))?;
        let result = self.inner.replace_documents(documents)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = diagnosticsJson)
    )]
    pub fn diagnostics_json(&self, uri: &str) -> Result<String, String> {
        let result = self.inner.diagnostics(uri)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = canonicalAstJson)
    )]
    pub fn canonical_ast_json(&self, source: &str) -> Result<String, String> {
        json_string(&canonical_ast_summary(source))
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = astSimilarityJson)
    )]
    pub fn ast_similarity_json(&self, request_json: &str) -> Result<String, String> {
        let request: AstSimilarityRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid ast similarity request json: {err}"))?;
        json_string(&canonical_ast_similarity(&request.left, &request.right))
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = hoverJson)
    )]
    pub fn hover_json(&self, request_json: &str) -> Result<String, String> {
        let request: HoverRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid hover request json: {err}"))?;
        let result = self.inner.hover(request)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = completionJson)
    )]
    pub fn completion_json(&self, request_json: &str) -> Result<String, String> {
        let request: CompletionRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid completion request json: {err}"))?;
        let result = self.inner.completion(request)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = referencesJson)
    )]
    pub fn references_json(&self, request_json: &str) -> Result<String, String> {
        let request: ReferencesRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid references request json: {err}"))?;
        let result = self.inner.references(request)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = definitionJson)
    )]
    pub fn definition_json(&self, request_json: &str) -> Result<String, String> {
        let request: DefinitionRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid definition request json: {err}"))?;
        let result = self.inner.definition(request)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = documentHighlightJson)
    )]
    pub fn document_highlight_json(&self, request_json: &str) -> Result<String, String> {
        let request: DocumentHighlightRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid documentHighlight request json: {err}"))?;
        let result = self.inner.document_highlight(request)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = renameJson)
    )]
    pub fn rename_json(&self, request_json: &str) -> Result<String, String> {
        let request: RenameRequest = serde_json::from_str(request_json)
            .map_err(|err| format!("invalid rename request json: {err}"))?;
        let result = self.inner.rename(request)?;
        json_string(&result)
    }

    #[cfg_attr(
        all(target_arch = "wasm32", feature = "wasm"),
        wasm_bindgen(js_name = statusJson)
    )]
    pub fn status_json(&self) -> Result<String, String> {
        json_string(&self.inner.status())
    }
}

impl Default for WasmAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}
