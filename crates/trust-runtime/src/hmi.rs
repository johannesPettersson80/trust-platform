//! HMI Builder runtime support (specification 39): page files, the component catalogue, the
//! member paths that page objects bind, the variable tree the editor offers for binding, and the
//! binding checks.

pub mod access;
pub mod alarms;
pub mod binding_checks;
pub mod bundle;
pub mod catalogue;
pub mod commands;
pub mod display_values;
pub mod exchange;
pub mod faceplate_fit;
pub(crate) mod instance_holdings;
pub(crate) mod instance_walk;
pub mod member_paths;
pub(crate) mod method_exchange;
pub mod my_symbols;
pub mod page_files;
pub mod presentation;
pub mod remote_operations;
pub mod remote_values;
pub mod setting_values;
pub mod texts;
pub mod trend_history;
pub mod variable_tree;
pub mod write_value;
