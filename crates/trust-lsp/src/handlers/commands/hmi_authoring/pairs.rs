//! Component definitions (specification 39, §8.5) are read and written by the runtime's one parser;
//! the language server adds no model of its own.
pub(crate) use trust_runtime::hmi::page_files::{
    parse_pair, write_pair, HmiDependency, HmiPair as Pair, HmiSetting, HmiTerminal,
    HmiTerminalDirection,
};
