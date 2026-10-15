//! Control Studio's loop setup (specification 35, B1 and §0.1b, and the setup parts of Part C): the
//! rules that turn what the compiled project states into a loop definition, the form the setup page
//! shows, the discovery of configured loops, the save rules and the step-by-step plan. Pure Rust:
//! the engine (`trust-control-studio`) reads the compiled catalog and the loop files and writes the
//! result; no function here touches the file system, a clock or a process.

pub mod binding_route;
pub mod catalog;
pub mod configure;
pub mod configure_adapter;
pub mod configure_channels;
pub mod discovery;
pub mod feedforward_loop;
pub mod form;
pub mod identify_loop;
pub mod loop_bindings;
pub mod messages;
pub mod own_law_loop;
pub mod pid_bindings;
pub mod pid_loop;
pub mod project_loop;
pub mod prompts;
pub mod save;
pub mod texts;
pub mod trajectory_loop;
