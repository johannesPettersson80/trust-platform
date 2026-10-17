//! Native tests of the topology merge. They port the assertions of the panel's TypeScript tests
//! `network-canvas-fleet-identity.test.ts`, `network-canvas.test.ts` and
//! `connector-status-contract.test.ts` with their numbers and texts; what those tests assert of
//! the drawing stays with the view.

pub(crate) mod fixtures;
mod identity;
mod merge;
mod synthetic;
