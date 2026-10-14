//! The HMI agent's authoring engine in Rust (specification 43, §3 and §6; checklist H4).
//!
//! The HMI agent's authoring engine in Rust (specification 43, §3 to §6; checklist H4).
//!
//! The composition (`patterns`, `layout`, `compose`, `pattern_references`, `intent`), the page edits
//! (`edits`, `geometric_edits`, `pipe_edits`, `review`), the builder's editing rules they use
//! (`attachments`, `object_rules`, `pipe_rules`, `align`, `clipboard`), the quality checks (`adaptive`,
//! `ports`, `routes`, `geometry_quality`, `routing_quality`), the draft store (`documents`,
//! `draft_records`, `drafts`, `review_record`), the request boundary (`boundary`), the tool input schemas
//! (`schema`), the command argument (`argument`), the existing HMI commands it calls (`native`), the
//! context, library and inspection answers (`queries`, `query_metadata`), and the operations of the
//! authoring commands (`service`, `review_apply`).
//!
//! The extension keeps what presents and wires: the rendering and preview (AT-07, AT-08), the editor
//! transaction (AT-09) and the editor's quality warnings (`editor/quality.ts`), which depend on the drawing
//! engine and are not yet in Rust (see the report of this stage).

mod argument;
mod native;
mod queries;
mod query_metadata;
mod review_apply;
mod schema;
mod service;

pub(super) use service::{execute, is_command};

mod error;
mod geometry;

mod align;
mod attachments;
mod clipboard;
mod object_rules;
mod pipe_rules;

mod adaptive;
mod geometry_quality;
mod ports;
mod routes;
mod routing_quality;

mod documents;
mod draft_records;
mod drafts;
mod review_record;

mod compose;
mod edits;
mod geometric_edits;
mod intent;
mod layout;
mod pattern_references;
mod patterns;
mod pipe_edits;
mod review;

mod boundary;
