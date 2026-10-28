//! The builder's view of a project: page documents, equipment definitions, declarations,
//! hardware terminals and settings, read from sources without compiling them.
pub(crate) mod bundle;
pub(crate) mod checks;
pub(crate) mod command_checks;
pub(crate) mod commands;
pub(crate) mod components;
pub(crate) mod declaration_walk;
pub(crate) mod declarations;
pub(crate) mod design_variables;
pub(crate) mod documents;
pub(crate) mod equipment_settings;
pub(crate) mod io;
pub(crate) mod pairs;
pub(crate) mod placements;
pub(crate) mod propose;
pub(crate) mod sources;
pub(crate) mod tasks;
