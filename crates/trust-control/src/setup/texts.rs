//! The user-facing sentences of loop setup (A11.1), one child module per setup rule group. Each sentence
//! names the values involved, and a lower layer's message is kept, never replaced by a generic text.

pub mod configure;
pub mod discovery;
pub mod generators;
pub mod project;
pub mod prompts;
pub mod save;
