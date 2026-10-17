//! Native tests of the view projection. They port the assertions of the panel's TypeScript tests
//! `network-canvas.test.ts` and `runtime-controls-contract.test.ts` with their numbers and texts,
//! and pin the rules of specification 25 §5.1, §5.5, §5.11 and §5.12. `golden` writes and
//! byte-compares the views that the webview's render tests read.

mod computers;
mod connections;
mod devices;
pub(crate) mod fixtures;
mod golden;
mod graph_scenarios;
mod health;
mod inputs;
mod issues;
mod peers;
mod runtimes;
mod scenarios;
mod search;
mod toolbar;
