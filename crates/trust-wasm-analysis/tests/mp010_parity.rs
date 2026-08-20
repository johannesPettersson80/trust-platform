use std::hint::black_box;
use std::time::{Duration, Instant};

use text_size::{TextRange, TextSize};
use trust_hir::project::{Project, SourceKey};
use trust_hir::DiagnosticSeverity;
use trust_ide::StdlibFilter;
use trust_wasm_analysis::{
    ApplyDocumentsResult, BrowserAnalysisEngine, CompletionItem, CompletionRequest,
    DefinitionRequest, DocumentHighlightRequest, DocumentInput, EngineStatus, HoverItem,
    HoverRequest, Position, Range, ReferencesRequest, RelatedInfoItem, RenameRequest,
    WasmAnalysisEngine,
};

include!("../../../tests/support/host_runtime_oracle.rs");

#[path = "mp010_parity/mp010_parity_part_01.rs"]
mod mp010_parity_part_01;
#[path = "mp010_parity/mp010_parity_part_02.rs"]
mod mp010_parity_part_02;
#[path = "mp010_parity/mp010_parity_part_03.rs"]
mod mp010_parity_part_03;
#[path = "mp010_parity/mp010_parity_part_04.rs"]
mod mp010_parity_part_04;
#[path = "mp010_parity/mp010_parity_part_05.rs"]
mod mp010_parity_part_05;

#[allow(unused_imports)]
use mp010_parity_part_01::*;
#[allow(unused_imports)]
use mp010_parity_part_02::*;
#[allow(unused_imports)]
use mp010_parity_part_03::*;
#[allow(unused_imports)]
use mp010_parity_part_04::*;
#[allow(unused_imports)]
use mp010_parity_part_05::*;
